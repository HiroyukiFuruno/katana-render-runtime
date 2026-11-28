import path from "node:path";
import { fileSha256 } from "./mermaid-pair-manifest";
import { type MagickOps, PaintProjectedRasterDimensionsError } from "./reference_image_ops";
import type { ReferencePair } from "./reference_pair_repository";
import type { PaintProjectedReferenceScore, ReferenceScoreRow } from "./reference_score";
import { ReferenceScorePolicy } from "./reference_score_policy";
import { isNeutralGeometryTargetSlug } from "./reference_svg_geometry";

export interface NativeProductFrame {
  slug: string;
  png: { path: string; sha256: string };
}

export interface NativeProductPaintReference {
  projectedPng: { path: string; sha256: string };
  officialPngSha256: string;
  actualPngSha256: string;
  evidenceSha256: string;
}

type NativeProductActualPngShape = NonNullable<ReferenceScoreRow["actualPngShape"]>;

export interface NativeProductDimensionFailure {
  slug: string;
  reason: "paint-projected-raster-dimensions-differ";
  official: { width: number; height: number };
  actual: { width: number; height: number };
}

export interface NativeProductScoreBatch {
  scores: ReferenceScoreRow[];
  failures: NativeProductDimensionFailure[];
}

export function scoreNativeProductRows(
  pairs: readonly ReferencePair[],
  frames: readonly NativeProductFrame[],
  projectedScores: ReadonlyMap<string, PaintProjectedReferenceScore>,
  projectedReferences: ReadonlyMap<string, NativeProductPaintReference>,
  magick: MagickOps,
  outputDirectory: string,
): NativeProductScoreBatch {
  const framesBySlug = indexFrames(pairs, frames);
  magick.prepare();
  const policy = new ReferenceScorePolicy(99),
    workDirectory = path.join(outputDirectory, "work");
  const scores: ReferenceScoreRow[] = [],
    failures: NativeProductDimensionFailure[] = [];
  for (const pair of pairs) {
    try {
      scores.push(
        scoreNativeProductRow(
          pair,
          framesBySlug.get(pair.slug),
          projectedScores,
          projectedReferences,
          magick,
          workDirectory,
          policy,
        ),
      );
    } catch (error) {
      if (!(error instanceof PaintProjectedRasterDimensionsError)) {
        throw error;
      }
      failures.push({
        slug: error.slug,
        reason: "paint-projected-raster-dimensions-differ",
        official: error.referenceSize,
        actual: error.actualSize,
      });
    }
  }
  return { scores, failures };
}

function indexFrames(
  pairs: readonly ReferencePair[],
  frames: readonly NativeProductFrame[],
): Map<string, NativeProductFrame> {
  const expected = pairs.map((pair) => pair.slug),
    actual = frames.map((frame) => frame.slug),
    indexed = new Map(frames.map((frame) => [frame.slug, frame]));
  if (
    expected.length === 0 ||
    new Set(expected).size !== expected.length ||
    new Set(actual).size !== actual.length ||
    JSON.stringify(expected) !== JSON.stringify(actual)
  ) {
    throw new Error("Native product frame inventory differs from the reference pairs");
  }
  return indexed;
}

function scoreNativeProductRow(
  pair: ReferencePair,
  frame: NativeProductFrame | undefined,
  projectedScores: ReadonlyMap<string, PaintProjectedReferenceScore>,
  projectedReferences: ReadonlyMap<string, NativeProductPaintReference>,
  magick: MagickOps,
  workDirectory: string,
  policy: ReferenceScorePolicy,
): ReferenceScoreRow {
  if (frame === undefined) throw new Error(`Missing native product PNG: ${pair.slug}`);
  const actualPngSha256 = fileSha256(frame.png.path);
  if (actualPngSha256 !== frame.png.sha256) {
    throw new Error(`Native product PNG digest differs from its evidence: ${pair.slug}`);
  }
  const metrics = magick.comparePaintProjectedImages(
      pair.officialPath,
      frame.png.path,
      workDirectory,
      pair.slug,
    ),
    projected = projectedScores.get(pair.slug),
    projectedReference = projectedReferences.get(pair.slug);
  if (isNeutralGeometryTargetSlug(pair.slug) && projected === undefined) {
    throw new Error(`Missing native geometry score: ${pair.slug}`);
  }
  const paintReference = isNeutralGeometryTargetSlug(pair.slug)
    ? assertPaintReference(pair, actualPngSha256, projected, projectedReference)
    : undefined;
  const actualPngShape =
    paintReference === undefined
      ? undefined
      : buildPaintAlignedShape(pair, frame, actualPngSha256, paintReference, magick, workDirectory);
  return buildNativeProductScoreRow(pair.slug, metrics, projected, actualPngShape, policy);
}

function buildPaintAlignedShape(
  pair: ReferencePair,
  frame: NativeProductFrame,
  actualPngSha256: string,
  paintReference: {
    projected: PaintProjectedReferenceScore;
    reference: NativeProductPaintReference;
  },
  magick: MagickOps,
  workDirectory: string,
): NativeProductActualPngShape {
  const comparison = magick.compareRgbChannelEdgeSupportImages(
    paintReference.reference.projectedPng.path,
    frame.png.path,
    workDirectory,
    pair.slug,
  );
  return {
    ...comparison,
    officialPngSha256: fileSha256(pair.officialPath),
    actualPngSha256,
    projectedReferencePngSha256: paintReference.reference.projectedPng.sha256,
    projectionEvidenceSha256: paintReference.projected.evidenceSha256,
    policy: "paint-aligned-rgb-channel-edge-support-v2",
  };
}

function assertPaintReference(
  pair: ReferencePair,
  actualPngSha256: string,
  projected: PaintProjectedReferenceScore | undefined,
  reference: NativeProductPaintReference | undefined,
): { projected: PaintProjectedReferenceScore; reference: NativeProductPaintReference } {
  if (projected === undefined || reference === undefined) {
    throw new Error(`Missing native paint reference pin: ${pair.slug}`);
  }
  if (fileSha256(pair.officialPath) !== reference.officialPngSha256) {
    throw new Error(`Native paint reference official PNG differs from its evidence: ${pair.slug}`);
  }
  if (fileSha256(reference.projectedPng.path) !== reference.projectedPng.sha256) {
    throw new Error(`Native paint reference projected PNG differs from its evidence: ${pair.slug}`);
  }
  if (reference.actualPngSha256 !== actualPngSha256) {
    throw new Error(`Native paint reference actual PNG differs from its evidence: ${pair.slug}`);
  }
  if (reference.evidenceSha256 !== projected.evidenceSha256) {
    throw new Error(`Native paint reference evidence differs from its score: ${pair.slug}`);
  }
  return { projected, reference };
}

function buildNativeProductScoreRow(
  slug: string,
  metrics: ReturnType<MagickOps["comparePaintProjectedImages"]>,
  projected: PaintProjectedReferenceScore | undefined,
  actualPngShape: ReferenceScoreRow["actualPngShape"],
  policy: ReferenceScorePolicy,
): ReferenceScoreRow {
  const geometry = isNeutralGeometryTargetSlug(slug),
    threshold = policy.thresholdFor(slug),
    score = geometry ? Math.min(projected?.score ?? 0, actualPngShape?.score ?? 0) : metrics.score;
  return {
    slug,
    score,
    rgbScore: metrics.score,
    scoreSemantics: geometry
      ? "actual-png-paint-aligned-rgb-channel-edge-support-v2+paint-projected-reference-v1"
      : "rgb-v1",
    canvasRmseScore: metrics.fullCanvasRmseScore,
    canvasMaeScore: metrics.fullCanvasMaeScore,
    contentRmseScore: metrics.contentRmseScore,
    contentMaeScore: metrics.contentMaeScore,
    minScore: threshold.minScore,
    exceptionReason: threshold.reason,
    passed:
      score >= threshold.minScore &&
      (projected?.structuralIntegrityAccepted ?? true) &&
      (actualPngShape === undefined || actualPngShape.score >= threshold.minScore),
    ...(geometry && projected !== undefined ? { paintProjectedReference: projected } : {}),
    ...(geometry && actualPngShape !== undefined ? { actualPngShape } : {}),
  };
}
