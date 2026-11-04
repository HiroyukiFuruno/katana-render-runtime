import { afterEach, expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { brotliDecompressSync } from "node:zlib";
import { RuntimeAssetCatalog, RuntimeAssetPaths } from "./runtime-asset-common";
import {
  buildDrawioResourceArchive,
  buildZenumlRuntimeAssetArchive,
  compressRuntimePackageAsset,
  generatedRuntimeRustAssetInputPaths,
  generatedRuntimeRustAssetPaths,
  parseCompressionOptions,
  syncDrawioLicenseFromVendor,
} from "./runtime-package-asset-compressor";

const temporaryRoots: string[] = [];

afterEach(() => {
  for (const root of temporaryRoots.splice(0)) fs.rmSync(root, { recursive: true, force: true });
});

function temporaryRoot(): string {
  const root = fs.realpathSync(
    fs.mkdtempSync(path.join(os.tmpdir(), "krr-runtime-generated-paths-")),
  );
  temporaryRoots.push(root);
  return root;
}

function writeNames(root: string, names: string[]): void {
  for (const name of names) {
    const file = path.join(root, name);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, "names-only");
  }
}

async function runCompressorCli(root: string, args: string[]) {
  const cli = Bun.spawn(
    [
      process.execPath,
      "run",
      fileURLToPath(new URL("./runtime-package-asset-compressor.ts", import.meta.url)),
      ...args,
    ],
    { cwd: root, stdout: "pipe", stderr: "pipe" },
  );
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(cli.stdout).text(),
    new Response(cli.stderr).text(),
    cli.exited,
  ]);
  return { stdout, stderr, exitCode };
}

test("配布用圧縮資産は決定的かつ元バイト列へ復元できる", () => {
  const source = Buffer.from("const runtimeAsset = '刀'.repeat(1024);\n", "utf8");

  const first = compressRuntimePackageAsset(source);
  const second = compressRuntimePackageAsset(source);

  expect(first.equals(second)).toBe(true);
  expect(brotliDecompressSync(first).equals(source)).toBe(true);
});

test("Draw.io package license はvendor原文へ同期し、欠落と不一致を拒否する", () => {
  const root = temporaryRoot();
  const drawio = RuntimeAssetCatalog.byKind("drawio");
  const vendorLicense = path.join(
    root,
    `crates/katana-render-runtime/vendor/drawio/${drawio.version}/LICENSE`,
  );
  const packageLicense = path.join(
    root,
    "crates/katana-render-runtime-assets/assets/licenses/drawio/LICENSE",
  );
  fs.mkdirSync(path.dirname(vendorLicense), { recursive: true });
  fs.writeFileSync(vendorLicense, "official license bytes\n");

  syncDrawioLicenseFromVendor(root, "--write");
  expect(fs.readFileSync(packageLicense).equals(fs.readFileSync(vendorLicense))).toBe(true);
  expect(() => syncDrawioLicenseFromVendor(root, "--check")).not.toThrow();

  fs.writeFileSync(packageLicense, "stale license bytes\n");
  expect(() => syncDrawioLicenseFromVendor(root, "--check")).toThrow(
    "Draw.io package license is missing or differs from vendor source",
  );
  fs.rmSync(packageLicense);
  expect(() => syncDrawioLicenseFromVendor(root, "--check")).toThrow(
    "Draw.io package license is missing or differs from vendor source",
  );
});

test("ZenUML runtime assets は共通archive内のoffsetとlengthを生成する", () => {
  const archive = buildZenumlRuntimeAssetArchive([
    { kind: "mermaid-zenuml", bytes: Buffer.from("PLUGIN") },
    { kind: "zenuml-core", bytes: Buffer.from("CORE") },
  ]);

  expect(brotliDecompressSync(archive.compressedBytes).toString()).toBe("COREPLUGIN");
  expect(archive.indexSource).toContain("ZENUML_CORE_ASSET_OFFSET: usize = 0");
  expect(archive.indexSource).toContain("ZENUML_CORE_ASSET_LENGTH: usize = 4");
  expect(archive.indexSource.match(/#\[cfg\(test\)\]/g)).toHaveLength(2);
  expect(archive.indexSource).toContain("MERMAID_ZENUML_ASSET_OFFSET: usize = 4");
  expect(archive.indexSource).toContain("MERMAID_ZENUML_ASSET_LENGTH: usize = 6");
});

test("Draw.io resource archive はgroupごとに独立圧縮しRust indexを生成する", () => {
  const archive = buildDrawioResourceArchive([
    { path: "z/file.svg", bytes: Buffer.from("Z") },
    { path: "a/file.xml", bytes: Buffer.from("ABC") },
  ]);

  const [firstGroupInfo, secondGroupInfo] = archive.groups;
  if (firstGroupInfo === undefined || secondGroupInfo === undefined) {
    throw new Error("expected two Draw.io archive groups");
  }
  expect(firstGroupInfo.name).toBe("a");
  expect(secondGroupInfo.name).toBe("z");
  const firstGroup = archive.compressedBytes.subarray(
    firstGroupInfo.compressedStart,
    firstGroupInfo.compressedStart + firstGroupInfo.compressedLength,
  );
  const secondGroup = archive.compressedBytes.subarray(
    secondGroupInfo.compressedStart,
    secondGroupInfo.compressedStart + secondGroupInfo.compressedLength,
  );
  expect(brotliDecompressSync(firstGroup).toString()).toBe("ABC");
  expect(brotliDecompressSync(secondGroup).toString()).toBe("Z");
  expect(archive.indexSources).toHaveLength(2);
  expect(archive.indexSources.map((it) => it.fileName)).toEqual([
    "drawio-resources-a-index.rs",
    "drawio-resources-z-index.rs",
  ]);
  expect(archive.indexSources.at(0)?.source).toContain('("a/file.xml", 0, 3)');
  expect(archive.indexSources.at(1)?.source).toContain('("z/file.svg", 0, 1)');
  expect(archive.indexSource).toContain("compressed_start: 0");
  expect(archive.indexSource).toContain("uncompressed_length: 3");
  expect(archive.indexSource).toContain("DRAWIO_RESOURCE_ARCHIVE_GROUPS");
});

test("Draw.io stencil resources は旧flatten順を維持しdirect・basic・nestedを別groupで展開する", () => {
  const archive = buildDrawioResourceArchive([
    { path: "stencils/aws4.xml", bytes: Buffer.from("AWS4") },
    { path: "stencils/basic.xml", bytes: Buffer.from("BASIC") },
    { path: "stencils/bpmn.xml", bytes: Buffer.from("BPMN") },
    { path: "stencils/archimate/archimate.xml", bytes: Buffer.from("NESTED") },
  ]);

  expect(archive.indexSources.map((source) => source.fileName)).toEqual([
    "drawio-resources-stencils-archimate-index.rs",
    "drawio-resources-stencils-aws4-index.rs",
    "drawio-resources-stencils-bpmn-index.rs",
    "drawio-resources-stencils-basic-index.rs",
  ]);
  expect(
    archive.groups.map((group) => ({
      name: group.name,
      uncompressedLength: group.uncompressedLength,
      bytes: brotliDecompressSync(
        archive.compressedBytes.subarray(
          group.compressedStart,
          group.compressedStart + group.compressedLength,
        ),
      ).toString(),
    })),
  ).toEqual([
    { name: "stencils-archimate", uncompressedLength: 6, bytes: "NESTED" },
    { name: "stencils-aws4", uncompressedLength: 4, bytes: "AWS4" },
    { name: "stencils-bpmn", uncompressedLength: 4, bytes: "BPMN" },
    { name: "stencils-basic", uncompressedLength: 5, bytes: "BASIC" },
  ]);
});

test("generated Rust path inventory reuses resource group naming and excludes non-archive roots", () => {
  const root = temporaryRoot();
  const resourceRoot = path.join(
    root,
    "crates/katana-render-runtime/src/markdown/drawio_renderer/js_runtime/resources",
  );
  writeNames(resourceRoot, [
    "icons/one.svg",
    "icons/two.svg",
    "stencils/aws4.xml",
    "stencils/archimate/archimate.xml",
    "drawio-libs/ignored.js",
    "drawio-resource-manifest.json",
  ]);

  expect(generatedRuntimeRustAssetPaths(root)).toEqual([
    "crates/katana-render-runtime/src/markdown/drawio_renderer/generated/drawio-resources-icons-index.rs",
    "crates/katana-render-runtime/src/markdown/drawio_renderer/generated/drawio-resources-index.rs",
    "crates/katana-render-runtime/src/markdown/drawio_renderer/generated/drawio-resources-stencils-archimate-index.rs",
    "crates/katana-render-runtime/src/markdown/drawio_renderer/generated/drawio-resources-stencils-aws4-index.rs",
    "crates/katana-render-runtime/src/markdown/generated/zenuml-runtime-assets-index.rs",
  ]);
  expect(generatedRuntimeRustAssetInputPaths(root)).toEqual([
    "crates/katana-render-runtime/src/markdown/drawio_renderer/js_runtime/resources/icons/one.svg",
    "crates/katana-render-runtime/src/markdown/drawio_renderer/js_runtime/resources/icons/two.svg",
    "crates/katana-render-runtime/src/markdown/drawio_renderer/js_runtime/resources/stencils/archimate/archimate.xml",
    "crates/katana-render-runtime/src/markdown/drawio_renderer/js_runtime/resources/stencils/aws4.xml",
    "crates/katana-render-runtime/vendor/mermaid-zenuml/1.0.1/mermaid-zenuml.min.js",
    RuntimeAssetPaths.assetFile(RuntimeAssetCatalog.byKind("zenuml-core"))
      .split(path.sep)
      .join("/"),
  ]);
});

test("ZenUML限定CLIは専用archiveとindexだけを書き、引数異常を拒否する", async () => {
  expect(parseCompressionOptions(["--write", "--zenuml-only"])).toEqual({
    mode: "--write",
    scope: "zenuml-only",
  });
  expect(parseCompressionOptions(["--zenuml-only", "--check"])).toEqual({
    mode: "--check",
    scope: "zenuml-only",
  });
  expect(parseCompressionOptions(["--write"])).toEqual({ mode: "--write", scope: "all" });
  for (const args of [
    ["--write", "--zenuml-only", "--zenuml-only"],
    ["--write", "--unknown"],
    ["--write", "--check"],
    ["--zenuml-only"],
  ]) {
    expect(() => parseCompressionOptions(args)).toThrow(
      "Usage: runtime-package-asset-compressor.ts",
    );
  }

  const root = temporaryRoot();
  const zenumlArchive = path.join(
    root,
    "crates/katana-render-runtime/src/markdown/generated/zenuml-runtime-assets.bin.br",
  );
  const zenumlIndex = path.join(
    root,
    "crates/katana-render-runtime/src/markdown/generated/zenuml-runtime-assets-index.rs",
  );
  const drawioArchive = path.join(
    root,
    "crates/katana-render-runtime/src/markdown/drawio_renderer/generated/drawio-resources.bin.br",
  );
  const drawioIndex = path.join(
    root,
    "crates/katana-render-runtime/src/markdown/drawio_renderer/generated/drawio-resources-index.rs",
  );
  const drawioLicense = path.join(
    root,
    "crates/katana-render-runtime-assets/assets/licenses/drawio/LICENSE",
  );
  const mermaidCompressedPath = RuntimeAssetPaths.compressedAssetFile(
    RuntimeAssetCatalog.byKind("mermaid"),
  );
  const mermaidCompressed = path.join(root, mermaidCompressedPath);
  const zenumlCoreSource = RuntimeAssetPaths.assetFile(RuntimeAssetCatalog.byKind("zenuml-core"));
  const mermaidZenumlSource = RuntimeAssetPaths.assetFile(
    RuntimeAssetCatalog.byKind("mermaid-zenuml"),
  );
  writeNames(root, [
    zenumlCoreSource,
    mermaidZenumlSource,
    "crates/katana-render-runtime/src/markdown/drawio_renderer/generated/drawio-resources.bin.br",
    "crates/katana-render-runtime/src/markdown/drawio_renderer/generated/drawio-resources-index.rs",
    "crates/katana-render-runtime-assets/assets/licenses/drawio/LICENSE",
    mermaidCompressedPath,
  ]);
  const untouchedPaths = [drawioArchive, drawioIndex, drawioLicense, mermaidCompressed];
  const untouchedBefore = untouchedPaths.map((file) => fs.readFileSync(file));

  const writeResult = await runCompressorCli(root, ["--write", "--zenuml-only"]);

  expect(writeResult.exitCode).toBe(0);
  expect(fs.existsSync(zenumlArchive)).toBe(true);
  expect(fs.existsSync(zenumlIndex)).toBe(true);
  expect(untouchedPaths.map((file) => fs.readFileSync(file))).toEqual(untouchedBefore);
  expect((await runCompressorCli(root, ["--zenuml-only", "--check"])).exitCode).toBe(0);
  fs.writeFileSync(drawioArchive, "changed outside selected scope");
  expect((await runCompressorCli(root, ["--check", "--zenuml-only"])).exitCode).toBe(0);
});
