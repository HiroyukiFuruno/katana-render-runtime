import path from "node:path";
import { compareImages, type ScoreReport, sha256File } from "./reference-capture";

export class ImageComparisonSession {
  readonly #scores = new Map<string, ScoreReport>();

  compare(referencePath: string, candidatePath: string, threshold: number): ScoreReport {
    const referenceAbsolutePath = path.resolve(referencePath);
    const candidateAbsolutePath = path.resolve(candidatePath);
    const reference = sha256File(referenceAbsolutePath);
    const candidate = sha256File(candidateAbsolutePath);
    const key = [
      referenceAbsolutePath,
      reference,
      candidateAbsolutePath,
      candidate,
      threshold,
    ].join("\0");
    const cached = this.#scores.get(key);
    if (cached) return copyScoreReport(cached);
    const score = compareImages(referenceAbsolutePath, candidateAbsolutePath, threshold);
    this.#scores.set(key, score);
    return copyScoreReport(score);
  }
}

function copyScoreReport(score: ScoreReport): ScoreReport {
  return { ...score, dimensions: { ...score.dimensions } };
}
