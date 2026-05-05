import fs from "node:fs";
import path from "node:path";
import { CropRect, type CliParsedOptions } from "../mermaid/reference_compare_options";
import { MagickOps } from "../mermaid/reference_image_ops";
import { ReferencePairRepository } from "../mermaid/reference_pair_repository";
import { ReferenceCompareReport, ReferenceCompareReportLabels } from "../mermaid/reference_report";
import {
  type DrawioReferenceScoreBaseline,
  DrawioReferenceScorer,
  DrawioReferenceScores,
  type DrawioReferenceScoreRow,
} from "./reference-score";

class CliOptions {
  static parse(argv: string[]): CliParsedOptions {
    return {
      officialDir: path.resolve(
        CliOptions.get(argv, "--official", "tests/fixtures/drawio/basic/official"),
      ),
      katanaDir: path.resolve(CliOptions.get(argv, "--katana", "tmp/drawio-katana-rendered")),
      outputDir: path.resolve(CliOptions.get(argv, "--output", "tmp/drawio-official-comparison")),
      katanaCrop: CropRect.parseOptional(CliOptions.get(argv, "--katana-crop", "none")),
      baselineScoresPath: CliOptions.getOptionalPath(argv, "--baseline"),
      minScore: CliOptions.number(argv, "--min-score", 99),
      theme: "dark",
    };
  }

  private static get(argv: string[], name: string, fallback: string): string {
    const index = argv.indexOf(name);
    return index >= 0 ? argv[index + 1] : fallback;
  }

  private static getOptionalPath(argv: string[], name: string): string | null {
    const index = argv.indexOf(name);
    return index >= 0 ? path.resolve(argv[index + 1]) : null;
  }

  private static number(argv: string[], name: string, fallback: number): number {
    const value = Number(CliOptions.get(argv, name, String(fallback)));
    if (!Number.isFinite(value)) {
      throw new Error(`Invalid number option: ${name}`);
    }
    return value;
  }
}

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
    const scores = new DrawioReferenceScorer(magick, this.options.minScore, baselines).score(
      normalized,
    );
    const contactSheet = magick.renderContactSheet(normalized.map((it) => it.pairImagePath));

    new ReferenceCompareReport(this.options.outputDir, ReferenceCompareReportLabels.drawio()).write(
      pairs,
      scores,
      contactSheet,
    );
    this.printSummary(scores, contactSheet);
    process.exitCode = this.exitCode(scores);
  }

  private printSummary(scores: DrawioReferenceScoreRow[], contactSheet: string) {
    console.log(`pairs: ${scores.length}`);
    console.log(`minimum score: ${DrawioReferenceScores.minimum(scores).toFixed(2)}`);
    console.log(`contact: ${contactSheet}`);
  }

  private exitCode(scores: DrawioReferenceScoreRow[]): number {
    return scores.every((score) => score.passed) ? 0 : 1;
  }

  private loadBaselines(filePath: string): DrawioReferenceScoreBaseline[] {
    const file = fs.readFileSync(filePath, "utf8");
    const parsed = JSON.parse(file) as DrawioReferenceScoreBaseline[];
    if (!Array.isArray(parsed)) {
      throw new Error(`Invalid Draw.io baseline score file: expected array at ${filePath}`);
    }
    return parsed.map((entry) => this.parseBaselineScore(entry, filePath));
  }

  private parseBaselineScore(
    entry: DrawioReferenceScoreBaseline,
    filePath: string,
  ): DrawioReferenceScoreBaseline {
    if (Object.prototype.toString.call(entry) !== "[object Object]") {
      throw new Error(`Invalid Draw.io baseline score entry: not an object in ${filePath}`);
    }
    if (typeof entry.slug !== "string") {
      throw new Error(`Invalid Draw.io baseline score slug in ${filePath}`);
    }
    if (typeof entry.score !== "number") {
      throw new Error(`Invalid Draw.io baseline score value in ${filePath}`);
    }
    return {
      slug: entry.slug,
      score: entry.score,
    };
  }
}

new ReferenceCompare(CliOptions.parse(process.argv.slice(2))).run();
