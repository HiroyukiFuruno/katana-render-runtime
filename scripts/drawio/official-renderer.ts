import { expandHome, PlaywrightLoader } from "../mermaid/official-renderer";
import type { BrowserHandle, PageHandle } from "../mermaid/official-renderer-types";
import { OfficialDrawioResourceResolver } from "./official-resource-resolver";
import { installDrawioDeterminism } from "./official-runtime-determinism";
import { captureDrawioSvg, drawioCaptureHtml } from "./svg-capture";

export interface DrawioRendererOptions {
  outputDir: string;
  drawioJs: string;
  resourcesDir: string;
  resourceManifest: string;
}

export interface DrawioRenderFixture {
  slug: string;
  source: string;
}

interface DrawioGraph {
  getSvg(): SVGSVGElement;
}

interface DrawioViewer {
  graph: DrawioGraph;
}

interface DrawioGraphViewer {
  createViewerForElement(element: HTMLElement, callback: (viewer: DrawioViewer) => void): void;
}

interface DrawioWindow extends Window {
  GraphViewer?: DrawioGraphViewer;
}

export class OfficialDrawioRenderer {
  private browser: BrowserHandle | null = null;
  private readonly resourceResolver: OfficialDrawioResourceResolver;

  constructor(private options: DrawioRendererOptions) {
    this.resourceResolver = new OfficialDrawioResourceResolver(
      options.resourcesDir,
      options.resourceManifest,
    );
  }

  async start() {
    const { chromium } = await new PlaywrightLoader().load();
    this.browser = await chromium
      .launch({ headless: true })
      .catch((error: Error) => this.rethrowLaunchError(error));
  }

  async stop() {
    await this.browser?.close();
  }

  async render(fixture: DrawioRenderFixture) {
    const page = await this.currentBrowser().newPage({
      viewport: { width: 1520, height: 845 },
      deviceScaleFactor: 1,
    });
    try {
      await this.renderPage(page, fixture);
    } finally {
      await page.close();
    }
  }

  private async renderPage(page: PageHandle, fixture: DrawioRenderFixture) {
    await page.setContent(this.baseHtml(), { waitUntil: "load" });
    await page.evaluate(installDrawioDeterminism);
    await page.addScriptTag({ path: this.options.drawioJs });
    const source = prepareDrawioSource(fixture.source);
    const svg = this.resourceResolver.resolveSvg(await this.renderSvg(page, source));
    await this.capture(page, fixture, svg, source);
  }

  private renderSvg(page: PageHandle, source: string): Promise<string> {
    return page.evaluate(
      (input) =>
        new Promise<string>((resolve) => {
          const diagram = document.getElementById("diagram") as HTMLElement;
          const container = document.createElement("div");
          container.setAttribute("class", "mxgraph");
          container.setAttribute(
            "data-mxgraph",
            JSON.stringify({
              xml: input.source,
              toolbar: "",
              "auto-fit": false,
              "auto-crop": false,
              "auto-origin": true,
              "check-visible-state": false,
            }),
          );
          diagram.appendChild(container);
          const requireGraphViewer = (runtime: DrawioWindow): DrawioGraphViewer => {
            if (runtime.GraphViewer === undefined) {
              throw new Error("Draw.io GraphViewer was not registered");
            }
            return runtime.GraphViewer;
          };
          const graphViewer = requireGraphViewer(window as DrawioWindow);
          graphViewer.createViewerForElement(container, (viewer) => {
            const svg = viewer.graph.getSvg();
            diagram.innerHTML = new XMLSerializer().serializeToString(svg);
            resolve(diagram.innerHTML);
          });
        }),
      {
        source,
      },
    );
  }

  private async capture(
    page: PageHandle,
    fixture: DrawioRenderFixture,
    svg: string,
    source: string,
  ) {
    await captureDrawioSvg(page, {
      slug: fixture.slug,
      svg: Buffer.from(svg, "utf8"),
      sourceFonts: source,
      outputDir: this.options.outputDir,
    });
  }

  private rethrowLaunchError(error: Error): never {
    if (String(error).includes("Executable doesn't exist")) {
      throw new Error(
        "Playwright browser is missing. Run `just drawio-diagram-browser-install` first.",
      );
    }
    throw error;
  }

  private currentBrowser(): BrowserHandle {
    if (this.browser === null) {
      throw new Error("Official Draw.io renderer is not started");
    }
    return this.browser;
  }

  private baseHtml() {
    return drawioCaptureHtml();
  }
}

const DrawioSource = {
  prepare(source: string): string {
    const xml = source.trim().replace(/^<\?xml[^>]*>\s*/i, "");
    return DrawioSource.uncompressedModel(xml) ?? xml;
  },

  uncompressedModel(xml: string): string | null {
    const start = xml.indexOf("<mxGraphModel");
    const end = xml.indexOf("</mxGraphModel>");
    if (DrawioSource.hasModelRange(start, end)) {
      return xml.slice(start, end + "</mxGraphModel>".length);
    }
    return null;
  },

  hasModelRange(start: number, end: number): boolean {
    return [start >= 0, end >= start].every(Boolean);
  },
};

export function prepareDrawioSource(source: string): string {
  return DrawioSource.prepare(source);
}

export { expandHome };
