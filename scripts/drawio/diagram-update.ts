import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { getOptionValue, printHelpIfRequested } from "./cli-options";
import {
  type DrawioRendererOptions,
  type DrawioRenderFixture,
  expandHome,
  OfficialDrawioRenderer,
} from "./official-renderer";

interface CliParsedOptions extends DrawioRendererOptions {
  fixturesDir: string;
}

const CliOptions = {
  parse(argv: string[]): CliParsedOptions {
    if (
      printHelpIfRequested(
        argv,
        "Usage: bun run scripts/drawio/diagram-update.ts [--fixtures DIR] [--output DIR] [--drawio-js FILE]",
      )
    ) {
      process.exit(0);
    }
    const fixturesDir = path.resolve(
      getOptionValue(argv, "--fixtures", "tests/fixtures/drawio/basic"),
    );
    return {
      fixturesDir,
      outputDir: path.resolve(getOptionValue(argv, "--output", path.join(fixturesDir, "official"))),
      drawioJs: path.resolve(
        expandHome(
          getOptionValue(
            argv,
            "--drawio-js",
            process.env.DRAWIO_JS || "~/.local/katana/drawio.min.js",
          ),
        ),
      ),
      resourcesDir: path.resolve(
        getOptionValue(
          argv,
          "--resources",
          "crates/katana-render-runtime/src/markdown/drawio_renderer/js_runtime/resources",
        ),
      ),
      resourceManifest: path.resolve(
        getOptionValue(
          argv,
          "--resource-manifest",
          "crates/katana-render-runtime/src/markdown/drawio_renderer/js_runtime/resources/drawio-resource-manifest.json",
        ),
      ),
    };
  },
};

class DrawioFixtureRepository {
  constructor(private fixturesDir: string) {}

  list(): DrawioRenderFixture[] {
    return fs
      .readdirSync(this.fixturesDir)
      .filter((fileName) => fileName.endsWith(".drawio"))
      .sort()
      .map((fileName) => this.read(fileName));
  }

  private read(fileName: string): DrawioRenderFixture {
    const filePath = path.join(this.fixturesDir, fileName);
    return {
      slug: path.basename(fileName, ".drawio"),
      source: fs.readFileSync(filePath, "utf8"),
    };
  }
}

class DrawioDiagramUpdate {
  constructor(private options: CliParsedOptions) {}

  async run() {
    this.validate();
    fs.mkdirSync(this.options.outputDir, { recursive: true });
    const renderer = new OfficialDrawioRenderer(this.options);
    await this.renderFixtures(
      renderer,
      new DrawioFixtureRepository(this.options.fixturesDir).list(),
    );
  }

  private async renderFixtures(renderer: OfficialDrawioRenderer, fixtures: DrawioRenderFixture[]) {
    await renderer.start();
    await this.renderStartedFixtures(renderer, fixtures).finally(() => renderer.stop());
  }

  private async renderStartedFixtures(
    renderer: OfficialDrawioRenderer,
    fixtures: DrawioRenderFixture[],
  ) {
    for (const fixture of fixtures) {
      await renderer.render(fixture);
      console.log(`updated ${fixture.slug}`);
    }
  }

  private validate() {
    this.validateDrawioJs();
    this.validateResourcesDir();
    this.validateResourceManifest();
  }

  private validateDrawioJs() {
    assert(
      fs.existsSync(this.options.drawioJs),
      `drawio.min.js not found: ${this.options.drawioJs}`,
    );
  }

  private validateResourcesDir() {
    assert(
      fs.existsSync(this.options.resourcesDir),
      `Draw.io resources directory not found: ${this.options.resourcesDir}`,
    );
  }

  private validateResourceManifest() {
    assert(
      fs.existsSync(this.options.resourceManifest),
      `Draw.io resource manifest not found: ${this.options.resourceManifest}`,
    );
  }
}

new DrawioDiagramUpdate(CliOptions.parse(process.argv.slice(2))).run().catch((error: Error) => {
  console.error(error.message);
  process.exit(1);
});
