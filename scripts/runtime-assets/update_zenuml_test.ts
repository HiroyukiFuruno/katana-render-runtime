import { expect, test } from "bun:test";
import { RuntimeAssetCatalog } from "./runtime-asset-common";
import { RuntimeSourceUpdater } from "./update";

test("mermaid-zenuml の vendor include path を更新できる", () => {
  const source = [
    "const MERMAID_ZENUML: &str =",
    '    include_str!("../../../vendor/mermaid-zenuml/0.2.2/mermaid-zenuml.min.js");',
  ].join("\n");

  const updated = new RuntimeSourceUpdater().replaceVendorAssetVersion(
    source,
    RuntimeAssetCatalog.byKind("mermaid-zenuml"),
    "0.2.3",
  );

  expect(updated).toContain("../../../vendor/mermaid-zenuml/0.2.3/mermaid-zenuml.min.js");
});
