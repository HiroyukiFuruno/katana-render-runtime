import type { MagickOps, NormalizedPair } from "./reference_image_ops";
import { type ReferenceScoreBaseline, ReferenceScorePolicy } from "./reference_score_policy";

export interface ReferenceScoreRow {
  slug: string;
  score: number;
  canvasRmseScore: number;
  canvasMaeScore: number;
  contentRmseScore: number;
  contentMaeScore: number;
  minScore: number;
  exceptionReason: string;
  passed: boolean;
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

  score(normalized: NormalizedPair[]): ReferenceScoreRow[] {
    return normalized.map((pair) => this.scorePair(pair));
  }

  private scorePair(normalized: NormalizedPair): ReferenceScoreRow {
    const scores = this.metricScores(normalized);
    const score = this.aggregateScores(scores);
    const threshold = this.policy.thresholdFor(normalized.pair.slug);
    return {
      slug: normalized.pair.slug,
      score,
      canvasRmseScore: ReferenceScorer.scoreAt(scores, 0),
      canvasMaeScore: ReferenceScorer.scoreAt(scores, 1),
      contentRmseScore: ReferenceScorer.scoreAt(scores, 2),
      contentMaeScore: ReferenceScorer.scoreAt(scores, 3),
      minScore: threshold.minScore,
      exceptionReason: threshold.reason,
      passed: score >= threshold.minScore,
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
