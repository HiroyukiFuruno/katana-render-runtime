import { expect, test } from "bun:test";
import {
  buildRawMetricsDiagnosticReceipt,
  isRawSvgUsableForDiagnosis,
  parseRawMetricsDiagnosticArgs,
  resolveManifestStagePath,
} from "./official_raw_metrics_diagnostic";

test("raw metrics diagnostics keep text geometry separate from vendor bbox errors", async () => {
  const source = await Bun.file(
    `${process.cwd()}/scripts/mermaid/official_raw_metrics_diagnostic.ts`,
  ).text();
  expect(source).toContain("textGeometryError");
  expect(source).toContain("getStartPositionOfChar(0)");
  expect(source).toContain("baseVal");
  expect(source).toContain("valueInSpecifiedUnits");
  expect(source).toContain("unitType");
  expect(source).toContain('"empty text"');
  expect(source).toContain("originalGetBBox.apply(element, args)");
});

test("raw metrics diagnostics require an exact raw SVG digest match", () => {
  expect(isRawSvgUsableForDiagnosis("a".repeat(64), "a".repeat(64))).toBe(true);
  expect(isRawSvgUsableForDiagnosis("a".repeat(64), "b".repeat(64))).toBe(false);
});

test("raw metrics diagnostic receipt is permanently non-accepted", () => {
  const receipt = buildRawMetricsDiagnosticReceipt({
    slug: "08-01-c4-context-simple",
    sourceManifestSha256: "a".repeat(64),
    canonicalSourceSha256: "b".repeat(64),
    expectedPrimarySvgSha256: "c".repeat(64),
    actualRawSvgSha256: "c".repeat(64),
    bboxCallCount: 7,
    canvasMeasureTextCallCount: 3,
  });
  expect(receipt.diagnosticOnly).toBe(true);
  expect(receipt.accepted).toBe(false);
  expect(receipt.usableForDiagnosis).toBe(true);
  expect(receipt.bboxCallCount).toBe(7);
  expect(receipt.canvasMeasureTextCallCount).toBe(3);
});

test("raw metrics diagnostic CLI accepts only one manifest and output directory", () => {
  expect(
    parseRawMetricsDiagnosticArgs(["--manifest", "/tmp/manifest.json", "--output-dir", "/tmp/out"]),
  ).toEqual({ manifestPath: "/tmp/manifest.json", outputDir: "/tmp/out" });
  expect(() => parseRawMetricsDiagnosticArgs(["--manifest", "/tmp/manifest.json"])).toThrow(
    "Usage:",
  );
});

test("raw stage paths resolve relative to the provenance manifest and reject escapes", () => {
  const manifest = `${process.cwd()}/scripts/mermaid/official_raw_metrics_diagnostic.ts`;
  expect(resolveManifestStagePath(manifest, "official_raw_metrics_diagnostic.ts")).toBe(manifest);
  expect(() => resolveManifestStagePath(manifest, "../official_raw_metrics_diagnostic.ts")).toThrow(
    "escapes provenance manifest directory",
  );
});
