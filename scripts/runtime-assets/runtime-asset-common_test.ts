import { expect, test } from "bun:test";
import {
  RuntimeAssetCatalog,
  RuntimeAssetChecksum,
  RuntimeAssetPaths,
} from "./runtime-asset-common";

test("runtime asset path は version を含む", () => {
  const mermaid = RuntimeAssetCatalog.byKind("mermaid");
  const drawio = RuntimeAssetCatalog.byKind("drawio");

  expect(RuntimeAssetPaths.assetFile(mermaid)).toBe(
    "crates/katana-canvas-forge/vendor/mermaid/3.3.1/mermaid.min.js",
  );
  expect(RuntimeAssetPaths.assetFile(drawio)).toBe(
    "crates/katana-canvas-forge/vendor/drawio/29.7.10/drawio.min.js",
  );
});

test("runtime asset checksum は固定ファイルと一致する", () => {
  const mermaid = RuntimeAssetCatalog.byKind("mermaid");
  const drawio = RuntimeAssetCatalog.byKind("drawio");

  expect(RuntimeAssetChecksum.digestFile(RuntimeAssetPaths.assetFile(mermaid))).toBe(
    "217b66ef4279c33c141b4afe22effad10a91c02558dc70917be2c0981e78ed87",
  );
  expect(RuntimeAssetChecksum.digestFile(RuntimeAssetPaths.assetFile(drawio))).toBe(
    "a8b7897de995a4e7dd3a541a5e7250d64a295440f728f0ddae72179cdf5a83d5",
  );
});
