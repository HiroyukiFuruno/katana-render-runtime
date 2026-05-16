import type { MagickOps, NormalizedPair } from "../mermaid/reference_image_ops";

export interface DrawioReferenceScoreRow {
  slug: string;
  score: number;
  canvasRmseScore: number;
  canvasMaeScore: number;
  contentRmseScore: number;
  contentMaeScore: number;
  contentPhashScore: number;
  rawDimensionScore: number;
  minScore: number;
  exceptionReason: string;
  passed: boolean;
}

export interface DrawioReferenceScoreBaseline {
  slug: string;
  score: number;
}

interface DrawioReferenceScoreThreshold {
  readonly minScore: number;
  readonly reason: string;
}

export const DrawioReferenceScores = {
  minimum(scores: DrawioReferenceScoreRow[]): number {
    return Math.min(...scores.map((it) => it.score));
  },
};

export class DrawioReferenceScorePolicy {
  private baselineBySlug = new Map<string, number>();

  constructor(
    private globalMinScore: number,
    baselines: DrawioReferenceScoreBaseline[] = [],
  ) {
    for (const baseline of baselines) {
      this.baselineBySlug.set(baseline.slug, this.validateScore(baseline));
    }
  }

  thresholdFor(slug: string): DrawioReferenceScoreThreshold {
    const baseline = this.baselineBySlug.get(slug);
    if (baseline === undefined) {
      return {
        minScore: this.globalMinScore,
        reason: "",
      };
    }
    return {
      minScore: Math.max(baseline, this.globalMinScore),
      reason: `既知差分の下限を採用 (${slug})`,
    };
  }

  private validateScore(baseline: DrawioReferenceScoreBaseline): number {
    if (!Number.isFinite(baseline.score)) {
      throw new Error(`Invalid Draw.io baseline score for ${baseline.slug}`);
    }
    return baseline.score;
  }
}

export class DrawioReferenceScorer {
  private policy: DrawioReferenceScorePolicy;

  constructor(
    private magick: MagickOps,
    private minScore: number,
    private baselines: DrawioReferenceScoreBaseline[] = [],
  ) {
    this.policy = new DrawioReferenceScorePolicy(this.minScore, this.baselines);
  }

  score(normalized: NormalizedPair[]): DrawioReferenceScoreRow[] {
    return normalized.map((pair) => this.scorePair(pair));
  }

  private scorePair(normalized: NormalizedPair): DrawioReferenceScoreRow {
    const metrics = this.metricScores(normalized);
    const score = this.visualRecognitionScore(metrics);
    const threshold = this.policy.thresholdFor(normalized.pair.slug);
    return {
      slug: normalized.pair.slug,
      score,
      canvasRmseScore: metrics.canvasRmseScore,
      canvasMaeScore: metrics.canvasMaeScore,
      contentRmseScore: metrics.contentRmseScore,
      contentMaeScore: metrics.contentMaeScore,
      contentPhashScore: metrics.contentPhashScore,
      rawDimensionScore: metrics.rawDimensionScore,
      minScore: threshold.minScore,
      exceptionReason: threshold.reason,
      passed: score >= threshold.minScore,
    };
  }

  private metricScores(normalized: NormalizedPair): DrawioReferenceMetrics {
    return {
      canvasRmseScore: this.scoreMetric(
        "RMSE",
        normalized.officialCanvasPath,
        normalized.katanaCanvasPath,
      ),
      canvasMaeScore: this.scoreMetric(
        "MAE",
        normalized.officialCanvasPath,
        normalized.katanaCanvasPath,
      ),
      contentRmseScore: this.scoreMetric(
        "RMSE",
        normalized.officialContentPath,
        normalized.katanaContentPath,
      ),
      contentMaeScore: this.scoreMetric(
        "MAE",
        normalized.officialContentPath,
        normalized.katanaContentPath,
      ),
      contentPhashScore: this.scoreMetric(
        "PHASH",
        normalized.officialContentPath,
        normalized.katanaContentPath,
      ),
      rawDimensionScore: this.rawDimensionScore(normalized),
    };
  }

  private visualRecognitionScore(metrics: DrawioReferenceMetrics): number {
    return Math.min(
      Math.max(metrics.contentMaeScore, metrics.contentPhashScore),
      this.severeCropScore(metrics),
    );
  }

  private scoreMetric(metric: string, left: string, right: string): number {
    return Math.max(0, 100 * (1 - this.magick.compareNormalizedError(metric, left, right)));
  }

  private rawDimensionScore(normalized: NormalizedPair): number {
    return (
      100 *
      this.magick
        .imageSize(normalized.pair.officialPath)
        .coverageBy(this.magick.imageSize(normalized.pair.katanaPath))
    );
  }

  private severeCropScore(metrics: DrawioReferenceMetrics): number {
    return metrics.rawDimensionScore < 50 ? metrics.rawDimensionScore : 100;
  }
}

interface DrawioReferenceMetrics {
  readonly canvasRmseScore: number;
  readonly canvasMaeScore: number;
  readonly contentRmseScore: number;
  readonly contentMaeScore: number;
  readonly contentPhashScore: number;
  readonly rawDimensionScore: number;
}
