import type { RuntimeFragmentTransform } from "#shared/runtime_bundle";

// Biome表示列と混同せず、libraryの1-based行・0-based UTF-16列を保持する。
export interface RuntimeSourcePosition {
  readonly line: number;
  readonly column: number;
}

// Acorn/MagicStringの同じUTF-16区間を各map layerに残す。
export interface RuntimeSourceRange {
  readonly start: number;
  readonly end: number;
  readonly startPosition: RuntimeSourcePosition;
  readonly endPosition: RuntimeSourcePosition;
}

export interface RuntimeFragmentInput {
  readonly occurrence: number;
  readonly sourcePath: string;
  readonly sourceSha256: string;
  readonly label: string;
  readonly transform: RuntimeFragmentTransform;
  readonly rawLength: number;
  readonly transformRemovals: readonly RuntimeSourceRange[];
}

export interface RuntimeGeneratedSpan extends RuntimeSourceRange {
  readonly kind: "label" | "separator";
}

export interface RuntimeEntrypointInput {
  readonly kind: "entrypoint";
  readonly definition: string;
  readonly source: string;
  readonly sourceSha256: string;
  // factoryの文字列値をTSのraw位置へ偽って帰属させない。
  readonly rawSourcePath: null;
  readonly assembledRange: RuntimeSourceRange;
}

export interface RuntimeNormalizationProvenance {
  readonly code: string;
  readonly map: string;
  // assembled側の削除区間。置換改行のmap anchorはraw証拠に使わない。
  readonly removedRanges: readonly RuntimeSourceRange[];
}

export interface RuntimeDefinitionProvenance {
  readonly code: string;
  readonly assemblyMap: string;
  readonly normalizationMap: string;
  readonly rawInputs: readonly RuntimeFragmentInput[];
  readonly entrypoint: RuntimeEntrypointInput;
  readonly generatedSpans: readonly RuntimeGeneratedSpan[];
  readonly duplicateRemovals: readonly RuntimeSourceRange[];
}
