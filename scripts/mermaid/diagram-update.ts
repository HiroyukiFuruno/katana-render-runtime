import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { DiagramTheme, type DiagramThemeName } from "./diagram_theme";
import { type Fixture, FixtureRepository } from "./diagram_update_fixtures";
import { MarkdownReferenceAssets } from "./diagram_update_markdown_assets";
import { MarkdownReferenceWriter } from "./diagram_update_markdown_reference";
import { SkippedFixtureReport } from "./diagram_update_skipped_report";
import { assertReferenceScope, referenceMode } from "./official-reference-mode";
import { OfficialMermaidRenderer, type RendererOptions } from "./official-renderer";
import { resolveOfficialRendererAssetInputs } from "./official-renderer-asset-inputs";
import { RasterProductionSession } from "./raster-production-session";

interface CliParsedOptions extends RendererOptions {
  fixturesDir: string;
  markdownOutputDir: string;
  writeMarkdown: boolean;
  skipErrors: boolean;
}

const CliOptions = {
  parse(argv: string[]): CliParsedOptions {
    CliOptions.exitIfHelp(argv);
    const assetInputs = resolveOfficialRendererAssetInputs(
      argv,
      { ...process.env },
      process.cwd(),
      os.homedir(),
    );
    const fixturesDir = path.resolve(
      CliOptions.get(argv, "--fixtures", "tests/fixtures/mermaid/en"),
    );
    const outputDir = path.resolve(
      CliOptions.get(argv, "--output", path.join(fixturesDir, "official")),
    );
    const mode = referenceMode(CliOptions.get(argv, "--reference-mode", "strictVendorOriginal"));
    const referenceScope = CliOptions.get(argv, "--reference-scope", "formal");
    if (referenceScope !== "formal" && referenceScope !== "diagnostic") {
      throw new Error(`Unsupported reference scope: ${referenceScope}`);
    }
    assertReferenceScope(mode, referenceScope);
    return {
      fixturesDir,
      outputDir,
      markdownOutputDir: path.resolve(CliOptions.get(argv, "--markdown-output", outputDir)),
      ...assetInputs,
      writeMarkdown: mode === "legacyNormalized" && !argv.includes("--no-write-md"),
      skipErrors: argv.includes("--skip-errors"),
      referenceMode: mode,
      referenceScope,
      theme: CliOptions.theme(argv),
      rasterSessionConfigPath: CliOptions.get(argv, "--raster-session-config", ""),
      rasterSessionConfigSha256: CliOptions.get(argv, "--raster-session-config-sha", ""),
    };
  },

  theme(argv: string[]): DiagramThemeName {
    return DiagramTheme.parse(CliOptions.get(argv, "--theme", "dark")).name;
  },

  get(argv: string[], name: string, fallback: string): string {
    const index = argv.indexOf(name);
    return index >= 0 ? (argv.at(index + 1) ?? fallback) : fallback;
  },

  exitIfHelp(argv: string[]) {
    if (argv.includes("--help")) {
      console.log(
        "Usage: bun run scripts/mermaid/diagram-update.ts [--fixtures DIR] [--output DIR] [--markdown-output DIR] [--theme dark|light] [--reference-mode strictVendorOriginal|legacyNormalized] [--reference-scope formal|diagnostic] [--mermaid-js FILE] [--mermaid-zenuml-js FILE]",
      );
      process.exit(0);
    }
  },
};

export class MermaidDiagramUpdate {
  private options: CliParsedOptions;
  private markdownAssets: MarkdownReferenceAssets;
  private skippedReport: SkippedFixtureReport;
  private renderedCount = 0;

  constructor(options: CliParsedOptions) {
    this.options = options;
    this.markdownAssets = new MarkdownReferenceAssets(options.outputDir, options.markdownOutputDir);
    this.skippedReport = new SkippedFixtureReport(options.outputDir);
  }

  async run() {
    this.validate();
    this.prepareOutputDirs();
    const fixtures = new FixtureRepository(this.options.fixturesDir).list();
    if (
      this.options.referenceMode === "strictVendorOriginal" &&
      this.options.rasterSession === undefined
    ) {
      if (!this.options.rasterSessionConfigPath || !this.options.rasterSessionConfigSha256)
        throw new Error(
          "Strict official raster producer requires external session config path and SHA",
        );
      this.options.rasterSession = RasterProductionSession.fromPinnedConfig(
        this.options.rasterSessionConfigPath,
        this.options.rasterSessionConfigSha256,
      );
    }
    const renderer = new OfficialMermaidRenderer(this.options);
    const writer = new MarkdownReferenceWriter(this.options.markdownOutputDir);
    await this.renderAll(renderer, writer, fixtures);
    this.skippedReport.write();
    this.assertRendered(fixtures);
  }

  private prepareOutputDirs() {
    if (
      this.options.referenceMode === "strictVendorOriginal" &&
      fs.existsSync(this.options.outputDir) &&
      fs.readdirSync(this.options.outputDir).length > 0
    ) {
      throw new Error("Strict official raster production requires a fresh output directory");
    }
    fs.mkdirSync(this.options.outputDir, { recursive: true });
    if (this.options.writeMarkdown) {
      this.markdownAssets.prepare();
    }
  }

  private async renderAll(
    renderer: OfficialMermaidRenderer,
    writer: MarkdownReferenceWriter,
    fixtures: Fixture[],
  ) {
    renderer.prepareRasterInputs(fixtures);
    await renderer.start();
    try {
      await this.renderFixtures(renderer, writer, fixtures);
    } finally {
      await renderer.stop();
    }
    renderer.finishRasterReceipt();
  }

  private async renderFixtures(
    renderer: OfficialMermaidRenderer,
    writer: MarkdownReferenceWriter,
    fixtures: Fixture[],
  ) {
    for (const fixture of fixtures) {
      await this.renderFixture(renderer, writer, fixture);
    }
  }

  private async renderFixture(
    renderer: OfficialMermaidRenderer,
    writer: MarkdownReferenceWriter,
    fixture: Fixture,
  ) {
    if (
      this.options.referenceMode === "strictVendorOriginal" &&
      (fixture.originalFenceSource ?? "").trim() === ""
    ) {
      this.skippedReport.add(
        fixture,
        "empty captured fence body; excluded by fixture inventory contract",
      );
      return;
    }
    if (this.options.skipErrors) {
      await this.renderOptionalFixture(renderer, writer, fixture);
      return;
    }
    await this.renderRequiredFixture(renderer, writer, fixture);
  }

  private async renderOptionalFixture(
    renderer: OfficialMermaidRenderer,
    writer: MarkdownReferenceWriter,
    fixture: Fixture,
  ) {
    const referencesBeforeRender = this.markdownAssets.capture(fixture);
    try {
      await this.renderRequiredFixture(renderer, writer, fixture);
    } catch (error) {
      if (!this.markdownAssets.skipFailedFixture(fixture, referencesBeforeRender)) {
        throw error;
      }
      this.skippedReport.add(fixture, ErrorSummary.fromString(String(error)));
    }
  }

  private async renderRequiredFixture(
    renderer: OfficialMermaidRenderer,
    writer: MarkdownReferenceWriter,
    fixture: Fixture,
  ) {
    await renderer.render(fixture);
    this.renderedCount += 1;
    this.syncMarkdownAssets(fixture);
    this.writeMarkdownReference(writer, fixture);
    console.log(`updated ${fixture.slug}`);
  }

  private assertRendered(fixtures: Fixture[]) {
    if (fixtures.length > 0 && this.renderedCount === 0) {
      throw new Error("All Mermaid fixtures were skipped; check render-skipped.json");
    }
  }

  private writeMarkdownReference(writer: MarkdownReferenceWriter, fixture: Fixture) {
    if (this.options.writeMarkdown) {
      writer.write(fixture);
    }
  }

  private syncMarkdownAssets(fixture: Fixture) {
    if (this.options.writeMarkdown) {
      this.markdownAssets.sync(fixture);
    }
  }

  private validate() {
    assertReferenceScope(this.options.referenceMode, this.options.referenceScope);
    if (
      this.options.referenceMode === "strictVendorOriginal" &&
      this.options.referenceScope === "formal" &&
      this.options.skipErrors
    ) {
      throw new Error("strictVendorOriginal formal reference generation cannot use --skip-errors");
    }
    if (!fs.existsSync(this.options.mermaidJs)) {
      throw new Error(`mermaid.min.js not found: ${this.options.mermaidJs}`);
    }
    if (!fs.existsSync(this.options.mermaidZenumlJs)) {
      throw new Error(`mermaid-zenuml.min.js not found: ${this.options.mermaidZenumlJs}`);
    }
  }
}

const ErrorSummary = {
  fromString(value: string): string {
    return ErrorSummary.truncate(value.split("\n").at(0) ?? "");
  },

  truncate(value: string): string {
    return value.length > 180 ? `${value.slice(0, 180)}...` : value;
  },
};

if (import.meta.main)
  new MermaidDiagramUpdate(CliOptions.parse(process.argv.slice(2))).run().catch((error: Error) => {
    console.error(error.message);
    process.exit(1);
  });
