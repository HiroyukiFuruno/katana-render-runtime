import fs from "node:fs";
import path from "node:path";
import { validateGeneratedClassReference } from "./class-reference-gate";
import { DiagramTheme, type DiagramThemeName, type MermaidThemeName } from "./diagram_theme";
import {
  type JsonValue,
  parseJsonValue,
  publishMermaidPair,
  sha256,
} from "./mermaid-pair-manifest";
import {
  assertReferenceScope,
  assertStrictOriginal,
  type OfficialReferenceMode,
} from "./official-reference-mode";
import { OfficialRendererDeterminism } from "./official-renderer-determinism";
import { MermaidRuntimeScripts } from "./official-renderer-i18n";
import {
  primaryStageForDiagramType,
  primaryStageForReferenceMode,
  primarySvgForStages,
  writeOfficialRendererProvenance,
} from "./official-renderer-provenance";
import type {
  BrowserHandle,
  FontReadyDocument,
  MermaidConfig,
  MermaidI18nWindow,
  MermaidSvgStages,
  PageHandle,
  PlaywrightModule,
} from "./official-renderer-types";
import { OfficialSourceNormalizer } from "./official-source-normalizer";
import {
  assignStrictSvg,
  closeCaptureObservation,
  type LoadedSvgObservation,
  observeCaptureSnapshot,
  prepareStrictSvgCapture,
  resizeSvgCapture,
} from "./raster-capture-observation";
import type { RasterProductionSession } from "./raster-production-session";
import { waitForStrictSvgExternalImages } from "./strict-svg-external-images";

export { expandHome } from "./official-renderer-i18n";

export class PlaywrightLoader {
  async load(): Promise<PlaywrightModule> {
    return (await import("playwright")) as PlaywrightModule;
  }
}

export interface RenderFixture {
  slug: string;
  source: string;
  title: string;
  originalFenceSource?: string;
  canonicalSource?: string;
  fenceLanguage?: "mermaid" | "zenuml";
  markdown?: string;
}

interface MermaidNormalizationRequest {
  theme: MermaidThemeName;
  text: string;
  fill: string;
  stroke: string;
  arrow: string;
  source: string;
  background: string;
}

export interface RendererOptions {
  outputDir: string;
  mermaidJs: string;
  mermaidZenumlJs: string;
  theme: DiagramThemeName;
  referenceMode: OfficialReferenceMode;
  referenceScope: "formal" | "diagnostic";
  rasterSession?: RasterProductionSession;
  rasterSessionConfigPath?: string;
  rasterSessionConfigSha256?: string;
}

export class OfficialMermaidRenderer {
  private browser: BrowserHandle | null = null;
  private options: RendererOptions;
  private theme: DiagramTheme;
  private rasterInventory: string[] | null = null;

  constructor(options: RendererOptions) {
    this.options = options;
    this.theme = DiagramTheme.parse(options.theme);
  }

  prepareRasterInputs(
    fixtures: Array<{
      slug: string;
      filePath: string;
      markdown: string;
      originalFenceSource?: string;
    }>,
  ) {
    if (!this.isStrictMode()) return;
    const session = this.requireRasterSession();
    this.rasterInventory = fixtures
      .filter((fixture) => (fixture.originalFenceSource ?? "").trim() !== "")
      .map((fixture) => fixture.slug)
      .sort();
    for (const fixture of fixtures)
      session.pinInput(path.resolve(fixture.filePath), Buffer.from(fixture.markdown));
  }

  finishRasterReceipt() {
    if (this.isStrictMode()) return this.requireRasterSession().finish();
  }

  private requireRasterSession() {
    if (this.options.rasterSession === undefined)
      throw new Error("Strict official producer requires externally orchestrated raster session");
    return this.options.rasterSession;
  }

  async start() {
    if (this.isStrictMode()) {
      if (this.rasterInventory === null)
        throw new Error("Strict official raster source inventory is not prepared");
      this.requireRasterSession().requireReady(
        "official",
        this.options.outputDir,
        this.rasterInventory,
      );
      for (const filePath of [
        this.options.mermaidJs,
        this.options.mermaidZenumlJs,
        ...this.configSourcePaths(true),
      ]) {
        this.requireRasterSession().pinInput(path.resolve(filePath), fs.readFileSync(filePath));
      }
    }
    const { chromium } = await new PlaywrightLoader().load();
    this.browser = await chromium
      .launch({ headless: true })
      .catch((error) => this.rethrowLaunchError(error));
    if (this.isStrictMode()) {
      try {
        await this.requireRasterSession().observeBrowserBefore(this.browser);
      } catch (error) {
        await this.browser.close();
        throw error;
      }
    }
  }

  async stop() {
    if (this.browser === null) return;
    try {
      if (this.isStrictMode()) await this.requireRasterSession().observeBrowserEnd(this.browser);
    } finally {
      await this.browser.close();
      if (this.isStrictMode()) this.requireRasterSession().markBrowserClosed();
    }
  }

  async render(fixture: RenderFixture) {
    assertReferenceScope(this.options.referenceMode, this.options.referenceScope);
    if (
      this.options.referenceMode === "strictVendorOriginal" &&
      (fixture.originalFenceSource === undefined || fixture.canonicalSource === undefined)
    ) {
      throw new Error(`Strict reference requires original fence bytes: ${fixture.slug}`);
    }
    const page = await this.newPage();
    try {
      await this.renderPage(page, fixture);
    } finally {
      await page.close();
    }
  }

  private rethrowLaunchError(error: Error): never {
    if (error.message.includes("Executable doesn't exist")) {
      throw new Error(
        "Playwright browser is missing. Run `just mermaid-diagram-browser-install` first.",
      );
    }
    throw error;
  }

  private newPage(): Promise<PageHandle> {
    return this.currentBrowser().newPage({
      viewport: { width: 1520, height: 845 },
      deviceScaleFactor: 1,
    });
  }

  private async renderPage(page: PageHandle, fixture: RenderFixture) {
    await page.setContent(this.baseHtml(), { waitUntil: "load" });
    await this.installStorage(page);
    await OfficialRendererDeterminism.install(page);
    await this.installModeRuntime(page);
    const strict = this.isStrictMode();
    const stages = await this.renderSvg(page, fixture);
    this.validateRenderedStages(fixture, stages);
    const primarySvg =
      primaryStageForReferenceMode(this.options.referenceMode, stages.diagramType) === "rawMermaid"
        ? stages.rawSvg
        : primarySvgForStages(stages);
    const provenancePath = this.writeRenderProvenance(fixture, stages, strict);
    if (strict) {
      const capturePage = await this.newPage();
      try {
        await capturePage.setContent(this.baseHtml(), { waitUntil: "load" });
        await prepareStrictSvgCapture(capturePage, primarySvg);
        const loadedSvg = await assignStrictSvg(capturePage, primarySvg);
        await this.capture(capturePage, fixture, primarySvg, true, provenancePath, true, loadedSvg);
      } finally {
        await capturePage.close();
      }
      return;
    }
    await this.setPrimarySvg(page, primarySvg);
    await this.capture(
      page,
      fixture,
      primarySvg,
      this.isClassDiagram(stages.diagramType),
      provenancePath,
      false,
      null,
    );
  }

  private async installModeRuntime(page: PageHandle) {
    const strict = this.isStrictMode();
    if (!strict) await this.installRuntime(page);
    await page.addScriptTag({ path: this.options.mermaidJs });
    await page.addScriptTag({ path: this.options.mermaidZenumlJs });
    if (!strict) await page.addScriptTag({ path: MermaidRuntimeScripts.zenumlBridgePath() });
  }

  private validateRenderedStages(fixture: RenderFixture, stages: MermaidSvgStages) {
    const strict = this.isStrictMode();
    if (strict) assertStrictOriginal(fixture.canonicalSource ?? "", stages);
    const primaryStage = primaryStageForDiagramType(stages.diagramType);
    const isClassDiagram = strict
      ? this.isClassDiagram(stages.diagramType)
      : primaryStage === "rawMermaid";
    const source = strict ? (fixture.canonicalSource ?? "") : fixture.source;
    if (
      isClassDiagram &&
      (stages.preprocessedSource !== source || stages.renderSource !== source)
    ) {
      throw new Error("classDiagram render source must exactly match the fixture source");
    }
    validateGeneratedClassReference({
      slug: fixture.slug,
      diagramType: isClassDiagram ? "classDiagram" : stages.diagramType,
      source,
      renderSource: stages.renderSource,
      rawSvg: stages.rawSvg,
      restoredSvg: stages.restoredSvg,
    });
  }

  private writeRenderProvenance(
    fixture: RenderFixture,
    stages: MermaidSvgStages,
    strict: boolean,
  ): string {
    const provenanceFixture = strict
      ? { ...fixture, source: fixture.canonicalSource ?? "" }
      : fixture;
    return writeOfficialRendererProvenance({
      outputDir: this.options.outputDir,
      fixture: provenanceFixture,
      stages,
      themeName: this.theme.name,
      themeVariables: this.theme.variables(),
      mermaidConfig: this.mermaidConfig(),
      normalizationRequest: this.normalizationRequest(),
      mermaidJs: this.options.mermaidJs,
      mermaidZenumlJs: this.options.mermaidZenumlJs,
      runtimeScripts: this.runtimeScriptPaths(strict),
      configSourceFiles: this.configSourcePaths(strict),
      referenceMode: this.options.referenceMode,
      referenceScope: this.options.referenceScope,
    });
  }

  private runtimeScriptPaths(strict: boolean): string[] {
    return strict
      ? []
      : [...MermaidRuntimeScripts.paths(), MermaidRuntimeScripts.zenumlBridgePath()];
  }

  private configSourcePaths(strict: boolean): string[] {
    const shared = [
      "scripts/mermaid/official-renderer.ts",
      "scripts/mermaid/official-renderer-determinism.ts",
    ];
    return strict
      ? [
          ...shared,
          "scripts/mermaid/official-reference-mode.ts",
          "scripts/mermaid/raster-capture-observation.ts",
          "scripts/mermaid/strict-svg-external-images.ts",
          "scripts/mermaid/diagram_theme.ts",
          "scripts/mermaid/diagram_update_fixtures.ts",
        ]
      : [
          ...shared,
          "scripts/mermaid/official-renderer-i18n.ts",
          "scripts/mermaid/official-source-normalizer.ts",
          "scripts/mermaid/diagram_theme.ts",
        ];
  }

  private isStrictMode(): boolean {
    return this.options.referenceMode === "strictVendorOriginal";
  }

  private isClassDiagram(diagramType: string): boolean {
    return diagramType.toLowerCase().startsWith("class");
  }

  private async installRuntime(page: PageHandle) {
    await installSvgTransformBoundsBridge(page);
    for (const scriptPath of MermaidRuntimeScripts.paths()) {
      await page.addScriptTag({ path: scriptPath });
    }
    await page.evaluate(() => {
      (window as MermaidI18nWindow).__katanaOfficialReference = true;
    });
  }

  private installStorage(page: PageHandle): Promise<void> {
    return page.evaluate(() => {
      const storage = () => {
        const values = new Map<string, string>();
        return {
          get length() {
            return values.size;
          },
          clear() {
            values.clear();
          },
          getItem(key: string) {
            return values.get(String(key)) ?? null;
          },
          key(index: number) {
            return Array.from(values.keys()).at(index) ?? null;
          },
          removeItem(key: string) {
            values.delete(String(key));
          },
          setItem(key: string, value: string) {
            values.set(String(key), String(value));
          },
        };
      };
      Object.defineProperty(window, "localStorage", { value: storage(), configurable: true });
      Object.defineProperty(window, "sessionStorage", { value: storage(), configurable: true });
    });
  }

  private async capture(
    page: PageHandle,
    fixture: RenderFixture,
    svg: string,
    preserveSvgBytes: boolean,
    provenancePath: string,
    strict: boolean,
    loadedSvg: LoadedSvgObservation | null,
  ) {
    const attemptDir = fs.mkdtempSync(
      path.join(this.options.outputDir, `.capture-${fixture.slug}-`),
    );
    const pngPath = path.join(attemptDir, `${fixture.slug}.png`);
    let published = false;
    try {
      await this.resizeCapture(page, strict);
      if (strict) await this.waitForSvgImages(page);
      await page.evaluate(() => (document as FontReadyDocument).fonts.ready);
      const before = strict ? await observeCaptureSnapshot(page) : null;
      const screenshot = await page.locator("#capture").screenshot({
        path: pngPath,
        omitBackground: false,
      });
      const after = strict ? await observeCaptureSnapshot(page) : null;
      const sourceSvg = Buffer.from(preserveSvgBytes ? svg : svg.replace(/[ \t]+$/gm, ""), "utf8");
      const renderedSvg = Buffer.from(svg, "utf8");
      const provenanceDocument = parseJsonValue(
        fs.readFileSync(provenancePath, "utf8"),
        "official renderer provenance",
      );
      const manifest = publishMermaidPair({
        directory: this.options.outputDir,
        attemptDirectory: attemptDir,
        slug: fixture.slug,
        role: "official",
        sourceSvg,
        renderedSvg,
        png: screenshot,
        provenance: {
          path: "embedded:official-renderer-provenance",
          sha256: sha256(Buffer.from(JSON.stringify(provenanceDocument))),
          document: provenanceDocument,
        },
        config: this.captureConfig(fixture, preserveSvgBytes, strict, before?.viewport),
        runtimeAssets: officialRuntimeAssets(provenanceDocument),
      });
      if (strict) {
        if (before === null || after === null || loadedSvg === null)
          throw new Error("Actual official capture observation missing");
        this.requireRasterSession().recordPublishedPair({
          manifest,
          sourceSvg,
          renderedSvg,
          screenshot,
          attemptPng: fs.readFileSync(pngPath),
          publishedPng: fs.readFileSync(path.join(this.options.outputDir, `${fixture.slug}.png`)),
          capture: closeCaptureObservation(sourceSvg, loadedSvg, before, after),
          rawSvgSha256: sha256(sourceSvg),
          canonicalSourceSha256: sha256(Buffer.from(fixture.canonicalSource ?? "")),
        });
      }
      published = true;
    } catch (error) {
      const detail = error instanceof Error ? error.message : "unclassified capture failure";
      throw new Error(`Official capture failed; attempt retained at ${attemptDir}: ${detail}`);
    } finally {
      if (published) fs.rmSync(attemptDir, { recursive: true, force: true });
    }
  }

  private captureConfig(
    fixture: RenderFixture,
    preserveSvgBytes: boolean,
    strict: boolean,
    actualViewport: { width: number; height: number } | undefined,
  ) {
    if (strict && actualViewport === undefined) {
      throw new Error("Official viewport observation missing");
    }
    return {
      theme: this.theme.name,
      viewport: { ...(actualViewport ?? { width: 1520, height: 845 }), deviceScaleFactor: 1 },
      ...(strict ? { initialViewport: { width: 1520, height: 845, deviceScaleFactor: 1 } } : {}),
      captureResize: strict
        ? "raw-svg-viewbox-viewport-before-assignment-v1"
        : "svg-viewbox-or-dimensions-ceil-plus-24px-padding-v1",
      sourceFontUrls: strict ? [] : null,
      preserveSvgBytes,
      externalImageWaitPolicy: strict
        ? "svg-image-load-decode-positive-dimensions-and-frame-v2"
        : null,
      referenceMode: this.options.referenceMode,
      referenceScope: this.options.referenceScope,
      originalFenceLanguage: fixture.fenceLanguage ?? null,
      originalFenceSourceSha256: sha256(
        Buffer.from(fixture.originalFenceSource ?? fixture.source, "utf8"),
      ),
      canonicalSourceSha256: sha256(Buffer.from(fixture.canonicalSource ?? fixture.source, "utf8")),
      sourceAdapterPolicy:
        fixture.fenceLanguage === "zenuml" ? "zenuml-prefix-v1" : "mermaid-fence-lines-v1",
      sourceMarkdownSha256:
        fixture.markdown === undefined ? null : sha256(Buffer.from(fixture.markdown, "utf8")),
    };
  }

  private renderSvg(page: PageHandle, fixture: RenderFixture): Promise<MermaidSvgStages> {
    if (this.options.referenceMode === "strictVendorOriginal") {
      return this.renderVendorOriginalSvg(page, fixture);
    }
    const input = {
      ...fixture,
      originalSource: fixture.source,
      source: OfficialSourceNormalizer.normalize(fixture.source),
    };
    return page.evaluate(
      async ({ config, input, request }) => {
        const i18nWindow = window as MermaidI18nWindow;
        const requireMermaid = (runtime: MermaidI18nWindow) => {
          if (runtime.mermaid === undefined) {
            throw new Error("Mermaid runtime was not registered");
          }
          return runtime.mermaid;
        };
        const requireDiagramType = (runtime: MermaidI18nWindow) => {
          if (runtime.katanaMermaidDiagramType === undefined) {
            throw new Error("Mermaid diagram-type helper was not registered");
          }
          return runtime.katanaMermaidDiagramType;
        };
        const requireNormalizer = (runtime: MermaidI18nWindow) => {
          if (runtime.katanaNormalizeMermaidSourceI18n === undefined) {
            throw new Error("Mermaid i18n normalizer was not registered");
          }
          return runtime.katanaNormalizeMermaidSourceI18n;
        };
        const requireRestore = (runtime: MermaidI18nWindow) => {
          if (runtime.katanaRestoreMermaidI18nText === undefined) {
            throw new Error("Mermaid i18n restore helper was not registered");
          }
          return runtime.katanaRestoreMermaidI18nText;
        };
        const requireSvgNormalizer = (runtime: MermaidI18nWindow) => {
          if (runtime.katanaNormalizeMermaidSvg === undefined) {
            throw new Error("Mermaid SVG normalizer was not registered");
          }
          return runtime.katanaNormalizeMermaidSvg;
        };
        const normalized = requireNormalizer(i18nWindow)(input.source);
        const mermaidValue = requireMermaid(i18nWindow);
        mermaidValue.initialize(config);
        if (requireDiagramType(i18nWindow)(normalized.source) === "zenuml") {
          if (i18nWindow.__katanaMermaidZenuml === undefined) {
            throw new Error("ZenUML runtime asset was not registered");
          }
          await mermaidValue.registerExternalDiagrams([i18nWindow.__katanaMermaidZenuml]);
        }
        const diagramType = requireDiagramType(i18nWindow)(normalized.source);
        return mermaidValue.render(`official-${input.slug}`, normalized.source).then((result) => {
          const restored = requireRestore(i18nWindow)(result.svg, normalized.replacements);
          const normalizedSvg = requireSvgNormalizer(i18nWindow)(restored, {
            ...request,
            source: normalized.source,
          });
          if (diagramType === "classdiagram" && normalized.source !== input.source) {
            throw new Error("classDiagram source normalization changed the render input");
          }
          return {
            diagramType,
            rawSvg: result.svg,
            restoredSvg: restored,
            normalizedSvg,
            originalSource: input.originalSource,
            preprocessedSource: input.source,
            renderSource: normalized.source,
            replacements: normalized.replacements,
            mermaidVersion: mermaidValue.version ?? null,
          };
        });
      },
      {
        config: this.mermaidConfig(),
        input,
        request: this.normalizationRequest(),
      },
    );
  }

  private renderVendorOriginalSvg(
    page: PageHandle,
    fixture: RenderFixture,
  ): Promise<MermaidSvgStages> {
    const source = fixture.originalFenceSource;
    if (source === undefined) throw new Error(`Missing original fence bytes: ${fixture.slug}`);
    const canonicalSource = fixture.canonicalSource;
    if (canonicalSource === undefined) throw new Error(`Missing canonical source: ${fixture.slug}`);
    return page.evaluate(
      async ({ config, input }) => {
        const runtime = window as MermaidI18nWindow;
        const mermaid = runtime.mermaid;
        if (mermaid === undefined) throw new Error("Vendor Mermaid runtime was not registered");
        const zenuml = runtime["mermaid-zenuml"];
        if (zenuml === undefined) throw new Error("Vendor ZenUML runtime was not registered");
        mermaid.initialize(config);
        await mermaid.registerExternalDiagrams([zenuml]);
        const result = await mermaid.render(`official-${input.slug}`, input.source);
        const diagramType =
          result.diagramType.toLowerCase() === "class" ? "classDiagram" : result.diagramType;
        return {
          diagramType,
          rawSvg: result.svg,
          restoredSvg: result.svg,
          normalizedSvg: result.svg,
          originalSource: input.source,
          preprocessedSource: input.source,
          renderSource: input.source,
          replacements: [],
          mermaidVersion: mermaid.version ?? null,
        };
      },
      { config: this.mermaidConfig(), input: { slug: fixture.slug, source: canonicalSource } },
    );
  }

  private async setPrimarySvg(page: PageHandle, svg: string) {
    return page.evaluate(
      ({ value }) => {
        const diagramElement = document.getElementById("diagram") as HTMLElement;
        diagramElement.innerHTML = value;
      },
      { value: svg },
    );
  }

  private normalizationRequest(): MermaidNormalizationRequest {
    return {
      theme: this.theme.mermaidTheme,
      text: this.theme.text,
      fill: this.theme.getFill(),
      stroke: this.theme.getStroke(),
      arrow: this.theme.getArrow(),
      background: "transparent",
      source: "",
    };
  }

  private mermaidConfig(): MermaidConfig {
    return {
      startOnLoad: false,
      securityLevel: "loose",
      htmlLabels: false,
      theme: this.theme.mermaidTheme,
      flowchart: { htmlLabels: false, useMaxWidth: false },
      sequence: { useMaxWidth: false },
      themeVariables: this.theme.variables(),
    };
  }

  private resizeCapture(page: PageHandle, strict: boolean): Promise<void> {
    return resizeSvgCapture(page, strict, "officialFallback");
  }

  private waitForSvgImages(page: PageHandle): Promise<void> {
    return waitForStrictSvgExternalImages(page);
  }

  private currentBrowser(): BrowserHandle {
    if (this.browser === null) {
      throw new Error("Official Mermaid renderer is not started");
    }
    return this.browser;
  }

  baseHtml() {
    return `<!doctype html><html><head><meta charset="utf-8"><style>
html,body{margin:0;background:${this.theme.canvasBackground};color:${this.theme.text};color-scheme:${this.theme.colorScheme()};font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}
#capture{display:flex;align-items:center;justify-content:center;overflow:hidden;padding:12px;box-sizing:border-box;background:${this.theme.canvasBackground}}
#diagram{max-width:100%;max-height:100%;display:flex;align-items:center;justify-content:center}
#diagram svg{max-width:100%;max-height:100%}
</style></head><body><div id="capture"><div id="diagram"></div></div></body></html>`;
  }
}

export async function installSvgTransformBoundsBridge(page: PageHandle): Promise<void> {
  await page.evaluate(() => {
    const host = window as MermaidI18nWindow;
    if (host.__katanaTransformSvgBox !== undefined) {
      throw new Error("SVG transform bounds bridge is already registered");
    }
    host.__katanaTransformSvgBox = (transform, x, y, width, height) => {
      if (![x, y, width, height].every(Number.isFinite) || width < 0 || height < 0) {
        throw new Error("SVG transform bounds input must be finite and non-negative");
      }
      const element = document.createElementNS("http://www.w3.org/2000/svg", "g");
      element.setAttribute("transform", transform);
      const transformList = element.transform.baseVal;
      if (transformList.numberOfItems === 0) return { x, y, width, height };
      const consolidated = transformList.consolidate();
      if (consolidated === null) return { x, y, width, height };
      const sourceMatrix = consolidated.matrix;
      const matrix = new DOMMatrix([
        sourceMatrix.a,
        sourceMatrix.b,
        sourceMatrix.c,
        sourceMatrix.d,
        sourceMatrix.e,
        sourceMatrix.f,
      ]);
      const points = [
        matrix.transformPoint(new DOMPoint(x, y)),
        matrix.transformPoint(new DOMPoint(x + width, y)),
        matrix.transformPoint(new DOMPoint(x, y + height)),
        matrix.transformPoint(new DOMPoint(x + width, y + height)),
      ];
      const coordinates = points.flatMap((point) => [point.x, point.y]);
      if (!coordinates.every(Number.isFinite)) {
        throw new Error("SVG transformed bounds must be finite");
      }
      const left = Math.min(...points.map((point) => point.x));
      const top = Math.min(...points.map((point) => point.y));
      const right = Math.max(...points.map((point) => point.x));
      const bottom = Math.max(...points.map((point) => point.y));
      return { x: left, y: top, width: right - left, height: bottom - top };
    };
  });
}

function officialRuntimeAssets(document: JsonValue): Array<{ path: string; sha256: string }> {
  const root = jsonObject(document, "official renderer provenance");
  const rendererAssets = root.rendererAssets;
  if (rendererAssets === undefined)
    throw new Error("Official provenance is missing rendererAssets");
  const assets = jsonObject(rendererAssets, "official renderer asset bindings");
  const mermaidAsset = assets.mermaid;
  const zenumlAsset = assets.zenuml;
  if (mermaidAsset === undefined || zenumlAsset === undefined) {
    throw new Error("Official provenance is missing renderer runtime asset bindings");
  }
  const records: JsonValue[] = [mermaidAsset, zenumlAsset];
  for (const key of ["runtimeScripts", "configSourceFiles"] as const) {
    const collection = assets[key];
    if (!Array.isArray(collection)) throw new Error(`Official provenance is missing ${key}`);
    records.push(...collection);
  }
  return records.map((record) => {
    const binding = jsonObject(record, "official renderer asset binding");
    if (typeof binding.path !== "string" || typeof binding.sha256 !== "string") {
      throw new Error("Official renderer asset binding is incomplete");
    }
    return { path: binding.path, sha256: binding.sha256 };
  });
}

function jsonObject(value: JsonValue | undefined, label: string): { [key: string]: JsonValue } {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`${label} is not an object`);
  }
  return value;
}
