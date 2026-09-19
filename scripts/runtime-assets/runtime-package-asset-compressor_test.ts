import { expect, test } from "bun:test";
import { brotliDecompressSync } from "node:zlib";
import {
  buildDrawioResourceArchive,
  buildZenumlRuntimeAssetArchive,
  compressRuntimePackageAsset,
} from "./runtime-package-asset-compressor";

test("配布用圧縮資産は決定的かつ元バイト列へ復元できる", () => {
  const source = Buffer.from("const runtimeAsset = '刀'.repeat(1024);\n", "utf8");

  const first = compressRuntimePackageAsset(source);
  const second = compressRuntimePackageAsset(source);

  expect(first.equals(second)).toBe(true);
  expect(brotliDecompressSync(first).equals(source)).toBe(true);
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
