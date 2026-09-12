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

test("Draw.io resource archive はpath順で連結しRust indexを生成する", () => {
  const archive = buildDrawioResourceArchive([
    { path: "z/file.svg", bytes: Buffer.from("Z") },
    { path: "a/file.xml", bytes: Buffer.from("ABC") },
  ]);

  expect(brotliDecompressSync(archive.compressedBytes).toString()).toBe("ABCZ");
  expect(archive.indexSources).toHaveLength(2);
  expect(archive.indexSources.map((it) => it.fileName)).toEqual([
    "drawio-resources-a-index.rs",
    "drawio-resources-z-index.rs",
  ]);
  expect(archive.indexSources.at(0)?.source).toContain('("a/file.xml", 0, 3)');
  expect(archive.indexSources.at(1)?.source).toContain('("z/file.svg", 3, 1)');
  expect(archive.indexSource).toContain("UNCOMPRESSED_LENGTH: usize = 4");
  expect(archive.indexSource).toContain("DRAWIO_RESOURCE_ARCHIVE_INDEXES");
});
