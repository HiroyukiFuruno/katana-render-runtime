import { expect, test } from "bun:test";
import type { JsonValue } from "./mermaid-pair-manifest";
import { validateStrictOfficialSourceProvenance } from "./reference_pair_repository";

const source = "flowchart LR\nA --> B\n";
const sourceHash = "a".repeat(64);
const svgHash = "b".repeat(64);
const expected = {
  originalFenceSourceSha256: "c".repeat(64),
  canonicalSourceSha256: sourceHash,
  originalFenceLanguage: "mermaid",
  sourceAdapterPolicy: "mermaid-fence-lines-v1",
  sourceMarkdownSha256: "d".repeat(64),
  referenceScope: "formal" as const,
};

test("strict reader accepts raw stage bytes bound to source SVG and strict source context", () => {
  expect(() =>
    validateStrictOfficialSourceProvenance({
      slug: "flowchart-example",
      document: strictDocument() as JsonValue,
      sourceSvgSha256: svgHash,
      fenceSource: "flowchart LR\nA --> B",
      canonicalSource: source,
      expected,
    }),
  ).not.toThrow();
});

test("strict reader rejects a stage hash mismatch, runtime script, or non-raw primary", () => {
  const mismatchedRestore = strictDocument();
  mismatchedRestore.stages.restored.sha256 = "e".repeat(64);
  const runtimeScript = strictDocument();
  runtimeScript.rendererAssets.runtimeScripts = [{ path: "runtime.js" }];
  const normalizedPrimary = { ...strictDocument(), primaryStage: "normalized" };
  for (const document of [mismatchedRestore, runtimeScript, normalizedPrimary]) {
    expect(() =>
      validateStrictOfficialSourceProvenance({
        slug: "flowchart-example",
        document: document as JsonValue,
        sourceSvgSha256: svgHash,
        fenceSource: "flowchart LR\nA --> B",
        canonicalSource: source,
        expected,
      }),
    ).toThrow("stage provenance is inconsistent");
  }
});

test("strict reader rejects stale canonical source claims", () => {
  const document = strictDocument();
  document.sources.renderInput = "flowchart LR\nA --> C\n";
  expect(() =>
    validateStrictOfficialSourceProvenance({
      slug: "flowchart-example",
      document: document as JsonValue,
      sourceSvgSha256: svgHash,
      fenceSource: "flowchart LR\nA --> B",
      canonicalSource: source,
      expected,
    }),
  ).toThrow("source provenance does not match");
});

function strictDocument() {
  return {
    schema: "katana-mermaid-render-provenance-v1",
    diagnosticOnly: true,
    accepted: false,
    referenceMode: "strictVendorOriginal",
    referenceScope: "formal",
    primaryStage: "rawMermaid",
    primarySvgSha256: svgHash,
    fixture: {
      capturedFenceSource: "flowchart LR\nA --> B",
      capturedFenceSourceSha256: expected.originalFenceSourceSha256,
      originalSource: source,
      canonicalSource: source,
      canonicalSourceSha256: sourceHash,
      sourceAdapterPolicy: expected.sourceAdapterPolicy,
      fenceLanguage: "mermaid",
      markdownSha256: expected.sourceMarkdownSha256,
    },
    sources: {
      preprocessed: source,
      preprocessedSha256: sourceHash,
      renderInput: source,
      renderInputSha256: sourceHash,
    },
    rendererAssets: { runtimeScripts: [] as Array<{ path: string }> },
    stages: {
      rawMermaid: { sha256: svgHash },
      restored: { sha256: svgHash },
      normalized: { sha256: svgHash },
    },
  };
}
