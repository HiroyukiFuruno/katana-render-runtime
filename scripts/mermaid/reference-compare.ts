import fs from "node:fs";
import { CliOptions, type CliParsedOptions } from "./reference_compare_options";
import { MagickOps } from "./reference_image_ops";
import { ReferencePairRepository } from "./reference_pair_repository";
import { ReferenceCompareReport } from "./reference_report";
import { type ReferenceScoreRow, ReferenceScorer, ReferenceScores } from "./reference_score";
import type { ReferenceScoreBaseline } from "./reference_score_policy";

class ReferenceCompare {
  constructor(private options: CliParsedOptions) {}

  run() {
    const pairs = new ReferencePairRepository(this.options).list();
    const magick = new MagickOps(
      this.options.outputDir,
      this.options.katanaCrop,
      this.options.theme,
    );
    magick.prepare();

    const normalized = pairs.map((pair) => magick.renderPair(pair));
    const baselines = this.options.baselineScoresPath
      ? this.loadBaselines(this.options.baselineScoresPath)
      : [];
    const scores = new ReferenceScorer(magick, this.options.minScore, baselines).score(normalized);
    const contactSheet = magick.renderContactSheet(normalized.map((it) => it.pairImagePath));

    new ReferenceCompareReport(this.options.outputDir).write(pairs, scores, contactSheet);
    this.printSummary(scores, contactSheet);
    process.exitCode = this.exitCode(scores);
  }

  private printSummary(scores: ReferenceScoreRow[], contactSheet: string) {
    console.log(`pairs: ${scores.length}`);
    console.log(`minimum score: ${ReferenceScores.minimum(scores).toFixed(2)}`);
    console.log(`contact: ${contactSheet}`);
  }

  private exitCode(scores: ReferenceScoreRow[]): number {
    return scores.every((score) => score.passed) ? 0 : 1;
  }

  private loadBaselines(filePath: string): ReferenceScoreBaseline[] {
    const file = fs.readFileSync(filePath, "utf8");
    const parsed = JSON.parse(file);
    if (!Array.isArray(parsed)) {
      throw new Error(`Invalid baseline score file: expected array at ${filePath}`);
    }
    return parsed.map((entry) => this.parseBaselineScore(BaselineScoreEntry.from(entry), filePath));
  }

  private parseBaselineScore(entry: BaselineScoreEntry, filePath: string): ReferenceScoreBaseline {
    return {
      slug: this.assertBaselineSlug(entry.slug, filePath),
      score: this.assertBaselineScore(entry.score, filePath),
    };
  }

  private assertBaselineSlug(slug: string, filePath: string): string {
    if (typeof slug !== "string") {
      throw new Error(`Invalid baseline score entry in ${filePath}: ${JSON.stringify(slug)}`);
    }
    return slug;
  }

  private assertBaselineScore(score: number, filePath: string): number {
    if (typeof score !== "number") {
      throw new Error(`Invalid baseline score entry in ${filePath}: ${JSON.stringify(score)}`);
    }
    return score;
  }
}

class BaselineScoreEntry {
  constructor(
    readonly slug: string,
    readonly score: number,
  ) {}

  static from(entry: object): BaselineScoreEntry {
    if (!BaselineScoreEntry.isObject(entry)) {
      throw new Error("Invalid baseline score entry: not an object");
    }
    return new BaselineScoreEntry(
      BaselineScoreEntry.stringValue(entry, "slug"),
      BaselineScoreEntry.numberValue(entry, "score"),
    );
  }

  private static isObject(entry: object): boolean {
    return Object.prototype.toString.call(entry) === "[object Object]";
  }

  private static stringValue(entry: object, key: string): string {
    const value = Reflect.get(entry, key);
    if (typeof value !== "string") {
      throw new Error(`Invalid baseline score string: ${key}`);
    }
    return value;
  }

  private static numberValue(entry: object, key: string): number {
    const value = Reflect.get(entry, key);
    if (typeof value !== "number") {
      throw new Error(`Invalid baseline score number: ${key}`);
    }
    return value;
  }
}

new ReferenceCompare(CliOptions.parse(process.argv.slice(2))).run();
