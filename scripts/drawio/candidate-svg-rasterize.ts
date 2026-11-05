import fs from "node:fs";
import path from "node:path";
import { PlaywrightLoader } from "../mermaid/official-renderer";
import type { BrowserHandle } from "../mermaid/official-renderer-types";
import { prepareDrawioSource } from "./official-renderer";
import { installDrawioDeterminism } from "./official-runtime-determinism";
import {
  assertFreshDrawioOutput,
  assertMatchingDrawioSlugs,
  captureDrawioSvg,
  drawioCaptureHtml,
} from "./svg-capture";

interface Options {
  fixtures: string;
  candidateSvg: string;
  official: string;
  output: string;
}

const Options = {
  read(argv: string[], flag: string): string {
    const index = argv.indexOf(flag);
    const value = index < 0 ? undefined : argv[index + 1];
    if (value === undefined || value.startsWith("--")) {
      throw new Error(`Missing value for ${flag}`);
    }
    return path.resolve(value);
  },

  parse(argv: string[]): Options {
    return {
      fixtures: Options.read(argv, "--fixtures"),
      candidateSvg: Options.read(argv, "--candidate-svg"),
      official: Options.read(argv, "--official"),
      output: Options.read(argv, "--output"),
    };
  },
};

class CandidateSvgRasterizer {
  constructor(private readonly options: Options) {}

  async run(browser: BrowserHandle) {
    const fixtureSlugs = this.slugs(this.options.fixtures, ".drawio");
    const officialSvgSlugs = this.slugs(this.options.official, ".svg");
    const officialPngSlugs = this.slugs(this.options.official, ".png");
    assertMatchingDrawioSlugs(
      fixtureSlugs,
      this.slugs(this.options.candidateSvg, ".svg"),
      officialSvgSlugs,
      officialPngSlugs,
    );
    assertFreshDrawioOutput(this.options.output);
    fs.mkdirSync(this.options.output, { recursive: true });
    for (const slug of fixtureSlugs) {
      const page = await browser.newPage({
        viewport: { width: 1520, height: 845 },
        deviceScaleFactor: 1,
      });
      try {
        await page.setContent(drawioCaptureHtml(), { waitUntil: "load" });
        await page.evaluate(installDrawioDeterminism);
        await captureDrawioSvg(page, {
          slug,
          svg: fs.readFileSync(path.join(this.options.candidateSvg, `${slug}.svg`)),
          sourceFonts: prepareDrawioSource(
            fs.readFileSync(path.join(this.options.fixtures, `${slug}.drawio`), "utf8"),
          ),
          outputDir: this.options.output,
        });
      } finally {
        await page.close();
      }
    }
    assertMatchingDrawioSlugs(
      fixtureSlugs,
      this.slugs(this.options.output, ".svg"),
      officialSvgSlugs,
      this.slugs(this.options.output, ".png"),
    );
  }

  private slugs(directory: string, extension: string): string[] {
    return fs
      .readdirSync(directory)
      .filter((name) => name.endsWith(extension))
      .map((name) => name.slice(0, -extension.length))
      .sort();
  }
}

async function main() {
  const argv = process.argv.slice(2);
  const assertOutputIndex = argv.indexOf("--assert-output");
  if (assertOutputIndex >= 0) {
    const output = argv[assertOutputIndex + 1];
    if (assertOutputIndex !== 0 || argv.length !== 2 || output === undefined || output.startsWith("--")) {
      throw new Error("Invalid arguments for --assert-output");
    }
    assertFreshDrawioOutput(path.resolve(output));
    return;
  }
  const options = Options.parse(argv);
  const { chromium } = await new PlaywrightLoader().load();
  const browser = await chromium.launch({ headless: true });
  try {
    await new CandidateSvgRasterizer(options).run(browser);
  } finally {
    await browser.close();
  }
}

await main();
