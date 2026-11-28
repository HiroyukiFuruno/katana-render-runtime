import type { MermaidSvgStages } from "./official-renderer-types";

export type OfficialReferenceMode = "strictVendorOriginal" | "legacyNormalized";

export function referenceMode(value: string): OfficialReferenceMode {
  if (value === "strictVendorOriginal" || value === "legacyNormalized") return value;
  throw new Error(`Unsupported reference mode: ${value}`);
}

export function assertStrictOriginal(source: string, stages: MermaidSvgStages) {
  if (
    stages.originalSource !== source ||
    stages.preprocessedSource !== source ||
    stages.renderSource !== source ||
    stages.replacements.length !== 0
  ) {
    throw new Error("strictVendorOriginal requires exact canonical Mermaid source bytes as input");
  }
  if (stages.rawSvg !== stages.restoredSvg || stages.rawSvg !== stages.normalizedSvg) {
    throw new Error("strictVendorOriginal prohibits SVG postprocessing");
  }
}

export function assertReferenceScope(mode: OfficialReferenceMode, scope: string) {
  if (scope !== "formal" && scope !== "diagnostic") {
    throw new Error("Unsupported reference scope");
  }
  if (mode === "legacyNormalized" && scope !== "diagnostic") {
    throw new Error("legacyNormalized is diagnostic only and cannot supply formal 99 gold");
  }
}
