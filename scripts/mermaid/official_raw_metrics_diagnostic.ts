import { parseArgs } from "node:util";
import fs from "node:fs";
import path from "node:path";
import { DiagramTheme, type DiagramThemeName, type MermaidThemeName } from "./diagram_theme";
import { fileSha256, parseJsonValue, sha256, type JsonValue } from "./mermaid-pair-manifest";
import { referenceMode, type OfficialReferenceMode } from "./official-reference-mode";
import { OfficialRendererDeterminism } from "./official-renderer-determinism";
import { OfficialMermaidRenderer, PlaywrightLoader } from "./official-renderer";
import type { MermaidConfig, MermaidI18nWindow, PageHandle } from "./official-renderer-types";
import { canonicalRegularFile } from "./native-build-receipt";

const VIEWPORT = { width: 1520, height: 845, deviceScaleFactor: 1 } as const;
const OUTPUT_FILES = {
  svg: "raw.svg",
  bbox: "bbox.json",
  canvas: "canvas.json",
  receipt: "diagnostic.json",
} as const;

export interface RawMetricsDiagnosticOptions {
  manifestPath: string;
  outputDir: string;
}

export interface BBoxObservation {
  callIndex: number;
  tagName: string;
  id: string | null;
  className: string | null;
  transform: string | null;
  text: string | null;
  finalClassName: string | null;
  finalTransform: string | null;
  finalText: string | null;
  fontFamily: string;
  fontSize: string;
  fontWeight: string;
  localBox: { x: number; y: number; width: number; height: number } | null;
  error: string | null;
  textGeometry: SvgTextGeometryObservation[];
  textGeometryError: string | null;
}

export interface SvgLengthListObservation {
  attribute: string | null;
  items: Array<{
    value: number | null;
    valueInSpecifiedUnits: number | null;
    unitType: number | null;
  }>;
  diagnosticError: string | null;
}

export interface SvgTextGeometryObservation {
  tagName: string;
  id: string | null;
  className: string | null;
  text: string | null;
  x: SvgLengthListObservation;
  y: SvgLengthListObservation;
  dx: SvgLengthListObservation;
  dy: SvgLengthListObservation;
  startPosition: { x: number; y: number } | null;
  startPositionDiagnosticError: string | null;
  diagnosticError: string | null;
}

export interface CanvasMeasureTextObservation {
  callIndex: number;
  text: string;
  font: string;
  textAlign: string;
  textBaseline: string;
  width: number;
  actualBoundingBoxLeft: number | null;
  actualBoundingBoxRight: number | null;
  actualBoundingBoxAscent: number | null;
  actualBoundingBoxDescent: number | null;
}

export interface RawMetricsDiagnosticReceipt {
  schema: "katana-mermaid-raw-metrics-diagnostic-v1";
  diagnosticOnly: true;
  accepted: false;
  slug: string;
  sourceManifestSha256: string;
  canonicalSourceSha256: string;
  expectedPrimarySvgSha256: string;
  actualRawSvgSha256: string;
  usableForDiagnosis: boolean;
  output: {
    svg: string;
    bbox: string;
    canvas: string;
  };
  bboxCallCount: number;
  canvasMeasureTextCallCount: number;
}

interface DiagnosticManifest {
  manifestPath: string;
  slug: string;
  canonicalSource: string;
  mermaidConfig: MermaidConfig;
  theme: DiagramThemeName;
  referenceMode: OfficialReferenceMode;
  mermaidJs: string;
  mermaidZenumlJs: string;
  primarySvgSha256: string;
}

interface RawMetricsSnapshot {
  bboxCalls: BBoxObservation[];
  canvasMeasureTextCalls: CanvasMeasureTextObservation[];
}

interface BrowserBBoxObservation extends BBoxObservation {
  node: SVGGraphicsElement;
}

interface BrowserRawMetricsSnapshot {
  bboxCalls: BrowserBBoxObservation[];
  canvasMeasureTextCalls: CanvasMeasureTextObservation[];
}

interface RenderedRawSvg {
  svg: string;
  diagramType: string;
}

export function parseRawMetricsDiagnosticArgs(
  args: readonly string[],
): RawMetricsDiagnosticOptions {
  const { values, positionals } = parseArgs({
    args: [...args],
    options: {
      manifest: { type: "string" },
      "output-dir": { type: "string" },
    },
    strict: true,
    allowPositionals: false,
  });
  if (
    positionals.length !== 0 ||
    typeof values.manifest !== "string" ||
    typeof values["output-dir"] !== "string"
  ) {
    throw new Error(
      "Usage: bun run scripts/mermaid/official_raw_metrics_diagnostic.ts --manifest ABSOLUTE_MANIFEST --output-dir ABSOLUTE_OUTPUT_DIR",
    );
  }
  return {
    manifestPath: path.resolve(values.manifest),
    outputDir: path.resolve(values["output-dir"]),
  };
}

export function isRawSvgUsableForDiagnosis(
  expectedPrimarySvgSha256: string,
  actualRawSvgSha256: string,
): boolean {
  return expectedPrimarySvgSha256 === actualRawSvgSha256;
}

export function buildRawMetricsDiagnosticReceipt(input: {
  slug: string;
  sourceManifestSha256: string;
  canonicalSourceSha256: string;
  expectedPrimarySvgSha256: string;
  actualRawSvgSha256: string;
  bboxCallCount: number;
  canvasMeasureTextCallCount: number;
}): RawMetricsDiagnosticReceipt {
  return {
    schema: "katana-mermaid-raw-metrics-diagnostic-v1",
    diagnosticOnly: true,
    accepted: false,
    slug: input.slug,
    sourceManifestSha256: input.sourceManifestSha256,
    canonicalSourceSha256: input.canonicalSourceSha256,
    expectedPrimarySvgSha256: input.expectedPrimarySvgSha256,
    actualRawSvgSha256: input.actualRawSvgSha256,
    usableForDiagnosis: isRawSvgUsableForDiagnosis(
      input.expectedPrimarySvgSha256,
      input.actualRawSvgSha256,
    ),
    output: {
      svg: OUTPUT_FILES.svg,
      bbox: OUTPUT_FILES.bbox,
      canvas: OUTPUT_FILES.canvas,
    },
    bboxCallCount: input.bboxCallCount,
    canvasMeasureTextCallCount: input.canvasMeasureTextCallCount,
  };
}

export async function runRawMetricsDiagnostic(
  options: RawMetricsDiagnosticOptions,
): Promise<RawMetricsDiagnosticReceipt> {
  const manifest = readDiagnosticManifest(options.manifestPath);
  const sourceManifestSha256 = fileSha256(manifest.manifestPath);
  const renderer = new OfficialMermaidRenderer({
    outputDir: options.outputDir,
    mermaidJs: manifest.mermaidJs,
    mermaidZenumlJs: manifest.mermaidZenumlJs,
    theme: manifest.theme,
    referenceMode: manifest.referenceMode,
    referenceScope: "diagnostic",
  });
  const playwright = await new PlaywrightLoader().load();
  const browser = await playwright.chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport: VIEWPORT, deviceScaleFactor: 1 });
  try {
    await page.setContent(renderer.baseHtml(), { waitUntil: "load" });
    await OfficialRendererDeterminism.install(page);
    await page.addScriptTag({ path: manifest.mermaidJs });
    await page.addScriptTag({ path: manifest.mermaidZenumlJs });
    await installRawMetricsObserver(page);
    const rendered = await page.evaluate(renderRawMermaid, {
      config: manifest.mermaidConfig,
      slug: manifest.slug,
      source: manifest.canonicalSource,
    });
    const snapshot = await page.evaluate(readRawMetricsSnapshot);
    const rawSvg = Buffer.from(rendered.svg, "utf8");
    const actualRawSvgSha256 = sha256(rawSvg);
    const receipt = buildRawMetricsDiagnosticReceipt({
      slug: manifest.slug,
      sourceManifestSha256,
      canonicalSourceSha256: sha256(Buffer.from(manifest.canonicalSource, "utf8")),
      expectedPrimarySvgSha256: manifest.primarySvgSha256,
      actualRawSvgSha256,
      bboxCallCount: snapshot.bboxCalls.length,
      canvasMeasureTextCallCount: snapshot.canvasMeasureTextCalls.length,
    });
    writeDiagnosticOutputs(options.outputDir, rawSvg, snapshot, receipt);
    return receipt;
  } finally {
    await page.close();
    await browser.close();
  }
}

async function installRawMetricsObserver(page: PageHandle): Promise<void> {
  await page.evaluate(() => {
    const host = window as Window & {
      __katanaRawMetrics?: {
        bboxCalls: BrowserBBoxObservation[];
        canvasMeasureTextCalls: CanvasMeasureTextObservation[];
      };
    };
    if (host.__katanaRawMetrics !== undefined) {
      throw new Error("Raw metrics observer is already registered");
    }
    const bboxCalls: BrowserBBoxObservation[] = [];
    const canvasMeasureTextCalls: CanvasMeasureTextObservation[] = [];
    const originalGetBBox = SVGGraphicsElement.prototype.getBBox;
    const originalMeasureText = CanvasRenderingContext2D.prototype.measureText;
    const recordBBox = (
      element: SVGGraphicsElement,
      style: CSSStyleDeclaration,
      localBox: { x: number; y: number; width: number; height: number } | null,
      error: string | null,
      textGeometry: SvgTextGeometryObservation[],
      textGeometryError: string | null,
    ) => {
      bboxCalls.push({
        callIndex: bboxCalls.length,
        tagName: element.localName,
        id: element.getAttribute("id"),
        className: element.getAttribute("class"),
        transform: element.getAttribute("transform"),
        text: element.textContent,
        finalClassName: null,
        finalTransform: null,
        finalText: null,
        fontFamily: style.fontFamily,
        fontSize: style.fontSize,
        fontWeight: style.fontWeight,
        localBox,
        error,
        textGeometry,
        textGeometryError,
        node: element,
      });
    };
    const finiteNumber = (value: unknown): number | null =>
      typeof value === "number" && Number.isFinite(value) ? value : null;
    const readLengthList = (
      node: SVGTextPositioningElement,
      name: "x" | "y" | "dx" | "dy",
    ): SvgLengthListObservation => {
      const attribute = node.getAttribute(name);
      try {
        const animated = node[name];
        const list = animated?.baseVal;
        if (list === undefined || typeof list.numberOfItems !== "number") {
          return {
            attribute,
            items: [],
            diagnosticError: `SVGLengthList baseVal API unavailable: ${name}`,
          };
        }
        const items: SvgLengthListObservation["items"] = [];
        for (let index = 0; index < list.numberOfItems; index += 1) {
          const item = list.getItem(index);
          items.push({
            value: finiteNumber(item.value),
            valueInSpecifiedUnits: finiteNumber(item.valueInSpecifiedUnits),
            unitType: finiteNumber(item.unitType),
          });
        }
        return { attribute, items, diagnosticError: null };
      } catch (error) {
        return {
          attribute,
          items: [],
          diagnosticError: error instanceof Error ? error.message : String(error),
        };
      }
    };
    const readTextGeometry = (
      element: SVGGraphicsElement,
    ): { observations: SvgTextGeometryObservation[]; error: string | null } => {
      try {
        const nodes: SVGTextPositioningElement[] = [];
        if (element.localName === "text" || element.localName === "tspan") {
          nodes.push(element as unknown as SVGTextPositioningElement);
        }
        for (const node of element.querySelectorAll("text, tspan")) {
          nodes.push(node as SVGTextPositioningElement);
        }
        return {
          observations: nodes.map((node) => {
            const text = node.textContent;
            const textIsEmpty = text === null || text.length === 0;
            let startPosition: { x: number; y: number } | null = null;
            let startPositionDiagnosticError: string | null = null;
            if (textIsEmpty) {
              startPositionDiagnosticError = "empty text";
            } else if (typeof node.getStartPositionOfChar !== "function") {
              startPositionDiagnosticError = "getStartPositionOfChar API unavailable";
            } else {
              try {
                const point = node.getStartPositionOfChar(0);
                const x = finiteNumber(point.x);
                const y = finiteNumber(point.y);
                if (x === null || y === null) {
                  startPositionDiagnosticError =
                    "getStartPositionOfChar returned non-finite coordinates";
                } else {
                  startPosition = { x, y };
                }
              } catch (error) {
                startPositionDiagnosticError =
                  error instanceof Error ? error.message : String(error);
              }
            }
            const lengths = {
              x: readLengthList(node, "x"),
              y: readLengthList(node, "y"),
              dx: readLengthList(node, "dx"),
              dy: readLengthList(node, "dy"),
            };
            const diagnosticErrors = [
              startPositionDiagnosticError,
              lengths.x.diagnosticError,
              lengths.y.diagnosticError,
              lengths.dx.diagnosticError,
              lengths.dy.diagnosticError,
            ].filter((value): value is string => value !== null);
            return {
              tagName: node.localName,
              id: node.getAttribute("id"),
              className: node.getAttribute("class"),
              text,
              ...lengths,
              startPosition,
              startPositionDiagnosticError,
              diagnosticError:
                diagnosticErrors.length === 0 ? null : diagnosticErrors.join("; "),
            };
          }),
          error: null,
        };
      } catch (error) {
        return {
          observations: [],
          error: error instanceof Error ? error.message : String(error),
        };
      }
    };
    SVGGraphicsElement.prototype.getBBox = function (...args) {
      const element = this;
      const style = getComputedStyle(element);
      try {
        const box = originalGetBBox.apply(element, args);
        const textGeometry = readTextGeometry(element);
        recordBBox(
          element,
          style,
          { x: box.x, y: box.y, width: box.width, height: box.height },
          null,
          textGeometry.observations,
          textGeometry.error,
        );
        return box;
      } catch (error) {
        const textGeometry = readTextGeometry(element);
        recordBBox(
          element,
          style,
          null,
          error instanceof Error ? error.message : String(error),
          textGeometry.observations,
          textGeometry.error,
        );
        throw error;
      }
    };
    CanvasRenderingContext2D.prototype.measureText = function (text) {
      const metrics = originalMeasureText.call(this, text);
      const value = (key: keyof TextMetrics): number | null => {
        const candidate = metrics[key];
        return typeof candidate === "number" && Number.isFinite(candidate) ? candidate : null;
      };
      canvasMeasureTextCalls.push({
        callIndex: canvasMeasureTextCalls.length,
        text: String(text),
        font: this.font,
        textAlign: this.textAlign,
        textBaseline: this.textBaseline,
        width: metrics.width,
        actualBoundingBoxLeft: value("actualBoundingBoxLeft"),
        actualBoundingBoxRight: value("actualBoundingBoxRight"),
        actualBoundingBoxAscent: value("actualBoundingBoxAscent"),
        actualBoundingBoxDescent: value("actualBoundingBoxDescent"),
      });
      return metrics;
    };
    host.__katanaRawMetrics = { bboxCalls, canvasMeasureTextCalls };
  });
}

function renderRawMermaid({
  config,
  slug,
  source,
}: {
  config: MermaidConfig;
  slug: string;
  source: string;
}): Promise<RenderedRawSvg> {
  const runtime = window as MermaidI18nWindow;
  const mermaid = runtime.mermaid;
  if (mermaid === undefined) throw new Error("Vendor Mermaid runtime was not registered");
  const zenuml = runtime["mermaid-zenuml"];
  if (zenuml === undefined) throw new Error("Vendor ZenUML runtime was not registered");
  mermaid.initialize(config);
  return mermaid.registerExternalDiagrams([zenuml]).then(() =>
    mermaid.render(`official-${slug}`, source).then((result) => ({
      svg: result.svg,
      diagramType: result.diagramType,
    })),
  );
}

function readRawMetricsSnapshot(): RawMetricsSnapshot {
  const host = window as Window & {
    __katanaRawMetrics?: BrowserRawMetricsSnapshot;
  };
  if (host.__katanaRawMetrics === undefined) {
    throw new Error("Raw metrics observer is not registered");
  }
  return {
    bboxCalls: host.__katanaRawMetrics.bboxCalls.map(({ node, ...call }) => ({
      ...call,
      finalClassName: node.getAttribute("class"),
      finalTransform: node.getAttribute("transform"),
      finalText: node.textContent,
    })),
    canvasMeasureTextCalls: host.__katanaRawMetrics.canvasMeasureTextCalls,
  };
}

function writeDiagnosticOutputs(
  outputDir: string,
  rawSvg: Buffer,
  snapshot: RawMetricsSnapshot,
  receipt: RawMetricsDiagnosticReceipt,
) {
  const directory = path.resolve(outputDir);
  fs.mkdirSync(directory, { recursive: true });
  fs.writeFileSync(path.join(directory, OUTPUT_FILES.svg), rawSvg, { flag: "wx" });
  fs.writeFileSync(
    path.join(directory, OUTPUT_FILES.bbox),
    `${JSON.stringify(
      {
        schema: "katana-mermaid-svg-bbox-observations-v1",
        diagnosticOnly: true,
        accepted: false,
        calls: snapshot.bboxCalls,
      },
      null,
      2,
    )}\n`,
    { flag: "wx" },
  );
  fs.writeFileSync(
    path.join(directory, OUTPUT_FILES.canvas),
    `${JSON.stringify(
      {
        schema: "katana-mermaid-canvas-measure-text-observations-v1",
        diagnosticOnly: true,
        accepted: false,
        calls: snapshot.canvasMeasureTextCalls,
      },
      null,
      2,
    )}\n`,
    { flag: "wx" },
  );
  fs.writeFileSync(
    path.join(directory, OUTPUT_FILES.receipt),
    `${JSON.stringify(receipt, null, 2)}\n`,
    { flag: "wx" },
  );
}

function readDiagnosticManifest(manifestPath: string): DiagnosticManifest {
  const canonicalManifestPath = canonicalRegularFile(path.resolve(manifestPath), "manifest");
  const root = objectValue(
    parseJsonValue(fs.readFileSync(canonicalManifestPath, "utf8"), "official renderer provenance"),
    "official renderer provenance",
  );
  if (stringField(root, "schema", "manifest schema") !== "katana-mermaid-render-provenance-v1") {
    throw new Error("Unsupported official renderer provenance schema");
  }
  if (booleanField(root, "diagnosticOnly", "manifest diagnosticOnly") !== true) {
    throw new Error("Official renderer provenance must be diagnostic-only");
  }
  if (booleanField(root, "accepted", "manifest accepted") !== false) {
    throw new Error("Official renderer provenance must not be accepted");
  }
  const fixture = objectField(root, "fixture", "fixture");
  const slug = stringField(fixture, "slug", "fixture slug");
  const canonicalSource = stringField(fixture, "canonicalSource", "canonical source");
  if (slug.trim() === "" || canonicalSource.trim() === "") {
    throw new Error("Official renderer provenance requires canonical source and slug");
  }
  if (
    sha256(Buffer.from(canonicalSource, "utf8")) !==
    shaField(fixture, "canonicalSourceSha256", "canonical source")
  ) {
    throw new Error("Official renderer canonical source digest differs");
  }
  const config = objectField(root, "config", "config");
  const mermaidConfig = parseMermaidConfig(objectField(config, "mermaid", "Mermaid config"));
  const themeRecord = objectField(root, "theme", "theme");
  const theme = DiagramTheme.parse(stringField(themeRecord, "name", "theme name")).name;
  if (mermaidConfig.theme !== DiagramTheme.parse(theme).mermaidTheme) {
    throw new Error("Official renderer theme and Mermaid config theme differ");
  }
  const referenceModeValue = referenceMode(stringField(root, "referenceMode", "reference mode"));
  const referenceScope = stringField(root, "referenceScope", "reference scope");
  if (referenceScope !== "formal" && referenceScope !== "diagnostic") {
    throw new Error(`Unsupported reference scope: ${referenceScope}`);
  }
  const primaryStage = stringField(root, "primaryStage", "primary stage");
  if (primaryStage !== "rawMermaid") {
    throw new Error("Raw metrics diagnostic requires a rawMermaid primary stage");
  }
  const primarySvgSha256 = shaField(root, "primarySvgSha256", "primary SVG");
  const assets = objectField(root, "rendererAssets", "renderer assets");
  const mermaidJs = readPinnedAsset(
    objectField(assets, "mermaid", "Mermaid asset"),
    "Mermaid asset",
  );
  const mermaidZenumlJs = readPinnedAsset(
    objectField(assets, "zenuml", "ZenUML asset"),
    "ZenUML asset",
  );
  const rawStage = objectField(
    objectField(root, "stages", "stages"),
    "rawMermaid",
    "raw SVG stage",
  );
  const rawStagePath = resolveManifestStagePath(
    canonicalManifestPath,
    stringField(rawStage, "path", "raw SVG stage path"),
  );
  if (fileSha256(rawStagePath) !== shaField(rawStage, "sha256", "raw SVG stage")) {
    throw new Error("Official renderer raw SVG stage digest differs");
  }
  if (fileSha256(rawStagePath) !== primarySvgSha256) {
    throw new Error("Official renderer primary SVG digest differs from raw SVG stage");
  }
  return {
    manifestPath: canonicalManifestPath,
    slug,
    canonicalSource,
    mermaidConfig,
    theme,
    referenceMode: referenceModeValue,
    mermaidJs,
    mermaidZenumlJs,
    primarySvgSha256,
  };
}

export function resolveManifestStagePath(manifestPath: string, stagePath: string): string {
  const manifestDirectory = path.dirname(manifestPath);
  const resolved = path.isAbsolute(stagePath)
    ? path.resolve(stagePath)
    : path.resolve(manifestDirectory, stagePath);
  const relative = path.relative(manifestDirectory, resolved);
  if (relative === "" || relative === ".." || relative.startsWith(`..${path.sep}`)) {
    throw new Error("Raw SVG stage path escapes provenance manifest directory");
  }
  return canonicalRegularFile(resolved, "raw SVG stage");
}

function parseMermaidConfig(value: Record<string, JsonValue>): MermaidConfig {
  const themeValue = stringField(value, "theme", "Mermaid config theme");
  if (themeValue !== "dark" && themeValue !== "default") {
    throw new Error(`Unsupported Mermaid config theme: ${themeValue}`);
  }
  const theme: MermaidThemeName = themeValue;
  const flowchart = objectField(value, "flowchart", "Mermaid flowchart config");
  const sequence = objectField(value, "sequence", "Mermaid sequence config");
  const themeVariables = objectField(value, "themeVariables", "Mermaid theme variables");
  const variable = (name: string) =>
    stringField(themeVariables, name, `Mermaid theme variable ${name}`);
  const parsedVariables = {
    background: variable("background"),
    mainBkg: variable("mainBkg"),
    primaryColor: variable("primaryColor"),
    primaryTextColor: variable("primaryTextColor"),
    primaryBorderColor: variable("primaryBorderColor"),
    secondaryColor: variable("secondaryColor"),
    secondaryTextColor: variable("secondaryTextColor"),
    secondaryBorderColor: variable("secondaryBorderColor"),
    tertiaryColor: variable("tertiaryColor"),
    tertiaryTextColor: variable("tertiaryTextColor"),
    tertiaryBorderColor: variable("tertiaryBorderColor"),
    nodeTextColor: variable("nodeTextColor"),
    lineColor: variable("lineColor"),
    textColor: variable("textColor"),
    edgeLabelBackground: variable("edgeLabelBackground"),
    actorBkg: variable("actorBkg"),
    actorTextColor: variable("actorTextColor"),
    actorBorder: variable("actorBorder"),
    signalColor: variable("signalColor"),
    signalTextColor: variable("signalTextColor"),
    labelTextColor: variable("labelTextColor"),
    noteBkgColor: variable("noteBkgColor"),
    noteTextColor: variable("noteTextColor"),
    noteBorderColor: variable("noteBorderColor"),
    clusterBkg: variable("clusterBkg"),
    clusterBorder: variable("clusterBorder"),
    titleColor: variable("titleColor"),
  };
  return {
    startOnLoad: booleanField(value, "startOnLoad", "Mermaid config startOnLoad"),
    securityLevel: literalField(value, "securityLevel", "loose", "Mermaid config securityLevel"),
    htmlLabels: booleanField(value, "htmlLabels", "Mermaid config htmlLabels"),
    theme,
    flowchart: {
      htmlLabels: booleanField(flowchart, "htmlLabels", "Mermaid flowchart htmlLabels"),
      useMaxWidth: booleanField(flowchart, "useMaxWidth", "Mermaid flowchart useMaxWidth"),
    },
    sequence: {
      useMaxWidth: booleanField(sequence, "useMaxWidth", "Mermaid sequence useMaxWidth"),
    },
    themeVariables: parsedVariables,
  };
}

function readPinnedAsset(value: Record<string, JsonValue>, label: string): string {
  const assetPath = canonicalRegularFile(stringField(value, "path", `${label} path`), label);
  if (fileSha256(assetPath) !== shaField(value, "sha256", label)) {
    throw new Error(`${label} digest differs from provenance pin`);
  }
  return assetPath;
}

function objectValue(value: JsonValue, label: string): Record<string, JsonValue> {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} must be a JSON object`);
  }
  return value;
}

function objectField(
  record: Record<string, JsonValue>,
  key: string,
  label: string,
): Record<string, JsonValue> {
  return objectValue(requiredField(record, key, label), label);
}

function requiredField(record: Record<string, JsonValue>, key: string, label: string): JsonValue {
  const value = record[key];
  if (value === undefined) throw new Error(`${label} is missing: ${key}`);
  return value;
}

function stringField(record: Record<string, JsonValue>, key: string, label: string): string {
  const value = requiredField(record, key, label);
  if (typeof value !== "string" || value === "") throw new Error(`${label} is invalid: ${key}`);
  return value;
}

function booleanField(record: Record<string, JsonValue>, key: string, label: string): boolean {
  const value = requiredField(record, key, label);
  if (typeof value !== "boolean") throw new Error(`${label} is invalid: ${key}`);
  return value;
}

function literalField<T extends string>(
  record: Record<string, JsonValue>,
  key: string,
  expected: T,
  label: string,
): T {
  const value = stringField(record, key, label);
  if (value !== expected) throw new Error(`${label} must be ${expected}`);
  return expected;
}

function shaField(record: Record<string, JsonValue>, key: string, label: string): string {
  const value = stringField(record, key, label);
  if (!/^[0-9a-f]{64}$/.test(value)) throw new Error(`${label} SHA is invalid`);
  return value;
}

if (import.meta.main) {
  try {
    const options = parseRawMetricsDiagnosticArgs(process.argv.slice(2));
    const receipt = await runRawMetricsDiagnostic(options);
    process.stdout.write(`${JSON.stringify(receipt)}\n`);
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    process.stderr.write(`${detail}\n`);
    process.exitCode = 1;
  }
}
