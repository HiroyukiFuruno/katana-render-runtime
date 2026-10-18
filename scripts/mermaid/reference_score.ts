import type { MagickOps, NormalizedPair } from "./reference_image_ops";
import { type ReferenceScoreBaseline, ReferenceScorePolicy } from "./reference_score_policy";
import { isNeutralGeometryTargetSlug } from "./reference_svg_geometry";

export interface PaintProjectedReferenceScore {
  policy: "raw-svg-geometry-candidate-computed-paint-v1";
  score: number;
  structuralIntegrityAccepted: boolean;
  reasons: string[];
  maxTransformDelta: number;
  fullCanvasRmseScore: number;
  fullCanvasMaeScore: number;
  contentRmseScore: number;
  contentMaeScore: number;
  width: number;
  height: number;
  evidenceSha256: string;
}

export interface ReferenceScoreRow {
  slug: string;
  score: number;
  rgbScore: number;
  scoreSemantics: "rgb-v1" | "paint-projected-reference-v1";
  canvasRmseScore: number;
  canvasMaeScore: number;
  contentRmseScore: number;
  contentMaeScore: number;
  minScore: number;
  exceptionReason: string;
  passed: boolean;
  paintProjectedReference?: PaintProjectedReferenceScore;
}

export const ReferenceScores = {
  minimum(scores: ReferenceScoreRow[]): number {
    return Math.min(...scores.map((it) => it.score));
  },
};

export class ReferenceScorer {
  private policy: ReferenceScorePolicy;

  constructor(
    private magick: MagickOps,
    private minScore: number,
    private baselines: ReferenceScoreBaseline[] = [],
  ) {
    this.policy = new ReferenceScorePolicy(this.minScore, undefined, this.baselines);
  }

  score(
    normalized: NormalizedPair[],
    paintProjectedReference: Map<string, PaintProjectedReferenceScore> = new Map(),
    requirePaintProjectedReference = false,
  ): ReferenceScoreRow[] {
    return normalized.map((pair) =>
      this.scorePair(
        pair,
        paintProjectedReference.get(pair.pair.slug),
        requirePaintProjectedReference,
      ),
    );
  }

  private scorePair(
    normalized: NormalizedPair,
    projectedReference: PaintProjectedReferenceScore | undefined,
    requirePaintProjectedReference: boolean,
  ): ReferenceScoreRow {
    const scores = this.metricScores(normalized);
    const rgbScore = this.aggregateScores(scores);
    if (
      requirePaintProjectedReference &&
      isNeutralGeometryTargetSlug(normalized.pair.slug) &&
      projectedReference === undefined
    ) {
      throw new Error(`Paint-projected reference evidence is required for ${normalized.pair.slug}`);
    }
    const score = projectedReference?.score ?? rgbScore;
    const threshold = this.policy.thresholdFor(normalized.pair.slug);
    return {
      slug: normalized.pair.slug,
      score,
      rgbScore,
      scoreSemantics: projectedReference === undefined ? "rgb-v1" : "paint-projected-reference-v1",
      canvasRmseScore: ReferenceScorer.scoreAt(scores, 0),
      canvasMaeScore: ReferenceScorer.scoreAt(scores, 1),
      contentRmseScore: ReferenceScorer.scoreAt(scores, 2),
      contentMaeScore: ReferenceScorer.scoreAt(scores, 3),
      minScore: threshold.minScore,
      exceptionReason: threshold.reason,
      passed:
        score >= threshold.minScore && (projectedReference?.structuralIntegrityAccepted ?? true),
      ...(projectedReference === undefined ? {} : { paintProjectedReference: projectedReference }),
    };
  }

  private metricScores(normalized: NormalizedPair): number[] {
    return [
      this.scoreMetric("RMSE", normalized.officialCanvasPath, normalized.katanaCanvasPath),
      this.scoreMetric("MAE", normalized.officialCanvasPath, normalized.katanaCanvasPath),
      this.scoreMetric("RMSE", normalized.officialContentPath, normalized.katanaContentPath),
      this.scoreMetric("MAE", normalized.officialContentPath, normalized.katanaContentPath),
    ];
  }

  private aggregateScores(scores: number[]): number {
    return Math.min(...scores);
  }

  private scoreMetric(metric: string, left: string, right: string): number {
    return Math.max(0, 100 * (1 - this.magick.compareNormalizedError(metric, left, right)));
  }

  private static scoreAt(scores: number[], index: number): number {
    const score = scores.at(index);
    if (score === undefined) {
      throw new Error("Reference score metric is missing");
    }
    return score;
  }
}
