import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { DiagramTheme, type DiagramThemeName } from "./diagram_theme";
import { fileSha256 } from "./mermaid-pair-manifest";
import { PlaywrightLoader } from "./official-renderer";
import type {
  BrowserHandle,
  BrowserLauncher,
  FontReadyDocument,
  PageHandle,
} from "./official-renderer-types";
import { assignStrictSvg, resizeSvgCapture } from "./raster-capture-observation";
import { rasterCaptureHtml } from "./rasterize-svg-dir";
import { waitForStrictSvgExternalImages } from "./strict-svg-external-images";

export interface NeutralRasterInput {
  slug: string;
  role: "official" | "candidate";
  svgPath: string;
  svg: string;
  svgSha256: string;
  sourcePngPath: string;
  sourcePngSha256: string;
  outputSvgPath: string;
  outputPngPath: string;
}

export interface NeutralRasterRecord {
  slug: string;
  role: "official" | "candidate";
  svgPath: string;
  svgSha256: string;
  sourcePngPath: string;
  sourcePngSha256: string;
  neutralSvgPath: string;
  neutralSvgSha256: string;
  neutralPngPath: string;
  neutralPngSha256: string;
  width: number;
  height: number;
  graphicsElementCount: number;
  paintVisibilitySha256: string;
}

export interface NeutralRasterBatch {
  policy: "browser-neutral-paint-v2";
  browserVersion: string;
  browserExecutablePath: string;
  browserExecutableSha256: string;
  viewport: { width: number; height: number; deviceScaleFactor: number };
  records: NeutralRasterRecord[];
}

export interface PaintProjectedReferenceInput {
  slug: string;
  officialSvgPath: string;
  officialSvg: string;
  officialSvgSha256: string;
  officialPngPath: string;
  officialPngSha256: string;
  candidateSvgPath: string;
  candidateSvg: string;
  candidateSvgSha256: string;
  candidatePngPath: string;
  candidatePngSha256: string;
  outputSvgPath: string;
  outputPngPath: string;
}

export interface PaintProjectedReferenceRecord {
  slug: string;
  officialSvgPath: string;
  officialSvgSha256: string;
  officialPngPath: string;
  officialPngSha256: string;
  candidateSvgPath: string;
  candidateSvgSha256: string;
  candidatePngPath: string;
  candidatePngSha256: string;
  projectedReferenceSvgPath: string;
  projectedReferenceSvgSha256: string;
  projectedReferencePngPath: string;
  projectedReferencePngSha256: string;
  width: number;
  height: number;
  graphicsElementCount: number;
  officialPaintVisibilitySha256: string;
  paintVisibilitySha256: string;
}

export interface PaintProjectedReferenceBatch {
  policy: "browser-candidate-paint-projected-onto-official-geometry-v1";
  browserVersion: string;
  browserExecutablePath: string;
  browserExecutableSha256: string;
  viewport: { width: number; height: number; deviceScaleFactor: number };
  records: PaintProjectedReferenceRecord[];
}

interface PaintElementSnapshot {
  localName: string;
  id: string;
  fill: string;
  stroke: string;
  color: string;
  stopColor: string;
  floodColor: string;
  lightingColor: string;
}

interface CandidatePaintSnapshot {
  elements: PaintElementSnapshot[];
  visibility: string;
}

type PaintProjectionPageRequest =
  | { operation: "capture" }
  | { operation: "apply"; candidate: CandidatePaintSnapshot };

type PaintProjectionPageResult =
  | CandidatePaintSnapshot
  | { svg: string; graphicsElementCount: number; paintVisibility: string };

interface HeadlessBrowserServer {
  process(): { spawnfile?: string };
  wsEndpoint(): string;
  close(): Promise<void>;
}

interface HeadlessBrowserLauncher {
  launchServer(options: { headless: boolean }): Promise<HeadlessBrowserServer>;
  connect(options: { wsEndpoint: string }): Promise<BrowserHandle>;
}

export async function rasterizeNeutralSvgBatch(
  inputs: NeutralRasterInput[],
  outputDirectory: string,
  themeName: DiagramThemeName,
): Promise<NeutralRasterBatch> {
  assertFreshDirectory(outputDirectory, inputs);
  const playwright = await new PlaywrightLoader().load();
  const session = await launchHeadlessBrowser(playwright.chromium);
  try {
    return await renderBatch(inputs, themeName, session.browser, session.executablePath);
  } finally {
    await session.close();
  }
}

export async function rasterizePaintProjectedReferenceBatch(
  inputs: PaintProjectedReferenceInput[],
  outputDirectory: string,
  themeName: DiagramThemeName,
): Promise<PaintProjectedReferenceBatch> {
  assertFreshPaintProjectedDirectory(outputDirectory, inputs);
  const playwright = await new PlaywrightLoader().load();
  const session = await launchHeadlessBrowser(playwright.chromium);
  try {
    return await renderPaintProjectedBatch(
      inputs,
      themeName,
      session.browser,
      session.executablePath,
    );
  } finally {
    await session.close();
  }
}

async function launchHeadlessBrowser(chromium: BrowserLauncher) {
  if (!hasHeadlessBrowserLauncher(chromium)) {
    throw new Error("Launched Chromium process identity API is unavailable");
  }
  const server = await chromium.launchServer({ headless: true });
  try {
    const spawnedExecutable = server.process().spawnfile;
    if (spawnedExecutable === undefined || !path.isAbsolute(spawnedExecutable)) {
      throw new Error("Launched Chromium process executable path is unavailable");
    }
    const executablePath = fs.realpathSync(spawnedExecutable);
    const browser = await chromium.connect({ wsEndpoint: server.wsEndpoint() });
    return {
      browser,
      executablePath,
      close: async () => {
        try {
          await browser.close();
        } finally {
          await server.close();
        }
      },
    };
  } catch (error) {
    await server.close();
    throw error;
  }
}

function hasHeadlessBrowserLauncher(
  chromium: BrowserLauncher,
): chromium is BrowserLauncher & HeadlessBrowserLauncher {
  const candidate: unknown = chromium;
  if (typeof candidate !== "object" || candidate === null) return false;
  const launchApi = (candidate as { launchServer?: unknown }).launchServer;
  const connectApi = (candidate as { connect?: unknown }).connect;
  return typeof launchApi === "function" && typeof connectApi === "function";
}

async function renderPaintProjectedBatch(
  inputs: PaintProjectedReferenceInput[],
  themeName: DiagramThemeName,
  browser: BrowserHandle,
  browserExecutablePath: string,
): Promise<PaintProjectedReferenceBatch> {
  const browserVersion = browser.version?.();
  if (browserVersion === undefined || browserVersion.trim() === "") {
    throw new Error("Paint projected reference browser version is unavailable");
  }
  const records: PaintProjectedReferenceRecord[] = [];
  for (const input of inputs) {
    records.push(await renderPaintProjectedOne(input, browser, themeName));
  }
  return {
    policy: "browser-candidate-paint-projected-onto-official-geometry-v1",
    browserVersion,
    browserExecutablePath,
    browserExecutableSha256: fileSha256(browserExecutablePath),
    viewport: { width: 1520, height: 845, deviceScaleFactor: 1 },
    records,
  };
}

async function renderPaintProjectedOne(
  input: PaintProjectedReferenceInput,
  browser: BrowserHandle,
  themeName: DiagramThemeName,
): Promise<PaintProjectedReferenceRecord> {
  assertPaintProjectedSourcePins(input);
  const page = await browser.newPage({
    viewport: { width: 1520, height: 845 },
    deviceScaleFactor: 1,
  });
  try {
    await page.setContent(rasterCaptureHtml(DiagramTheme.parse(themeName)), { waitUntil: "load" });
    await assignStrictSvg(page, input.candidateSvg);
    await resizeSvgCapture(page, true, "strictViewBox");
    await waitForStrictSvgExternalImages(page);
    await page.evaluate(() => (document as FontReadyDocument).fonts.ready);
    const candidatePaint = await captureComputedPaint(page);
    await assignStrictSvg(page, input.officialSvg);
    await resizeSvgCapture(page, true, "strictViewBox");
    await waitForStrictSvgExternalImages(page);
    await page.evaluate(() => (document as FontReadyDocument).fonts.ready);
    const officialPaintVisibility = (await captureComputedPaint(page)).visibility;
    const projection = await applyComputedPaint(page, candidatePaint);
    writeNewFile(input.outputSvgPath, projection.svg);
    const png = await page.locator("#capture").screenshot({
      path: input.outputPngPath,
      omitBackground: false,
    });
    return makePaintProjectedRecord(input, projection, officialPaintVisibility, png);
  } finally {
    await page.close();
  }
}

async function captureComputedPaint(page: PageHandle): Promise<CandidatePaintSnapshot> {
  const result = await page.evaluate(paintProjectionPageOperation, { operation: "capture" });
  if ("elements" in result) return result;
  throw new Error("Paint projection capture returned an apply result");
}

async function applyComputedPaint(
  page: PageHandle,
  candidate: CandidatePaintSnapshot,
): Promise<{ svg: string; graphicsElementCount: number; paintVisibility: string }> {
  const result = await page.evaluate(paintProjectionPageOperation, {
    operation: "apply",
    candidate,
  });
  if ("svg" in result) return result;
  throw new Error("Paint projection apply returned a capture result");
}

function paintProjectionPageOperation(
  request: PaintProjectionPageRequest,
): PaintProjectionPageResult {
  const svg = document.querySelector<SVGSVGElement>("#diagram svg");
  if (svg === null) throw new Error("Paint projection SVG is missing");
  assertSupportedSvg(svg);
  const elements = svgElements(svg);
  if (request.operation === "capture") {
    return {
      elements: elements.map((element) => paintSnapshot(element)),
      visibility: visibilitySignature(elements),
    };
  }
  assertSameInventory(elements, request.candidate.elements);
  const idMap = buildIdMap(request.candidate.elements, elements);
  for (const [index, element] of elements.entries()) {
    const snapshot = request.candidate.elements[index];
    if (snapshot === undefined || snapshot.localName !== element.localName) {
      throw new Error("Paint projection source element order differs");
    }
    applyStyle(element, snapshot, idMap);
  }
  const paintVisibility = visibilitySignature(elements);
  if (paintVisibility !== request.candidate.visibility) {
    throw new Error("Paint projection changed computed visibility or alpha");
  }
  return { svg: svg.outerHTML, graphicsElementCount: elements.length, paintVisibility };

  function assertSupportedSvg(root: SVGSVGElement) {
    if (root.querySelector("image,foreignObject") !== null) {
      throw new Error("Paint projection does not support image or foreignObject content");
    }
  }

  function paintSnapshot(element: SVGElement): PaintElementSnapshot {
    const style = getComputedStyle(element);
    return {
      localName: element.localName,
      id: element.getAttribute("id") ?? "",
      fill: style.fill,
      stroke: style.stroke,
      color: style.color,
      stopColor: style.stopColor,
      floodColor: style.floodColor,
      lightingColor: style.lightingColor,
    };
  }

  function assertSameInventory(target: SVGElement[], source: PaintElementSnapshot[]) {
    if (target.length !== source.length) {
      throw new Error("Paint projection source element inventories differ");
    }
  }

  function visibilitySignature(items: SVGElement[]): string {
    return JSON.stringify(items.map((element) => visibilityRecord(element)));
  }

  function visibilityRecord(element: SVGElement): unknown[] {
    const style = getComputedStyle(element);
    return [
      element.localName,
      style.opacity,
      style.fillOpacity,
      style.strokeOpacity,
      style.display,
      style.visibility,
      paintAlpha(element, style.fill),
      paintAlpha(element, style.stroke),
      element.localName === "stop" ? colorAlpha(style.stopColor) : "",
      element.localName === "stop" ? style.stopOpacity : "",
    ];
  }

  function paintAlpha(element: SVGElement, value: string): string {
    if (value.trim().toLowerCase() === "none") return "none";
    const id = paintReferenceId(value);
    return id === null ? `solid/${colorAlpha(value)}` : gradientAlpha(element, id);
  }

  function paintReferenceId(value: string): string | null {
    if (!value.trim().toLowerCase().startsWith("url(")) return null;
    const match = value.trim().match(/^url\(["']?#([^"')]+)["']?\)$/);
    if (match === null || match[1] === undefined) {
      throw new Error(`Paint projection requires a local reference: ${value}`);
    }
    return match[1];
  }

  function gradientAlpha(element: SVGElement, id: string): string {
    const svgRoot = element.ownerSVGElement ?? (element instanceof SVGSVGElement ? element : null);
    if (svgRoot === null) throw new Error("Paint projection URL is invalid");
    const gradient = Array.from(
      svgRoot.querySelectorAll<SVGGradientElement>("linearGradient,radialGradient"),
    ).find((item) => item.id === id);
    if (gradient === undefined) throw new Error(`Paint projection reference is unresolved: ${id}`);
    const stops = Array.from(gradient.querySelectorAll<SVGStopElement>("stop"));
    if (stops.length === 0) throw new Error(`Paint projection gradient has no stops: ${id}`);
    return `gradient/${JSON.stringify(
      stops.map((stop) => {
        const style = getComputedStyle(stop);
        return [stop.getAttribute("offset") ?? "0", colorAlpha(style.stopColor), style.stopOpacity];
      }),
    )}`;
  }

  function colorAlpha(value: string): string {
    const normalized = value.trim().toLowerCase();
    if (normalized === "transparent") return "0";
    const match = normalized.match(/^rgba?\((.*)\)$/);
    if (match === null) throw new Error(`Paint projection color is unsupported: ${value}`);
    return alphaFromColorBody(match[1] ?? "", value);
  }

  function alphaFromColorBody(body: string, value: string): string {
    const parts = body.includes(",")
        ? body.split(",").map((part) => part.trim())
        : body.replace("/", " ").trim().split(/\s+/),
      alphaText = parts[3] ?? "1",
      alpha = Number.parseFloat(alphaText.replace(/%$/, ""));
    if ((parts.length !== 3 && parts.length !== 4) || !Number.isFinite(alpha)) {
      throw new Error(`Paint projection color alpha is invalid: ${value}`);
    }
    return String(Math.min(1, Math.max(0, alphaText.endsWith("%") ? alpha / 100 : alpha)));
  }

  function svgElements(root: SVGSVGElement): SVGElement[] {
    return [root, ...Array.from(root.querySelectorAll<SVGElement>("*"))];
  }

  function buildIdMap(source: PaintElementSnapshot[], target: SVGElement[]): Map<string, string> {
    const result = new Map<string, string>(),
      targetToSource = new Map<string, string>();
    for (const [index, sourceElement] of source.entries()) {
      if (sourceElement.id === "") continue;
      recordIdMapping(sourceElement.id, target[index]?.getAttribute("id"));
    }
    return result;

    function recordIdMapping(sourceId: string, targetId: string | null | undefined): void {
      if (targetId === null || targetId === undefined || targetId === "") {
        throw new Error("Paint projection cannot map an SVG reference ID");
      }
      const mappedTarget = result.get(sourceId),
        mappedSource = targetToSource.get(targetId);
      if (mappedTarget !== undefined && mappedTarget !== targetId) {
        throw new Error("Paint projection cannot map an SVG reference ID");
      }
      if (mappedSource !== undefined && mappedSource !== sourceId) {
        throw new Error("Paint projection cannot map an SVG reference ID");
      }
      if (mappedTarget !== undefined) return;
      targetToSource.set(targetId, sourceId);
      result.set(sourceId, targetId);
    }
  }

  function applyStyle(
    element: SVGElement,
    snapshot: PaintElementSnapshot,
    idMap: Map<string, string>,
  ) {
    const properties = [
      ["fill", snapshot.fill],
      ["stroke", snapshot.stroke],
      ["color", snapshot.color],
      ["stop-color", snapshot.stopColor],
      ["flood-color", snapshot.floodColor],
      ["lighting-color", snapshot.lightingColor],
    ] as const;
    for (const [property, value] of properties) {
      if (value.trim() === "") throw new Error(`Paint projection ${property} is unavailable`);
      element.style.setProperty(property, remapReference(value, idMap), "important");
    }
  }

  function remapReference(value: string, idMap: Map<string, string>): string {
    const trimmed = value.trim();
    if (!trimmed.toLowerCase().startsWith("url(")) return trimmed;
    const match = trimmed.match(/^url\(["']?#([^"')]+)["']?\)$/);
    if (match === null) throw new Error(`Paint projection requires a local reference: ${value}`);
    const id = match[1],
      targetId = id === undefined ? undefined : idMap.get(id);
    if (targetId === undefined)
      throw new Error(`Paint projection reference is unresolved: ${value}`);
    return `url(#${targetId})`;
  }
}

function assertFreshPaintProjectedDirectory(
  outputDirectory: string,
  inputs: PaintProjectedReferenceInput[],
) {
  const resolved = path.resolve(outputDirectory);
  if (!path.isAbsolute(resolved) || fs.existsSync(resolved)) {
    throw new Error("Paint projected reference output must be a fresh owned directory");
  }
  if (
    inputs.length === 0 ||
    new Set(inputs.map((input) => input.slug)).size !== inputs.length ||
    new Set(inputs.map((input) => input.outputPngPath)).size !== inputs.length ||
    new Set(inputs.map((input) => input.outputSvgPath)).size !== inputs.length
  ) {
    throw new Error("Paint projected reference inventory is empty or duplicated");
  }
  fs.mkdirSync(resolved, { recursive: false, mode: 0o700 });
  for (const input of inputs) {
    if (
      path.dirname(path.resolve(input.outputPngPath)) !== resolved ||
      path.basename(input.outputPngPath) !== `${input.slug}-reference.png` ||
      path.dirname(path.resolve(input.outputSvgPath)) !== resolved ||
      path.basename(input.outputSvgPath) !== `${input.slug}-reference.svg`
    ) {
      throw new Error("Paint projected reference files must use their owned deterministic paths");
    }
  }
}

function assertPaintProjectedSourcePins(input: PaintProjectedReferenceInput) {
  const sources = [
    [input.officialSvgPath, input.officialSvgSha256],
    [input.officialPngPath, input.officialPngSha256],
    [input.candidateSvgPath, input.candidateSvgSha256],
    [input.candidatePngPath, input.candidatePngSha256],
  ] as const;
  for (const [sourcePath, expectedSha] of sources) {
    if (fs.lstatSync(sourcePath).isSymbolicLink() || fileSha256(sourcePath) !== expectedSha) {
      throw new Error(`Paint projected source pin differs: ${input.slug}/${sourcePath}`);
    }
  }
}

function makePaintProjectedRecord(
  input: PaintProjectedReferenceInput,
  projection: { svg: string; graphicsElementCount: number; paintVisibility: string },
  officialPaintVisibility: string,
  png: Buffer,
): PaintProjectedReferenceRecord {
  assertPaintProjectedSourcePins(input);
  assertDerivedPins(input, projection.svg, png);
  const size = pngDimensions(png);
  return {
    slug: input.slug,
    officialSvgPath: path.resolve(input.officialSvgPath),
    officialSvgSha256: input.officialSvgSha256,
    officialPngPath: path.resolve(input.officialPngPath),
    officialPngSha256: input.officialPngSha256,
    candidateSvgPath: path.resolve(input.candidateSvgPath),
    candidateSvgSha256: input.candidateSvgSha256,
    candidatePngPath: path.resolve(input.candidatePngPath),
    candidatePngSha256: input.candidatePngSha256,
    projectedReferenceSvgPath: path.resolve(input.outputSvgPath),
    projectedReferenceSvgSha256: digest(Buffer.from(projection.svg)),
    projectedReferencePngPath: path.resolve(input.outputPngPath),
    projectedReferencePngSha256: digest(png),
    width: size.width,
    height: size.height,
    graphicsElementCount: projection.graphicsElementCount,
    officialPaintVisibilitySha256: digest(Buffer.from(officialPaintVisibility)),
    paintVisibilitySha256: digest(Buffer.from(projection.paintVisibility)),
  };
}

function assertDerivedPins(input: PaintProjectedReferenceInput, svg: string, png: Buffer) {
  if (
    fs.lstatSync(input.outputSvgPath).isSymbolicLink() ||
    fs.lstatSync(input.outputPngPath).isSymbolicLink() ||
    fileSha256(input.outputSvgPath) !== digest(Buffer.from(svg)) ||
    fileSha256(input.outputPngPath) !== digest(png)
  ) {
    throw new Error(`Paint projected output pin differs: ${input.slug}`);
  }
}

async function renderBatch(
  inputs: NeutralRasterInput[],
  themeName: DiagramThemeName,
  browser: BrowserHandle,
  browserExecutablePath: string,
): Promise<NeutralRasterBatch> {
  const browserVersion = browser.version?.();
  if (browserVersion === undefined || browserVersion.trim() === "") {
    throw new Error("Neutral raster browser version is unavailable");
  }
  const records: NeutralRasterRecord[] = [];
  for (const input of inputs) {
    records.push(await renderOne(input, browser, themeName));
  }
  return {
    policy: "browser-neutral-paint-v2",
    browserVersion,
    browserExecutablePath,
    browserExecutableSha256: fileSha256(browserExecutablePath),
    viewport: { width: 1520, height: 845, deviceScaleFactor: 1 },
    records,
  };
}

async function renderOne(
  input: NeutralRasterInput,
  browser: BrowserHandle,
  themeName: DiagramThemeName,
): Promise<NeutralRasterRecord> {
  assertSourcePins(input);
  const page = await browser.newPage({
    viewport: { width: 1520, height: 845 },
    deviceScaleFactor: 1,
  });
  try {
    await page.setContent(rasterCaptureHtml(DiagramTheme.parse(themeName)), { waitUntil: "load" });
    await assignStrictSvg(page, input.svg);
    await resizeSvgCapture(page, true, "strictViewBox");
    await waitForStrictSvgExternalImages(page);
    await page.evaluate(() => (document as FontReadyDocument).fonts.ready);
    const neutral = await neutralizePagePaint(page);
    writeNewFile(input.outputSvgPath, neutral.svg);
    const png = await page.locator("#capture").screenshot({
      path: input.outputPngPath,
      omitBackground: false,
    });
    return makeRecord(
      input,
      neutral.graphicsElementCount,
      neutral.paintVisibility,
      neutral.svg,
      png,
    );
  } finally {
    await page.close();
  }
}

function makeRecord(
  input: NeutralRasterInput,
  graphicsElementCount: number,
  paintVisibility: string,
  neutralSvg: string,
  png: Buffer,
): NeutralRasterRecord {
  assertSourcePins(input);
  const size = pngDimensions(png);
  return {
    slug: input.slug,
    role: input.role,
    svgPath: path.resolve(input.svgPath),
    svgSha256: input.svgSha256,
    sourcePngPath: path.resolve(input.sourcePngPath),
    sourcePngSha256: input.sourcePngSha256,
    neutralSvgPath: path.resolve(input.outputSvgPath),
    neutralSvgSha256: digest(Buffer.from(neutralSvg)),
    neutralPngPath: path.resolve(input.outputPngPath),
    neutralPngSha256: digest(png),
    width: size.width,
    height: size.height,
    graphicsElementCount,
    paintVisibilitySha256: digest(Buffer.from(paintVisibility)),
  };
}

function assertSourcePins(input: NeutralRasterInput) {
  if (
    fs.lstatSync(input.svgPath).isSymbolicLink() ||
    fs.lstatSync(input.sourcePngPath).isSymbolicLink()
  ) {
    throw new Error(`Neutral raster source must not be a symlink: ${input.slug}/${input.role}`);
  }
  if (
    fileSha256(input.svgPath) !== input.svgSha256 ||
    fileSha256(input.sourcePngPath) !== input.sourcePngSha256
  ) {
    throw new Error(`Neutral raster source changed for ${input.slug}/${input.role}`);
  }
}

async function neutralizePagePaint(page: PageHandle) {
  return page.evaluate(() => {
    const gradientColors = new Map<string, "black" | "white">();

    function assertSupportedNeutralSvg(svg: SVGSVGElement) {
      if (svg.querySelector("image, foreignObject") !== null) {
        throw new Error("Neutral paint does not support image or foreignObject content");
      }
    }

    function neutralColor(value: string, color: "black" | "white"): string {
      const normalized = value.trim().toLowerCase();
      if (normalized === "none") return "none";
      const channel = color === "black" ? 0 : 255;
      return `rgb(${channel} ${channel} ${channel} / ${computedColorAlpha(normalized, value)})`;
    }

    function computedColorAlpha(normalized: string, original: string): number {
      const match = normalized.match(/^rgba?\((.*)\)$/);
      if (match === null)
        throw new Error(`Neutral paint does not support computed color: ${original}`);
      const body = match[1] ?? "";
      const parts = body.includes(",")
        ? body.split(",").map((part) => part.trim())
        : body.replace("/", " ").trim().split(/\s+/);
      return alphaFromColorParts(parts, original);
    }

    function alphaFromColorParts(parts: string[], original: string): number {
      if (parts.length !== 3 && parts.length !== 4) {
        throw new Error(`Neutral paint computed color has invalid channels: ${original}`);
      }
      const alphaText = parts[3] ?? "1";
      const alpha = Number.parseFloat(alphaText.replace(/%$/, ""));
      if (!Number.isFinite(alpha))
        throw new Error(`Neutral paint computed alpha is invalid: ${original}`);
      return Math.min(1, Math.max(0, alphaText.endsWith("%") ? alpha / 100 : alpha));
    }

    function neutralizeElementPaint(element: SVGElement) {
      const style = getComputedStyle(element),
        isText = ["text", "tspan", "textPath"].includes(element.localName),
        fill = neutralizePaint(element, style.fill, isText ? "black" : "white"),
        stroke = neutralizePaint(element, style.stroke, "black");
      element.style.setProperty("fill", fill, "important");
      element.style.setProperty("stroke", stroke, "important");
    }

    function paintVisibilitySignature(svg: SVGSVGElement, graphics: SVGElement[]): string {
      const graphicSet = new Set<Element>(graphics);
      const elements: Element[] = [svg, ...Array.from(svg.querySelectorAll("*"))];
      return JSON.stringify(
        elements.map((element) => {
          const style = getComputedStyle(element);
          const record: Record<string, string | boolean> = {
            name: element.localName,
            graphic: graphicSet.has(element),
            opacity: style.opacity,
            fillOpacity: style.fillOpacity,
            strokeOpacity: style.strokeOpacity,
            display: style.display,
            visibility: style.visibility,
          };
          if (graphicSet.has(element)) {
            record.fill = paintAlphaSignature(element as SVGElement, style.fill);
            record.stroke = paintAlphaSignature(element as SVGElement, style.stroke);
          }
          if (element.localName === "stop") {
            record.stopColor = computedColorAlphaSignature(style.stopColor);
            record.stopOpacity = style.stopOpacity;
          }
          return record;
        }),
      );
    }

    function paintAlphaSignature(element: SVGElement, value: string): string {
      if (value.trim().toLowerCase() === "none") return "none";
      const gradientId = gradientReference(value);
      if (gradientId === null) return `solid/${computedColorAlphaSignature(value)}`;
      const svg = element.ownerSVGElement;
      if (svg === null) throw new Error("Paint reference is outside its SVG root");
      const gradient = Array.from(
        svg.querySelectorAll<SVGLinearGradientElement | SVGRadialGradientElement>(
          "linearGradient,radialGradient",
        ),
      ).find((item) => item.getAttribute("id") === gradientId);
      if (gradient === undefined) throw new Error(`Paint reference is unresolved: ${gradientId}`);
      const stops = Array.from(gradient.querySelectorAll<SVGStopElement>("stop"));
      if (stops.length === 0) throw new Error(`Paint gradient has no stops: ${gradientId}`);
      return `gradient/${JSON.stringify(
        stops.map((stop) => {
          const style = getComputedStyle(stop);
          return [
            stop.getAttribute("offset") ?? "0",
            computedColorAlphaSignature(style.stopColor),
            style.stopOpacity,
          ];
        }),
      )}`;
    }

    function computedColorAlphaSignature(value: string): string {
      const normalized = value.trim().toLowerCase();
      if (normalized === "transparent") return "0";
      const match = normalized.match(/^rgba?\((.*)\)$/);
      if (match === null)
        throw new Error(`Paint visibility has unsupported computed color: ${value}`);
      const body = match[1] ?? "";
      const parts = body.includes(",")
        ? body.split(",").map((part) => part.trim())
        : body.replace("/", " ").trim().split(/\s+/);
      return String(alphaFromColorParts(parts, value));
    }

    function neutralizePaint(element: SVGElement, value: string, color: "black" | "white"): string {
      const gradientId = gradientReference(value);
      if (gradientId !== null) {
        registerGradientColor(element.ownerSVGElement, gradientId, color);
        return value;
      }
      return neutralColor(value, color);
    }

    function gradientReference(value: string): string | null {
      const normalized = value.trim();
      if (!normalized.toLowerCase().startsWith("url(")) return null;
      const match = normalized.match(/^url\(["']?#([^"')]+)["']?\)$/);
      if (match === null) throw new Error(`Neutral paint requires a local SVG gradient: ${value}`);
      return match[1] ?? null;
    }

    function registerGradientColor(
      svg: SVGSVGElement | null,
      gradientId: string,
      color: "black" | "white",
    ) {
      if (svg === null) throw new Error("Neutral paint element is outside its SVG root");
      const gradients = Array.from(svg.querySelectorAll("linearGradient,radialGradient"));
      const gradient = gradients.find((item) => item.getAttribute("id") === gradientId);
      if (gradient === undefined || gradient.querySelectorAll("stop").length === 0) {
        throw new Error(`Neutral paint gradient is missing local stops: ${gradientId}`);
      }
      const existing = gradientColors.get(gradientId);
      if (existing !== undefined && existing !== color) {
        throw new Error(
          `Neutral paint gradient has conflicting fill and stroke uses: ${gradientId}`,
        );
      }
      gradientColors.set(gradientId, color);
    }

    function neutralizeGradientStops(svg: SVGSVGElement) {
      for (const [gradientId, color] of gradientColors) {
        const gradient = Array.from(svg.querySelectorAll("linearGradient,radialGradient")).find(
          (item) => item.getAttribute("id") === gradientId,
        );
        if (gradient === undefined)
          throw new Error(`Neutral paint gradient disappeared: ${gradientId}`);
        for (const stop of Array.from(gradient.querySelectorAll("stop"))) {
          stop.style.setProperty(
            "stop-color",
            neutralColor(getComputedStyle(stop).stopColor, color),
            "important",
          );
        }
      }
    }

    const svg = document.querySelector<SVGSVGElement>("#diagram svg");
    if (svg === null) throw new Error("Neutral paint source SVG is missing");
    assertSupportedNeutralSvg(svg);
    const selector = "path,rect,circle,ellipse,line,polyline,polygon,text,tspan,textPath,use";
    const graphics = Array.from(svg.querySelectorAll(selector));
    if (graphics.length === 0) throw new Error("Neutral paint has no supported graphics elements");
    const paintVisibility = paintVisibilitySignature(svg, graphics as SVGElement[]);
    for (const element of graphics) neutralizeElementPaint(element as SVGElement);
    neutralizeGradientStops(svg);
    const capture = document.querySelector<HTMLElement>("#capture");
    if (capture === null) throw new Error("Neutral paint capture canvas is missing");
    capture.style.backgroundColor = "#ffffff";
    return { svg: svg.outerHTML, graphicsElementCount: graphics.length, paintVisibility };
  });
}

function assertFreshDirectory(outputDirectory: string, inputs: NeutralRasterInput[]) {
  const resolved = path.resolve(outputDirectory);
  if (!path.isAbsolute(resolved) || fs.existsSync(resolved)) {
    throw new Error("Neutral derivative output must be a fresh owned directory");
  }
  if (
    inputs.length === 0 ||
    new Set(inputs.map((input) => input.outputPngPath)).size !== inputs.length ||
    new Set(inputs.map((input) => input.outputSvgPath)).size !== inputs.length ||
    new Set(inputs.map((input) => `${input.slug}/${input.role}`)).size !== inputs.length
  ) {
    throw new Error("Neutral raster input inventory is empty or duplicated");
  }
  fs.mkdirSync(resolved, { recursive: false, mode: 0o700 });
  for (const input of inputs) {
    if (
      path.dirname(path.resolve(input.outputPngPath)) !== resolved ||
      path.dirname(path.resolve(input.outputSvgPath)) !== resolved
    ) {
      throw new Error("Neutral derivative files must be direct children of their owned directory");
    }
  }
}

function writeNewFile(filePath: string, value: string) {
  fs.writeFileSync(filePath, value, { encoding: "utf8", flag: "wx", mode: 0o600 });
}

function pngDimensions(png: Buffer): { width: number; height: number } {
  if (png.length < 24 || png.subarray(0, 8).toString("hex") !== "89504e470d0a1a0a") {
    throw new Error("Neutral raster output is not a PNG");
  }
  return { width: png.readUInt32BE(16), height: png.readUInt32BE(20) };
}

function digest(value: Buffer): string {
  return createHash("sha256").update(value).digest("hex");
}
