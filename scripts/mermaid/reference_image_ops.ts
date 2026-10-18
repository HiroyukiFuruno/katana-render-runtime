import assert from "node:assert/strict";
import { execFileSync, type SpawnSyncReturns, spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { DiagramTheme, type DiagramThemeName } from "./diagram_theme";
import type { CropRect } from "./reference_compare_options";
import type { ReferencePair } from "./reference_pair_repository";

export interface NormalizedPair {
  pair: ReferencePair;
  officialCanvasPath: string;
  katanaCanvasPath: string;
  officialContentPath: string;
  katanaContentPath: string;
  pairImagePath: string;
}

export interface GeometryImageComparison {
  fullCanvasScore: number;
  foregroundUnionScore: number;
  roi: { x: number; y: number; width: number; height: number };
  width: number;
  height: number;
}

export interface PaintProjectedImageComparison {
  fullCanvasRmseScore: number;
  fullCanvasMaeScore: number;
  contentRmseScore: number;
  contentMaeScore: number;
  score: number;
  roi: { x: number; y: number; width: number; height: number };
  width: number;
  height: number;
}

export class MagickOps {
  private workDir: string;
  private theme: DiagramTheme;

  constructor(
    private outputDir: string,
    private crop: CropRect | null,
    themeName: DiagramThemeName,
  ) {
    this.workDir = path.join(outputDir, "work");
    this.theme = DiagramTheme.parse(themeName);
  }

  prepare() {
    fs.rmSync(this.outputDir, { recursive: true, force: true });
    fs.mkdirSync(path.join(this.outputDir, "pairs"), { recursive: true });
    fs.mkdirSync(this.workDir, { recursive: true });
  }

  renderPair(pair: ReferencePair): NormalizedPair {
    const normalized = this.normalizedPair(pair);
    this.normalizeOfficial(pair.officialPath, normalized.officialCanvasPath);
    this.normalizeKatana(pair.katanaPath, normalized.katanaCanvasPath);
    this.normalizeContent(normalized.officialCanvasPath, normalized.officialContentPath);
    this.normalizeContent(normalized.katanaCanvasPath, normalized.katanaContentPath);
    this.magick([
      normalized.officialCanvasPath,
      normalized.katanaCanvasPath,
      "+append",
      normalized.pairImagePath,
    ]);
    return normalized;
  }

  renderContactSheet(pairImages: string[]): string {
    const rows: string[] = [];
    for (let index = 0; index < pairImages.length; index += 2) {
      rows.push(this.renderContactRow(pairImages, index, rows.length));
    }
    const output = path.join(this.outputDir, "contact-sheet.png");
    this.magick([...rows, "-background", this.theme.canvasBackground, "-append", output]);
    return output;
  }

  compareNormalizedError(metric: string, left: string, right: string): number {
    const result = spawnSync("magick", ["compare", "-metric", metric, left, right, "null:"], {
      encoding: "utf8",
    });
    return new ImageMetricResult(result, metric).normalizedError();
  }

  imageSize(input: string): ImageSize {
    const result = execFileSync("magick", ["identify", "-format", "%w %h", input], {
      encoding: "utf8",
    });
    return ImageSize.parse(result);
  }

  compareGeometryImages(
    left: string,
    right: string,
    workDir: string,
    slug: string,
  ): GeometryImageComparison {
    const leftSize = this.imageSize(left),
      rightSize = this.imageSize(right);
    if (leftSize.width !== rightSize.width || leftSize.height !== rightSize.height) {
      throw new Error(`Neutral geometry raster dimensions differ for ${slug}`);
    }
    const bounds = [this.foregroundBounds(left), this.foregroundBounds(right)];
    if (bounds.some((bound) => bound.width < 2 || bound.height < 2)) {
      throw new Error(`Neutral geometry raster has no measurable foreground for ${slug}`);
    }
    const roi = unionBounds(bounds, leftSize);
    const officialCrop = path.join(workDir, `${slug}-official-roi.png`),
      candidateCrop = path.join(workDir, `${slug}-candidate-roi.png`);
    this.cropAt(left, roi, officialCrop);
    this.cropAt(right, roi, candidateCrop);
    return {
      fullCanvasScore: this.scoreMetric("RMSE", left, right),
      foregroundUnionScore: this.scoreMetric("RMSE", officialCrop, candidateCrop),
      roi,
      width: leftSize.width,
      height: leftSize.height,
    };
  }

  comparePaintProjectedImages(
    projectedReference: string,
    candidate: string,
    workDir: string,
    slug: string,
  ): PaintProjectedImageComparison {
    const referenceSize = this.imageSize(projectedReference),
      candidateSize = this.imageSize(candidate);
    if (
      referenceSize.width !== candidateSize.width ||
      referenceSize.height !== candidateSize.height
    ) {
      throw new Error(`Paint projected raster dimensions differ for ${slug}`);
    }
    const bounds = [this.foregroundBounds(projectedReference), this.foregroundBounds(candidate)];
    if (bounds.some((bound) => bound.width < 2 || bound.height < 2)) {
      throw new Error(`Paint projected raster has no measurable foreground for ${slug}`);
    }
    const roi = unionBounds(bounds, referenceSize);
    const referenceCrop = path.join(workDir, `${slug}-projected-reference-roi.png`),
      candidateCrop = path.join(workDir, `${slug}-candidate-source-roi.png`);
    this.cropAt(projectedReference, roi, referenceCrop);
    this.cropAt(candidate, roi, candidateCrop);
    const fullCanvasRmseScore = this.scoreMetric("RMSE", projectedReference, candidate),
      fullCanvasMaeScore = this.scoreMetric("MAE", projectedReference, candidate),
      contentRmseScore = this.scoreMetric("RMSE", referenceCrop, candidateCrop),
      contentMaeScore = this.scoreMetric("MAE", referenceCrop, candidateCrop);
    return {
      fullCanvasRmseScore,
      fullCanvasMaeScore,
      contentRmseScore,
      contentMaeScore,
      score: Math.min(fullCanvasRmseScore, fullCanvasMaeScore, contentRmseScore, contentMaeScore),
      roi,
      width: referenceSize.width,
      height: referenceSize.height,
    };
  }

  private normalizedPair(pair: ReferencePair): NormalizedPair {
    return {
      pair,
      officialCanvasPath: path.join(this.workDir, `${pair.slug}-official.png`),
      katanaCanvasPath: path.join(this.workDir, `${pair.slug}-katana.png`),
      officialContentPath: path.join(this.workDir, `${pair.slug}-official-content.png`),
      katanaContentPath: path.join(this.workDir, `${pair.slug}-katana-content.png`),
      pairImagePath: path.join(this.outputDir, "pairs", `${pair.slug}.png`),
    };
  }

  private normalizeOfficial(input: string, output: string) {
    this.normalizeCanvas(input, [], output);
  }

  private normalizeKatana(input: string, output: string) {
    const cropArgs = this.crop === null ? [] : ["-crop", this.crop.toMagickArg(), "+repage"];
    this.normalizeCanvas(input, cropArgs, output);
  }

  private normalizeCanvas(input: string, preArgs: string[], output: string) {
    this.magick([input, ...preArgs, ...this.canvasArgs(), output]);
  }

  private normalizeContent(input: string, output: string) {
    this.magick([input, "-fuzz", "4%", "-trim", "+repage", ...this.canvasArgs(), output]);
  }

  private scoreMetric(metric: string, left: string, right: string): number {
    return Math.max(0, 100 * (1 - this.compareNormalizedError(metric, left, right)));
  }

  private foregroundBounds(input: string): { x: number; y: number; width: number; height: number } {
    const geometry = execFileSync(
      "magick",
      [input, "-fuzz", "0%", "-trim", "-format", "%wx%h%X%Y", "info:"],
      { encoding: "utf8" },
    );
    return parseTrimGeometry(geometry);
  }

  private cropAt(input: string, roi: GeometryImageComparison["roi"], output: string) {
    this.magick([
      input,
      "-crop",
      `${roi.width}x${roi.height}+${roi.x}+${roi.y}`,
      "+repage",
      output,
    ]);
  }

  private canvasArgs(): string[] {
    return [
      "-resize",
      "760x423",
      "-background",
      this.theme.canvasBackground,
      "-gravity",
      "center",
      "-extent",
      "760x423",
    ];
  }

  private renderContactRow(pairImages: string[], index: number, rowIndex: number): string {
    const row = path.join(this.workDir, `row-${String(rowIndex).padStart(2, "0")}.png`);
    this.magick([
      this.firstImage(pairImages, index),
      this.secondImage(pairImages, index),
      "-background",
      this.theme.canvasBackground,
      "+append",
      row,
    ]);
    return row;
  }

  private secondImage(pairImages: string[], index: number): string {
    return pairImages.at(index + 1) ?? this.firstImage(pairImages, index);
  }

  private firstImage(pairImages: string[], index: number): string {
    const image = pairImages.at(index);
    if (image === undefined) {
      throw new Error(`Contact sheet image is missing at index ${index}`);
    }
    return image;
  }

  private magick(args: string[]) {
    execFileSync("magick", args, { stdio: "inherit" });
  }
}

function parseTrimGeometry(value: string): { x: number; y: number; width: number; height: number } {
  const match = value.trim().match(/^(\d+)x(\d+)([+-]\d+)([+-]\d+)$/);
  if (match === null) throw new Error(`ImageMagick foreground bounds are invalid: ${value}`);
  const [, width, height, x, y] = match;
  const result = { width: Number(width), height: Number(height), x: Number(x), y: Number(y) };
  if (Object.values(result).some((part) => !Number.isSafeInteger(part) || part < 0)) {
    throw new Error(`ImageMagick foreground bounds are outside the image: ${value}`);
  }
  return result;
}

function unionBounds(
  bounds: Array<{ x: number; y: number; width: number; height: number }>,
  image: ImageSize,
): GeometryImageComparison["roi"] {
  const x = Math.min(...bounds.map((bound) => bound.x)),
    y = Math.min(...bounds.map((bound) => bound.y)),
    right = Math.max(...bounds.map((bound) => bound.x + bound.width)),
    bottom = Math.max(...bounds.map((bound) => bound.y + bound.height));
  if (right > image.width || bottom > image.height || right <= x || bottom <= y) {
    throw new Error("Neutral geometry foreground union is invalid");
  }
  return { x, y, width: right - x, height: bottom - y };
}

export function prepareReferenceImageOps(options: {
  outputDir: string;
  katanaCrop: CropRect | null;
  theme: DiagramThemeName;
}): MagickOps {
  const magick = new MagickOps(options.outputDir, options.katanaCrop, options.theme);
  magick.prepare();
  return magick;
}

export function renderAndScoreReferencePairs<Baseline, Score>(
  pairs: ReferencePair[],
  magick: MagickOps,
  loadBaselines: () => Baseline[],
  score: (normalized: NormalizedPair[], baselines: Baseline[]) => Score,
): { normalized: NormalizedPair[]; scores: Score } {
  const normalized = pairs.map((pair) => magick.renderPair(pair));
  return { normalized, scores: score(normalized, loadBaselines()) };
}

export class ImageSize {
  static parse(value: string): ImageSize {
    const [width, height] = value.trim().split(/\s+/).map(Number);
    assert(Number.isFinite(width), `ImageMagick identify parse failed: ${value.trim()}`);
    assert(Number.isFinite(height), `ImageMagick identify parse failed: ${value.trim()}`);
    return new ImageSize(ImageSize.partAt(width), ImageSize.partAt(height));
  }

  private static partAt(value: number | undefined): number {
    if (value === undefined) {
      throw new Error("ImageMagick identify result is incomplete");
    }
    return value;
  }

  constructor(
    public readonly width: number,
    public readonly height: number,
  ) {}

  similarityTo(other: ImageSize): number {
    return Math.min(
      ImageSize.axisSimilarity(this.width, other.width),
      ImageSize.axisSimilarity(this.height, other.height),
    );
  }

  coverageBy(candidate: ImageSize): number {
    return Math.min(
      ImageSize.coverageAxisSimilarity(this.width, candidate.width),
      ImageSize.coverageAxisSimilarity(this.height, candidate.height),
    );
  }

  private static axisSimilarity(left: number, right: number): number {
    return Math.min(left, right) / Math.max(left, right);
  }

  private static coverageAxisSimilarity(expected: number, actual: number): number {
    return Math.min(1, actual / expected);
  }
}

const ImageMetric = {
  parseNormalizedError(value: string): number {
    const match = value.match(/\(([-+]?\d*\.?\d+(?:e[-+]?\d+)?)\)/i);
    if (!match) {
      throw new Error(`ImageMagick metric parse failed: ${value.trim()}`);
    }
    return Number(ImageMetric.matchedValue(match));
  },

  matchedValue(match: RegExpMatchArray): string {
    const value = match.at(1);
    if (value === undefined) {
      throw new Error("ImageMagick metric result is incomplete");
    }
    return value;
  },
};

class ImageMetricResult {
  constructor(
    private result: SpawnSyncReturns<string>,
    private metric: string,
  ) {}

  normalizedError(): number {
    this.throwIfFailed();
    return ImageMetric.parseNormalizedError(this.metricOutput());
  }

  private throwIfFailed() {
    if (!this.acceptedStatus()) {
      throw new Error(this.failureMessage());
    }
  }

  private acceptedStatus(): boolean {
    return [0, 1].includes(Number(this.result.status));
  }

  private failureMessage(): string {
    return (
      [this.result.stderr.trim(), `ImageMagick compare failed: ${this.metric}`].find(
        (it) => it.length > 0,
      ) ?? `ImageMagick compare failed: ${this.metric}`
    );
  }

  private metricOutput(): string {
    return [this.result.stderr, this.result.stdout].find((it) => it.length > 0) ?? "";
  }
}
