import { execSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
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
  assertLoadedSvgExternalImages,
  strictSvgExternalImageUrls,
} from "./strict-svg-external-images";

export { expandHome } from "./official-renderer-i18n";

export class PlaywrightLoader {
  async load(): Promise<PlaywrightModule> {
    try {
      return (await import("playwright")) as PlaywrightModule;
    } catch {
      const cliPath = this.findPlaywrightCli();
      const realPath = fs.realpathSync(cliPath);
      const modulePath = path.join(path.dirname(realPath), "index.mjs");
      return import(pathToFileURL(modulePath).href) as Promise<PlaywrightModule>;
    }
  }

  findPlaywrightCli() {
    try {
      return execSync("which playwright", { encoding: "utf8" }).trim();
    } catch {
      throw new Error(
        "playwright command not found. Install it, then run `just mermaid-diagram-browser-install`.",
      );
    }
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
}

export class OfficialMermaidRenderer {
  private browser: BrowserHandle | null = null;
  private options: RendererOptions;
  private theme: DiagramTheme;

  constructor(options: RendererOptions) {
    this.options = options;
    this.theme = DiagramTheme.parse(options.theme);
  }

  async start() {
    const { chromium } = await new PlaywrightLoader().load();
    this.browser = await chromium
      .launch({ headless: true })
      .catch((error) => this.rethrowLaunchError(error));
  }

  async stop() {
    await this.browser?.close();
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
    await this.setPrimarySvg(page, primarySvg);
    await this.capture(
      page,
      fixture,
      primarySvg,
      strict || this.isClassDiagram(stages.diagramType),
      provenancePath,
      strict,
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
      await page.locator("#capture").screenshot({
        path: pngPath,
        omitBackground: false,
      });
      const sourceSvg = Buffer.from(preserveSvgBytes ? svg : svg.replace(/[ \t]+$/gm, ""), "utf8");
      const renderedSvg = Buffer.from(svg, "utf8");
      const provenanceDocument = parseJsonValue(
        fs.readFileSync(provenancePath, "utf8"),
        "official renderer provenance",
      );
      publishMermaidPair({
        directory: this.options.outputDir,
        attemptDirectory: attemptDir,
        slug: fixture.slug,
        role: "official",
        sourceSvg,
        renderedSvg,
        png: fs.readFileSync(pngPath),
        provenance: {
          path: "embedded:official-renderer-provenance",
          sha256: sha256(Buffer.from(JSON.stringify(provenanceDocument))),
          document: provenanceDocument,
        },
        config: {
          theme: this.theme.name,
          viewport: { width: 1520, height: 845, deviceScaleFactor: 1 },
          captureResize: strict
            ? "capture-box-only-raw-svg-v1"
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
          canonicalSourceSha256: sha256(
            Buffer.from(fixture.canonicalSource ?? fixture.source, "utf8"),
          ),
          sourceAdapterPolicy:
            fixture.fenceLanguage === "zenuml" ? "zenuml-prefix-v1" : "mermaid-fence-lines-v1",
          sourceMarkdownSha256:
            fixture.markdown === undefined ? null : sha256(Buffer.from(fixture.markdown, "utf8")),
        },
        runtimeAssets: officialRuntimeAssets(provenanceDocument),
      });
      published = true;
    } catch (error) {
      const detail = error instanceof Error ? error.message : "unclassified capture failure";
      throw new Error(`Official capture failed; attempt retained at ${attemptDir}: ${detail}`);
    } finally {
      if (published) fs.rmSync(attemptDir, { recursive: true, force: true });
    }
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
    await page.evaluate(
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
    return page.evaluate(
      ({ strict }) => {
        const svgElement = document.querySelector("#diagram svg") as SVGSVGElement;
        const viewBox = String(svgElement.getAttribute("viewBox") ?? "")
          .split(/\s+/)
          .map((value) => Number(value));
        const dimensionAt = (values: number[], index: number): number => {
          const value = values.at(index);
          return value === undefined || !Number.isFinite(value) ? 0 : value;
        };
        const attributeDimension = (name: string): number => {
          const value = String(svgElement.getAttribute(name) ?? "").replace(/px$/, "");
          const parsed = Number(value);
          return Number.isFinite(parsed) ? parsed : 0;
        };
        const box = svgElement.getBoundingClientRect();
        const positiveDimension = (values: number[]): number =>
          values.find((value) => value > 0) ?? 1;
        const width = Math.ceil(
          positiveDimension([dimensionAt(viewBox, 2), attributeDimension("width"), box.width]),
        );
        const height = Math.ceil(
          positiveDimension([dimensionAt(viewBox, 3), attributeDimension("height"), box.height]),
        );
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
  }

  private waitForSvgImages(page: PageHandle): Promise<void> {
    return (async () => {
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
    })();
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
