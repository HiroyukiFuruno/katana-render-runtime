import path from "node:path";
import { fileSha256 } from "./mermaid-pair-manifest";
import type { MagickOps } from "./reference_image_ops";
import type { ReferencePair } from "./reference_pair_repository";
import type { PaintProjectedReferenceScore, ReferenceScoreRow } from "./reference_score";
import { ReferenceScorePolicy } from "./reference_score_policy";
import { isNeutralGeometryTargetSlug } from "./reference_svg_geometry";

export interface NativeProductFrame {
  slug: string;
  png: { path: string; sha256: string };
}

export function scoreNativeProductRows(
  pairs: readonly ReferencePair[],
  frames: readonly NativeProductFrame[],
  projectedScores: ReadonlyMap<string, PaintProjectedReferenceScore>,
  magick: MagickOps,
  outputDirectory: string,
): ReferenceScoreRow[] {
  const framesBySlug = indexFrames(pairs, frames);
  magick.prepare();
  const policy = new ReferenceScorePolicy(99),
    workDirectory = path.join(outputDirectory, "work");
  return pairs.map((pair) =>
    scoreNativeProductRow(
      pair,
      framesBySlug.get(pair.slug),
      projectedScores,
      magick,
      workDirectory,
      policy,
    ),
  );
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
    projected = projectedScores.get(pair.slug);
  if (isNeutralGeometryTargetSlug(pair.slug) && projected === undefined) {
    throw new Error(`Missing native geometry score: ${pair.slug}`);
  }
  const actualPngShape = isNeutralGeometryTargetSlug(pair.slug)
    ? magick.compareRgbChannelEdgeSupportImages(
        pair.officialPath,
        frame.png.path,
        workDirectory,
        pair.slug,
      )
    : undefined;
  return buildNativeProductScoreRow(
    pair.slug,
    metrics,
    projected,
    actualPngShape === undefined
      ? undefined
      : {
          ...actualPngShape,
          officialPngSha256: fileSha256(pair.officialPath),
          actualPngSha256,
        },
    policy,
  );
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
      ? "actual-png-rgb-channel-edge-support-v1+paint-projected-reference-v1"
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
