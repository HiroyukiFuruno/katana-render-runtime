import { randomUUID } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { DiagramTheme, type DiagramThemeName } from "./diagram_theme";
import { type Fixture, FixtureRepository } from "./diagram_update_fixtures";
import { fileSha256, parseJsonValue, publishMermaidPair, sha256 } from "./mermaid-pair-manifest";
import {
  requireNativeRenderInventory,
  type VerifiedNativeRenderBinding,
} from "./native-render-provenance";
import {
  assertReferenceScope,
  type OfficialReferenceMode,
  referenceMode,
} from "./official-reference-mode";
import { PlaywrightLoader } from "./official-renderer";
import type { BrowserHandle, FontReadyDocument, PageHandle } from "./official-renderer-types";
import { SvgSourceFonts } from "./rasterize_svg_fonts";
import { SvgRasterizeInput } from "./rasterize_svg_input";
import {
  assertLoadedSvgExternalImages,
  strictSvgExternalImageUrls,
} from "./strict-svg-external-images";

interface CliParsedOptions {
  inputDir: string;
  outputDir: string;
  theme: DiagramThemeName;
  fixturesDir?: string;
  fixtureGlob?: string;
  nativeBindingsPath?: string;
  referenceMode: OfficialReferenceMode;
  referenceScope: "formal" | "diagnostic";
}

const CliOptions = {
  parse(argv: string[]): CliParsedOptions {
    CliOptions.exitIfHelp(argv);
    const mode = referenceMode(CliOptions.get(argv, "--reference-mode", "strictVendorOriginal"));
    const scope = CliOptions.get(argv, "--reference-scope", "formal");
    if (scope !== "formal" && scope !== "diagnostic") {
      throw new Error(`Unsupported reference scope: ${scope}`);
    }
    assertReferenceScope(mode, scope);
    const fixturesDir = CliOptions.getOptional(argv, "--fixtures-dir");
    const fixtureGlob = CliOptions.getOptional(argv, "--fixture-glob");
    if ((fixturesDir === undefined) !== (fixtureGlob === undefined)) {
      throw new Error("Fixture source binding requires both --fixtures-dir and --fixture-glob");
    }
    if (mode === "strictVendorOriginal" && fixturesDir === undefined) {
      throw new Error("Strict rasterization requires --fixtures-dir and --fixture-glob");
    }
    const inputDir = path.resolve(CliOptions.get(argv, "--input", "tmp/mermaid-sample-official"));
    const nativeBindingsPath = CliOptions.getOptional(argv, "--native-bindings");
    if (mode === "strictVendorOriginal" && nativeBindingsPath !== undefined) {
      throw new Error("Strict native render bindings must be beside the input SVG files");
    }
    return {
      inputDir,
      outputDir: path.resolve(CliOptions.get(argv, "--output", "tmp/mermaid-sample-browser")),
      theme: DiagramTheme.parse(CliOptions.get(argv, "--theme", "dark")).name,
      ...(fixturesDir === undefined ? {} : { fixturesDir: path.resolve(fixturesDir) }),
      ...(fixtureGlob === undefined ? {} : { fixtureGlob }),
      ...(nativeBindingsPath === undefined
        ? mode === "strictVendorOriginal"
          ? { nativeBindingsPath: path.join(inputDir, "render-bindings.json") }
          : {}
        : { nativeBindingsPath: path.resolve(nativeBindingsPath) }),
      referenceMode: mode,
      referenceScope: scope,
    };
  },

  get(argv: string[], name: string, fallback: string): string {
    const index = argv.indexOf(name);
    return index >= 0 ? (argv.at(index + 1) ?? fallback) : fallback;
  },

  getOptional(argv: string[], name: string): string | undefined {
    const index = argv.indexOf(name);
    return index < 0 ? undefined : argv.at(index + 1);
  },

  exitIfHelp(argv: string[]) {
    if (argv.includes("--help")) {
      console.log(
        "Usage: bun run scripts/mermaid/rasterize-svg-dir.ts --input DIR --output DIR [--theme dark|light] [--reference-mode strictVendorOriginal|legacyNormalized] [--reference-scope formal|diagnostic] [--fixtures-dir DIR --fixture-glob GLOB]",
      );
      process.exit(0);
    }
  },
};

class SvgFixtureRepository {
  constructor(private inputDir: string) {}

  list(): string[] {
    return fs
      .readdirSync(this.inputDir)
      .filter((fileName) => fileName.endsWith(".svg"))
      .sort();
  }

  read(fileName: string): string {
    return fs.readFileSync(path.join(this.inputDir, fileName), "utf8");
  }
}

class SvgBrowserRasterizer {
  private browser: BrowserHandle | null = null;
  private theme: DiagramTheme;
  private nativeBindings = new Map<string, VerifiedNativeRenderBinding>();

  constructor(private options: CliParsedOptions) {
    this.theme = DiagramTheme.parse(options.theme);
  }

  async run() {
    this.prepareOutputDir();
    await this.start();
    try {
      await this.rasterizeAll(new SvgFixtureRepository(this.options.inputDir));
    } finally {
      await this.stop();
    }
  }

  private prepareOutputDir() {
    if (this.options.referenceMode === "strictVendorOriginal") {
      if (
        fs.existsSync(this.options.outputDir) &&
        fs.readdirSync(this.options.outputDir).length > 0
      ) {
        throw new Error(
          `Strict rasterization requires a fresh output directory: ${this.options.outputDir}`,
        );
      }
    } else {
      fs.rmSync(this.options.outputDir, { recursive: true, force: true });
    }
    fs.mkdirSync(this.options.outputDir, { recursive: true });
  }

  private async start() {
    const { chromium } = await new PlaywrightLoader().load();
    this.browser = await chromium.launch({ headless: true });
  }

  private async stop() {
    await this.browser?.close();
  }

  private async rasterizeAll(repository: SvgFixtureRepository) {
    const fixtureMap = this.loadFixtures();
    const inputFiles = repository.list();
    this.validateInputInventory(inputFiles, fixtureMap);
    if (this.options.referenceMode === "strictVendorOriginal") {
      if (this.options.nativeBindingsPath === undefined) {
        throw new Error("Strict rasterization has no native render binding receipt");
      }
      this.nativeBindings = requireNativeRenderInventory(
        this.options.nativeBindingsPath,
        [...fixtureMap.values()],
        this.options.inputDir,
      );
    }
    for (const fileName of inputFiles) {
      await this.rasterizeInputFile(repository, fileName, fixtureMap);
    }
  }

  private validateInputInventory(inputFiles: string[], fixtureMap: Map<string, Fixture>) {
    if (this.options.referenceMode !== "strictVendorOriginal") return;
    const expectedFiles = [...fixtureMap.values()]
      .filter((fixture) => (fixture.originalFenceSource ?? "").trim() !== "")
      .map((fixture) => `${fixture.slug}.svg`);
    const missing = expectedFiles.filter((fileName) => !inputFiles.includes(fileName));
    const unexpected = inputFiles.filter(
      (fileName) => !fixtureMap.has(path.basename(fileName, ".svg")),
    );
    if (missing.length > 0 || unexpected.length > 0) {
      throw new Error(
        `Strict rasterization SVG inventory mismatch; missing=${missing.join(",")}; unexpected=${unexpected.join(",")}`,
      );
    }
  }

  private async rasterizeInputFile(
    repository: SvgFixtureRepository,
    fileName: string,
    fixtureMap: Map<string, Fixture>,
  ) {
    const svg = repository.read(fileName);
    const fixture = fixtureMap.get(path.basename(fileName, ".svg"));
    if (fixture !== undefined && (fixture.originalFenceSource ?? "").trim() === "") {
      console.log(`skipped empty fixture ${fixture.slug}`);
      return;
    }
    if (this.options.referenceMode === "strictVendorOriginal" && fixture === undefined) {
      throw new Error(`Strict rasterization has no source fixture for ${fileName}`);
    }
    const input = new SvgRasterizeInput(svg);
    if (!input.shouldRasterize()) {
      if (this.options.referenceMode === "strictVendorOriginal") {
        throw new Error(`Strict rasterization input is not SVG: ${fileName}`);
      }
      console.log(`skipped ${fileName}`);
      return;
    }
    const renderedSvgInput =
      this.options.referenceMode === "strictVendorOriginal" ? svg : input.browserInnerHtml();
    await this.rasterize(fileName, svg, renderedSvgInput, fixture);
  }

  private loadFixtures(): Map<string, Fixture> {
    if (this.options.fixturesDir === undefined || this.options.fixtureGlob === undefined) {
      return new Map();
    }
    const fixtures = new FixtureRepository(this.options.fixturesDir)
      .list()
      .filter((fixture) => matchesFixtureGlob(fixture.fileName, this.options.fixtureGlob ?? ""));
    if (fixtures.length === 0) throw new Error("Strict rasterization fixture inventory is empty");
    return new Map(fixtures.map((fixture) => [fixture.slug, fixture]));
  }

  private async rasterize(
    fileName: string,
    sourceSvg: string,
    renderedSvgInput: string,
    fixture: Fixture | undefined,
  ) {
    const page = await this.currentBrowser().newPage({
      viewport: { width: 1520, height: 845 },
      deviceScaleFactor: 1,
    });
    try {
      await page.setContent(this.baseHtml(), { waitUntil: "load" });
      await this.capture(page, fileName, sourceSvg, renderedSvgInput, fixture);
      console.log(`rasterized ${fileName}`);
    } finally {
      await page.close();
    }
  }

  private async capture(
    page: PageHandle,
    fileName: string,
    sourceSvg: string,
    svg: string,
    fixture: Fixture | undefined,
  ) {
    if (this.options.referenceMode !== "strictVendorOriginal") {
      await this.installSourceFonts(page, sourceSvg);
    }
    await page.evaluate(
      (input) => {
        const diagramElement = document.getElementById("diagram") as HTMLElement;
        diagramElement.innerHTML = input.svg;
      },
      { svg },
    );
    await SvgCaptureSizer.resize(page, this.options.referenceMode === "strictVendorOriginal");
    await SvgExternalImages.waitUntilReady(page);
    await page.evaluate(() => (document as FontReadyDocument).fonts.ready);
    const attemptDir = fs.mkdtempSync(
      path.join(
        this.options.outputDir,
        `.capture-${path.basename(fileName, ".svg")}-${randomUUID()}-`,
      ),
    );
    const pngPath = path.join(attemptDir, `${path.basename(fileName, ".svg")}.png`);
    let published = false;
    try {
      await page.locator("#capture").screenshot({
        path: pngPath,
        omitBackground: false,
      });
      const sourceSvgBytes = Buffer.from(sourceSvg, "utf8");
      const usedSvg = Buffer.from(svg, "utf8");
      const scriptDir = path.dirname(fileURLToPath(import.meta.url));
      const assetPaths = [
        path.join(scriptDir, "rasterize-svg-dir.ts"),
        path.join(scriptDir, "rasterize_svg_input.ts"),
        path.join(scriptDir, "rasterize_svg_fonts.ts"),
        path.join(scriptDir, "diagram_theme.ts"),
        path.join(scriptDir, "official-renderer.ts"),
        path.join(scriptDir, "official-reference-mode.ts"),
        path.join(scriptDir, "strict-svg-external-images.ts"),
        path.join(scriptDir, "native-render-provenance.ts"),
        path.join(scriptDir, "diagram_update_fixtures.ts"),
        path.resolve("package.json"),
        path.resolve("bun.lock"),
      ];
      const runtimeAssets = assetPaths.map((assetPath) => ({
        path: path.relative(process.cwd(), assetPath),
        sha256: fileSha256(assetPath),
      }));
      const config = this.pairConfig(fixture, svg);
      const provenanceDocument = { config, runtimeAssets };
      const serializedProvenance = Buffer.from(JSON.stringify(provenanceDocument));
      publishMermaidPair({
        directory: this.options.outputDir,
        attemptDirectory: attemptDir,
        slug: path.basename(fileName, ".svg"),
        role: "candidate",
        sourceSvg: sourceSvgBytes,
        renderedSvg: usedSvg,
        png: fs.readFileSync(pngPath),
        provenance: {
          path: "embedded:rasterize-svg-dir",
          sha256: sha256(serializedProvenance),
          document: provenanceDocument,
        },
        config,
        runtimeAssets,
      });
      published = true;
    } catch (error) {
      const detail = error instanceof Error ? error.message : "unclassified rasterization failure";
      throw new Error(`Rasterization failed; attempt retained at ${attemptDir}: ${detail}`);
    } finally {
      if (published) fs.rmSync(attemptDir, { recursive: true, force: true });
    }
  }

  private nativeBinding(fixture: Fixture | undefined): VerifiedNativeRenderBinding {
    if (fixture === undefined) throw new Error("Strict native render binding requires a fixture");
    const binding = this.nativeBindings.get(fixture.slug);
    if (binding === undefined)
      throw new Error(`Strict native render binding is missing ${fixture.slug}`);
    return binding;
  }

  private nativeBindingConfig(fixture: Fixture | undefined) {
    const binding = this.nativeBinding(fixture);
    return {
      nativeRenderBindingsSha256: binding.receiptSha256,
      nativeRenderBinarySha256: binding.receipt.binarySha256After,
      nativeSourceParserSha256: binding.receipt.sourceParserSha256After,
      nativeInputMarkdownSha256: binding.row.inputMarkdownSha256,
      nativeOutputSvgSha256: binding.row.outputSvgSha256,
    };
  }

  private pairConfig(fixture: Fixture | undefined, svg: string) {
    const playwrightPackage = parseJsonValue(
      fs.readFileSync(path.resolve("node_modules/playwright/package.json"), "utf8"),
      "Playwright package metadata",
    );
    if (
      typeof playwrightPackage !== "object" ||
      playwrightPackage === null ||
      Array.isArray(playwrightPackage)
    ) {
      throw new Error("Playwright package metadata is not an object");
    }
    const playwrightVersion = playwrightPackage.version;
    if (typeof playwrightVersion !== "string") {
      throw new Error("Playwright package metadata has no version");
    }
    const strict = this.options.referenceMode === "strictVendorOriginal";
    const nativeBindingConfig = strict ? this.nativeBindingConfig(fixture) : {};
    return {
      theme: this.theme.name,
      viewport: { width: 1520, height: 845, deviceScaleFactor: 1 },
      captureResize: strict
        ? "capture-box-only-raw-svg-v1"
        : "svg-viewbox-or-dimensions-ceil-plus-24px-padding-v1",
      sourceFontUrls: strict ? [] : SvgSourceFonts.urls(svg),
      externalImageWaitPolicy: strict
        ? "svg-image-load-decode-positive-dimensions-and-frame-v2"
        : null,
      nodeVersion: process.version,
      bunVersion: process.versions.bun ?? null,
      playwrightVersion,
      referenceMode: this.options.referenceMode,
      referenceScope: this.options.referenceScope,
      originalFenceLanguage: fixture?.fenceLanguage ?? null,
      originalFenceSourceSha256:
        fixture?.originalFenceSource === undefined
          ? null
          : sha256(Buffer.from(fixture.originalFenceSource, "utf8")),
      canonicalSourceSha256:
        fixture?.canonicalSource === undefined
          ? null
          : sha256(Buffer.from(fixture.canonicalSource, "utf8")),
      sourceAdapterPolicy:
        fixture?.fenceLanguage === "zenuml" ? "zenuml-prefix-v1" : "mermaid-fence-lines-v1",
      sourceMarkdownSha256: fixture === undefined ? null : fileSha256(fixture.filePath),
      ...nativeBindingConfig,
    };
  }

  private installSourceFonts(page: PageHandle, svg: string): Promise<void> {
    return page.evaluate(async (urls) => {
      await Promise.all(
        urls.map(
          (url) =>
            new Promise<void>((resolve) => {
              const link = document.createElement("link");
              link.rel = "stylesheet";
              link.href = url;
              link.addEventListener("load", () => resolve(), { once: true });
              link.addEventListener("error", () => resolve(), { once: true });
              document.head.appendChild(link);
            }),
        ),
      );
    }, SvgSourceFonts.urls(svg));
  }

  private currentBrowser(): BrowserHandle {
    if (this.browser === null) {
      throw new Error("Browser rasterizer is not started");
    }
    return this.browser;
  }

  private baseHtml() {
    return `<!doctype html><html><head><meta charset="utf-8"><style>
html,body{margin:0;background:${this.theme.canvasBackground};color:${this.theme.text};color-scheme:${this.theme.colorScheme()};font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}
#capture{display:flex;align-items:center;justify-content:center;overflow:hidden;padding:12px;box-sizing:border-box;background:${this.theme.canvasBackground}}
#diagram{max-width:100%;max-height:100%;display:flex;align-items:center;justify-content:center}
#diagram svg{max-width:100%;max-height:100%}
</style></head><body><div id="capture"><div id="diagram"></div></div></body></html>`;
  }
}

const SvgCaptureSizer = {
  resize(page: PageHandle, strict: boolean): Promise<void> {
    return page.evaluate(
      ({ strict }) => {
        const svgElement = document.querySelector("#diagram svg") as SVGSVGElement;
        const viewBox = String(svgElement.getAttribute("viewBox"))
          .split(/\s+/)
          .map((value) => Number(value));
        const dimensionAt = (values: number[], index: number): number => {
          const value = values.at(index);
          if (value === undefined || !Number.isFinite(value)) {
            throw new Error("SVG viewBox is invalid");
          }
          return value;
        };
        const width = Math.ceil(dimensionAt(viewBox, 2));
        const height = Math.ceil(dimensionAt(viewBox, 3));
        if (!strict) {
          svgElement.setAttribute("width", String(width));
          svgElement.setAttribute("height", String(height));
          svgElement.style.maxWidth = `${width}px`;
        }
        const capture = document.getElementById("capture") as HTMLElement;
        capture.style.width = `${width + 24}px`;
        capture.style.height = `${height + 24}px`;
      },
      { strict },
    );
  },
};

function matchesFixtureGlob(fileName: string, glob: string): boolean {
  const escaped = glob
    .replace(/[.+^${}()|[\]\\]/g, "\\$&")
    .replace(/\*/g, ".*")
    .replace(/\?/g, ".");
  return new RegExp(`^${escaped}$`).test(fileName);
}

const SvgExternalImages = {
  async waitUntilReady(page: PageHandle): Promise<void> {
    const references = await page.evaluate(() => {
      const namespace = "http://www.w3.org/1999/xlink";
      return Array.from(document.querySelectorAll("#diagram image")).map((image) => ({
        href: image.getAttribute("href"),
        xlinkHref: image.getAttributeNS(namespace, "href") ?? image.getAttribute("xlink:href"),
      }));
    });
    const urls = strictSvgExternalImageUrls(references);
    const loaded = await page.evaluate(async (imageUrls) => {
      const loadAndDecode = (url: string) =>
        new Promise<{
          url: string;
          complete: boolean;
          naturalWidth: number;
          naturalHeight: number;
          decoded: boolean;
        }>((resolve, reject) => {
          const image = new Image();
          let decoding = false;
          const fail = () => reject(new Error(`Strict SVG image failed to load: ${url}`));
          const decodeIfComplete = () => {
            if (!image.complete || decoding) return;
            decoding = true;
            if (image.naturalWidth <= 0 || image.naturalHeight <= 0) {
              fail();
              return;
            }
            image.decode().then(
              () =>
                resolve({
                  url,
                  complete: image.complete,
                  naturalWidth: image.naturalWidth,
                  naturalHeight: image.naturalHeight,
                  decoded: true,
                }),
              fail,
            );
          };
          image.addEventListener("load", decodeIfComplete, { once: true });
          image.addEventListener("error", fail, { once: true });
          image.src = url;
          decodeIfComplete();
        });
      const images = await Promise.all(imageUrls.map(loadAndDecode));
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
      return images;
    }, urls);
    assertLoadedSvgExternalImages(loaded);
  },
};

new SvgBrowserRasterizer(CliOptions.parse(process.argv.slice(2))).run().catch((error: Error) => {
  console.error(error.message);
  process.exit(1);
});
