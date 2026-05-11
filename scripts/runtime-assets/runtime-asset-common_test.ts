import { expect, test } from "bun:test";
import {
  RuntimeAssetCatalog,
  RuntimeAssetCatalogSource,
  RuntimeAssetChecksum,
  RuntimeAssetPaths,
} from "./runtime-asset-common";

test("runtime asset path は version を含む", () => {
  const mermaid = RuntimeAssetCatalog.byKind("mermaid");
  const zenuml = RuntimeAssetCatalog.byKind("mermaid-zenuml");
  const drawio = RuntimeAssetCatalog.byKind("drawio");

  expect(RuntimeAssetPaths.assetFile(mermaid)).toBe(
    "crates/katana-canvas-forge/vendor/mermaid/3.3.1/mermaid.min.js",
  );
  expect(RuntimeAssetPaths.assetFile(zenuml)).toBe(
    "crates/katana-canvas-forge/vendor/mermaid-zenuml/0.2.2/mermaid-zenuml.min.js",
  );
  expect(RuntimeAssetPaths.assetFile(drawio)).toBe(
    "crates/katana-canvas-forge/vendor/drawio/29.7.10/drawio.min.js",
  );
  expect(RuntimeAssetPaths.justVersionVariable(zenuml)).toBe("MERMAID_ZENUML_JS_VERSION");
});

test("runtime asset checksum は固定ファイルと一致する", () => {
  const mermaid = RuntimeAssetCatalog.byKind("mermaid");
  const zenuml = RuntimeAssetCatalog.byKind("mermaid-zenuml");
  const drawio = RuntimeAssetCatalog.byKind("drawio");

  expect(RuntimeAssetChecksum.digestFile(RuntimeAssetPaths.assetFile(mermaid))).toBe(
    mermaid.checksum,
  );
  expect(RuntimeAssetChecksum.digestFile(RuntimeAssetPaths.assetFile(zenuml))).toBe(
    zenuml.checksum,
  );
  expect(RuntimeAssetChecksum.digestFile(RuntimeAssetPaths.assetFile(drawio))).toBe(
    drawio.checksum,
  );
});

test("runtime asset catalog source は対象 asset の固定値だけ更新する", () => {
  const source = [
    "const DEFINITIONS: RuntimeAssetDefinition[] = [",
    "  {",
    '    kind: "mermaid",',
    '    version: "3.3.1",',
    '    checksum: "old-mermaid-checksum",',
    "  },",
    "  {",
    '    kind: "drawio",',
    '    version: "29.7.10",',
    '    checksum: "old-drawio-checksum",',
    "  },",
    "];",
  ].join("\n");

  const updated = RuntimeAssetCatalogSource.updatePinnedAsset(
    source,
    "mermaid",
    "4.0.0",
    "new-mermaid-checksum",
  );

  expect(updated).toContain('kind: "mermaid",\n    version: "4.0.0",');
  expect(updated).toContain('checksum: "new-mermaid-checksum",');
  expect(updated).toContain('kind: "drawio",\n    version: "29.7.10",');
  expect(updated).toContain('checksum: "old-drawio-checksum",');
});
