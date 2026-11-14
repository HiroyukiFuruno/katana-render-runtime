import { expect, test } from "bun:test";
import {
  assertReferenceScope,
  assertStrictOriginal,
  referenceMode,
} from "./official-reference-mode";
import type { MermaidSvgStages } from "./official-renderer-types";

const source = "erDiagram\n  動物 ||--o{ 魚 : もつ\n";
const raw = '<svg xmlns="http://www.w3.org/2000/svg"></svg>';

function stages(): MermaidSvgStages {
  return {
    diagramType: "erDiagram",
    rawSvg: raw,
    restoredSvg: raw,
    normalizedSvg: raw,
    originalSource: source,
    preprocessedSource: source,
    renderSource: source,
    replacements: [],
    mermaidVersion: "12.1.0",
  };
}

test("strict preserves every source byte including nonASCII and trailing newline", () => {
  expect(() => assertStrictOriginal(source, stages())).not.toThrow();
});

test("strict rejects i18n placeholder source even when labels are restored later", () => {
  const value = stages();
  value.renderSource = "erDiagram\n KI000 ||--o{ KI001 : KI002";
  value.replacements = [{ placeholder: "KI000", text: "動物" }];
  expect(() => assertStrictOriginal(source, value)).toThrow("exact canonical Mermaid source bytes");
});

test("strict rejects hidden changes to canonical input", () => {
  for (const changed of [source.trim(), `${source}todayMarker off`, `zenuml\n${source}`]) {
    const value = stages();
    value.preprocessedSource = changed;
    value.renderSource = changed;
    expect(() => assertStrictOriginal(source, value)).toThrow(
      "exact canonical Mermaid source bytes",
    );
  }
});

test("strict preserves declared canonical ZenUML input without further changes", () => {
  const canonicalSource = "zenuml\ntitle Order Service\nUser -> Service: request\n";
  const value = stages();
  value.diagramType = "zenuml";
  value.originalSource = canonicalSource;
  value.preprocessedSource = canonicalSource;
  value.renderSource = canonicalSource;
  expect(() => assertStrictOriginal(canonicalSource, value)).not.toThrow();
});

test("strict rejects source restoration or layout postprocessing", () => {
  for (const field of ["restoredSvg", "normalizedSvg"] as const) {
    const value = stages();
    value[field] = raw.replace("</svg>", '<g transform="translate(1,2)"/></svg>');
    expect(() => assertStrictOriginal(source, value)).toThrow("SVG postprocessing");
  }
});

test("legacy reference can be diagnosed but never accepted as formal gold", () => {
  expect(() => assertReferenceScope("legacyNormalized", "diagnostic")).not.toThrow();
  expect(() => assertReferenceScope("legacyNormalized", "formal")).toThrow("diagnostic only");
  expect(() => assertReferenceScope("strictVendorOriginal", "formal")).not.toThrow();
  expect(() => referenceMode("raw")).toThrow("Unsupported reference mode");
  expect(() => assertReferenceScope("strictVendorOriginal", "other")).toThrow("Unsupported");
});
