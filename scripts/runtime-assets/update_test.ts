import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { DrawioWarExtractor } from "./drawio-war-extractor";
import { RuntimeAssetChecksum } from "./runtime-asset-common";
import { RuntimeSourceUpdater } from "./update";

test("Draw.io WAR から 1MiB を超える viewer.min.js を展開できる", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "katana-drawio-war-"));
  try {
    const jsDir = path.join(root, "js");
    fs.mkdirSync(jsDir, { recursive: true });
    const sourceFile = path.join(jsDir, "viewer.min.js");
    fs.writeFileSync(sourceFile, `const app = "${"x".repeat(1_200_000)}";\n`, "utf8");

    const archive = path.join(root, "draw.war");
    const zipped = spawnSync("zip", ["-q", archive, "js/viewer.min.js"], {
      cwd: root,
      encoding: "utf8",
    });
    if (zipped.status !== 0) {
      throw new Error(`zip failed: ${zipped.stderr}`);
    }

    const target = path.join(root, "drawio.min.js");
    new DrawioWarExtractor().extract(archive, target);

    expect(fs.statSync(target).size).toBeGreaterThan(1024 * 1024);
    expect(RuntimeAssetChecksum.digestFile(target)).toBe(
      RuntimeAssetChecksum.digestFile(sourceFile),
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("Rust runtime asset version const を 1 行形式でも更新できる", () => {
  const source = 'pub const DRAWIO_JS_VERSION: &str = "29.7.10";\n';

  const updated = new RuntimeSourceUpdater().replaceConst(source, "DRAWIO_JS_VERSION", "30.0.1");

  expect(updated).toBe('pub const DRAWIO_JS_VERSION: &str = "30.0.1";\n');
});

test("Draw.io の静的 assets crate 参照は version 更新で変化しない", () => {
  const source = "bytes: katana_render_runtime_assets::DRAWIO_RUNTIME_BROTLI,\n";
  const drawio = {
    kind: "drawio",
    displayName: "Draw.io",
    version: "32.0.2",
    checksum: "checksum",
    fileName: "drawio.min.js",
    rustVersionConst: "DRAWIO_JS_VERSION",
    rustChecksumConst: "DRAWIO_JS_CHECKSUM",
    rustDownloadConst: "DRAWIO_DOWNLOAD_URL",
    latestUrl: "latest",
    releasePageUrl: (version: string) => version,
    downloadUrl: (version: string) => version,
  } as const;

  expect(new RuntimeSourceUpdater().replaceVendorAssetVersion(source, drawio, "32.1.0")).toBe(
    source,
  );
});

test("長い Rust const も rustfmt と同じ 1 行形式で更新する", () => {
  const source = 'pub const PLANTUML_DOWNLOAD_URL: &str = "old";\n';
  const value =
    "https://repo1.maven.org/maven2/net/sourceforge/plantuml/plantuml-lgpl/1.2026.7/plantuml-lgpl-1.2026.7.jar";

  const updated = new RuntimeSourceUpdater().replaceConst(source, "PLANTUML_DOWNLOAD_URL", value);

  expect(updated).toBe(`pub const PLANTUML_DOWNLOAD_URL: &str = "${value}";\n`);
});

test("PlantUML package include は checksum manifest だけを更新する", () => {
  const plantuml = {
    kind: "plantuml",
    displayName: "PlantUML JAR",
    version: "1.2026.2",
    checksum: "checksum",
    fileName: "plantuml.jar",
    rustVersionConst: "PLANTUML_JAR_VERSION",
    rustChecksumConst: "PLANTUML_JAR_CHECKSUM",
    rustDownloadConst: "PLANTUML_DOWNLOAD_URL",
    latestUrl: "latest",
    releasePageUrl: (version: string) => version,
    downloadUrl: (version: string) => version,
  } as const;
  const source = 'include = ["vendor/plantuml/1.2026.2/plantuml.jar.sha256",]\n';

  const updated = new RuntimeSourceUpdater().replacePackageIncludeVersion(
    source,
    plantuml,
    "1.2026.4",
  );

  expect(updated).toBe('include = ["vendor/plantuml/1.2026.4/plantuml.jar.sha256",]\n');
});

test("Draw.io version 更新はruntime checksum pathだけを更新する", () => {
  const drawio = {
    kind: "drawio",
    displayName: "Draw.io",
    version: "32.0.2",
    checksum: "checksum",
    fileName: "drawio.min.js",
    rustVersionConst: "DRAWIO_JS_VERSION",
    rustChecksumConst: "DRAWIO_JS_CHECKSUM",
    rustDownloadConst: "DRAWIO_DOWNLOAD_URL",
    latestUrl: "latest",
    releasePageUrl: (version: string) => version,
    downloadUrl: (version: string) => version,
  } as const;
  const source = [
    '"vendor/drawio/32.0.2/drawio.min.js.sha256",',
    '"assets/drawio.min.js.br",',
  ].join("\n");

  const updated = new RuntimeSourceUpdater().replacePackageIncludeVersion(source, drawio, "33.0.0");

  expect(updated).toBe('"vendor/drawio/33.0.0/drawio.min.js.sha256",\n"assets/drawio.min.js.br",');
});

test("圧縮配布資産の package include は全ファイルを同じ version へ更新する", () => {
  const mermaid = {
    kind: "mermaid",
    displayName: "Mermaid.js",
    version: "11.17.2",
    checksum: "checksum",
    fileName: "mermaid.min.js",
    rustVersionConst: "MERMAID_JS_VERSION",
    rustChecksumConst: "MERMAID_JS_CHECKSUM",
    rustDownloadConst: "MERMAID_DOWNLOAD_URL",
    latestUrl: "latest",
    releasePageUrl: (version: string) => version,
    downloadUrl: (version: string) => version,
  } as const;
  const source = [
    "include = [",
    '    "vendor/mermaid/11.17.2/mermaid.min.js.br",',
    '    "vendor/mermaid/11.17.2/mermaid.min.js.sha256",',
    "]",
  ].join("\n");

  const updated = new RuntimeSourceUpdater().replacePackageIncludeVersion(
    source,
    mermaid,
    "11.18.0",
  );

  expect(updated).toContain('"vendor/mermaid/11.18.0/mermaid.min.js.br",');
  expect(updated).toContain('"vendor/mermaid/11.18.0/mermaid.min.js.sha256",');
  expect(updated).not.toContain("11.17.2");
});
