import path from "node:path";
import { CliOptions, type CliParsedOptions, CropRect } from "../mermaid/reference_compare_options";
import { MagickOps } from "../mermaid/reference_image_ops";
import { ReferencePairRepository } from "../mermaid/reference_pair_repository";
import { ReferenceCompareReport, ReferenceCompareReportLabels } from "../mermaid/reference_report";
import { ReferenceScorer, ReferenceScores } from "../mermaid/reference_score";

const PlantUmlCliOptions = {
  parse(argv: string[]): CliParsedOptions {
    return {
      officialDir: path.resolve(
        CliOptions.get(argv, "--official", "tests/fixtures/plantuml/official/official-dark"),
      ),
      katanaDir: path.resolve(
        CliOptions.get(argv, "--katana", "tmp/krr-plantuml/rendered-browser"),
      ),
      outputDir: path.resolve(CliOptions.get(argv, "--output", "tmp/krr-plantuml/comparison")),
      katanaCrop: CropRect.parseOptional(CliOptions.get(argv, "--katana-crop", "none")),
      baselineScoresPath: null,
      minScore: CliOptions.number(argv, "--min-score", 100),
      theme: "dark",
    };
  },
};

class PlantUmlReferenceCompare {
  constructor(private options: CliParsedOptions) {}

  run() {
    const pairs = new ReferencePairRepository(this.options).list();
    if (pairs.length === 0) {
      throw new Error("PlantUML reference pairs are empty");
    }
    const magick = new MagickOps(this.options.outputDir, this.options.katanaCrop, "dark");
    magick.prepare();

    const normalized = pairs.map((pair) => magick.renderPair(pair));
    const scores = new ReferenceScorer(magick, this.options.minScore).score(normalized);
    const contactSheet = magick.renderContactSheet(normalized.map((it) => it.pairImagePath));

    new ReferenceCompareReport(
      this.options.outputDir,
      ReferenceCompareReportLabels.plantuml(),
    ).write(pairs, scores, contactSheet);
    this.printSummary(scores, contactSheet);
    process.exitCode = scores.every((score) => score.passed) ? 0 : 1;
  }

  private printSummary(scores: ReturnType<ReferenceScorer["score"]>, contactSheet: string) {
    console.log(`pairs: ${scores.length}`);
    console.log(`minimum score: ${ReferenceScores.minimum(scores).toFixed(2)}`);
    console.log(`contact: ${contactSheet}`);
  }
}

new PlantUmlReferenceCompare(PlantUmlCliOptions.parse(process.argv.slice(2))).run();
