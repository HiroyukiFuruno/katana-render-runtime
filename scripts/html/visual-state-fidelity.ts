import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { chromium, type Page, type Request, type Response } from "playwright";
import {
  type CatalogCaptureProducerContext,
  captureCatalogProducerContext,
  sameCatalogProducerContext,
} from "./catalog-capture-producer-context";
import { assertCoreTextPostScriptOrder, validateFontProvenance } from "./coretext-font-provenance";
import { applyEventsInteraction } from "./dynamic-events";
import {
  captureKrrCatalogCondition,
  captureKrrState,
  DYNAMIC_VIEWPORTS,
  type DynamicViewport,
} from "./dynamic-fidelity";
import { type JsonValue, specForDynamicCase } from "./dynamic-state";
import { assertCatalogComplete, type CatalogCase, loadFeatureCatalog } from "./feature-catalog";
import { type FidelityPhase, fidelityThresholdForPhase } from "./fidelity-phase";
import { compareImages, type ScoreReport, sha256File } from "./reference-capture";
import {
  assertStrictHtml84ChromeReceipt,
  assertStrictHtml84KrrReceipt,
  type Html84ConditionIdentity,
} from "./stage-score";
import {
  assertCatalogCaptureIdentity,
  assertCatalogStage90ConditionPlan,
  type CatalogStage90ConditionPlan,
  inspectCatalogRawStage90Reuse,
} from "./stage-score-suite";

export const INPUT_STATE_OPERATIONS = [
  "empty",
  "filled",
  "selected",
  "expanded",
  "disabled",
  "focus-visible",
] as const;
type InputStateOperation = (typeof INPUT_STATE_OPERATIONS)[number];
const INPUT_STATE_FIXTURE = path.resolve("tests/fixtures/html/aspects/input-state.html");
const STATE_EXPRESSION = "JSON.parse(document.querySelector('#state-summary').textContent || '{}')";

export interface VisualStateSpec {
  caseId: string;
  state: string;
  targetSelector?: string;
  operationKind: string;
  expression: string;
}

interface ChromeResourceRequest {
  url: string;
  mainDocument: boolean;
  response?: Response;
  finished: boolean;
  failed: boolean;
}

interface ChromeResourceEntry {
  url: string;
  bodyPath: string;
  bytesSha256: string;
  loaded: true;
}

export interface ChromeSelectedFont {
  familyName: string;
  postScriptName: string;
  glyphCount: number;
  isCustomFont: boolean;
}

interface ChromeFontIdentityCapture {
  conditionId: string;
  selectedFonts: ChromeSelectedFont[];
  resourceFiles: Array<Pick<ChromeResourceEntry, "url" | "bodyPath" | "bytesSha256">>;
}

export interface CatalogStage90ResumeOptions {
  observationManifestPath: string;
  finalizedManifestPath: string;
}

export interface ChromeFontIdentityRequest {
  schema: "krr-chrome-font-identity-request-v1";
  captures: ChromeFontIdentityCapture[];
}

interface ChromeFontObservation {
  selectedFonts: ChromeSelectedFont[];
  unknownReasons: string[];
}

export function createChromeFontIdentityRequest(
  captures: ChromeFontIdentityCapture[],
  expectedConditionIds: readonly string[],
): ChromeFontIdentityRequest {
  const ids = captures.map((capture) => capture.conditionId);
  if (
    ids.length !== expectedConditionIds.length ||
    new Set(ids).size !== ids.length ||
    ids.some((id, index) => id !== expectedConditionIds[index])
  )
    throw new Error("Chrome font identity request condition coverage differs from the plan");
  for (const capture of captures) {
    if (
      capture.selectedFonts.some(
        (font) =>
          font.familyName.length === 0 ||
          font.postScriptName.length === 0 ||
          !Number.isSafeInteger(font.glyphCount) ||
          font.glyphCount <= 0,
      ) ||
      capture.resourceFiles.some(
        (resource) =>
          resource.url.length === 0 ||
          !path.isAbsolute(resource.bodyPath) ||
          !/^[a-f0-9]{64}$/.test(resource.bytesSha256),
      )
    )
      throw new Error(
        `Chrome font identity request contains invalid evidence: ${capture.conditionId}`,
      );
  }
  return { schema: "krr-chrome-font-identity-request-v1", captures };
}

function jsonRecord(value: JsonValue | undefined, label: string): Record<string, JsonValue> {
  if (typeof value !== "object" || value === null || Array.isArray(value))
    throw new Error(`${label} is not an object`);
  return value;
}

function jsonSelectedFonts(value: JsonValue | undefined): ChromeSelectedFont[] {
  if (!Array.isArray(value)) throw new Error("Chrome font identity observations are missing");
  return value.map((font) => {
    const row = jsonRecord(font, "Chrome selected font");
    if (
      typeof row.familyName !== "string" ||
      typeof row.postScriptName !== "string" ||
      typeof row.glyphCount !== "number" ||
      typeof row.isCustomFont !== "boolean"
    )
      throw new Error("Chrome selected font is incomplete");
    return {
      familyName: row.familyName,
      postScriptName: row.postScriptName,
      glyphCount: row.glyphCount,
      isCustomFont: row.isCustomFont,
    };
  });
}

export function jsonResourceFiles(
  value: JsonValue | undefined,
): ChromeFontIdentityCapture["resourceFiles"] {
  if (!Array.isArray(value)) throw new Error("Chrome resource file observations are missing");
  return value.map((resource) => {
    const row = jsonRecord(resource, "Chrome resource body");
    if (
      typeof row.url !== "string" ||
      typeof row.bodyPath !== "string" ||
      typeof row.bytesSha256 !== "string" ||
      row.loaded !== true
    )
      throw new Error("Chrome resource body is incomplete");
    return {
      url: row.url,
      bodyPath: path.resolve(row.bodyPath),
      bytesSha256: row.bytesSha256,
    };
  });
}

function fontObservationJson(observation: ChromeFontObservation): JsonValue {
  return {
    status: observation.unknownReasons.length === 0 ? "observed" : "unknown",
    selectedFonts: observation.selectedFonts as unknown as JsonValue,
    unknownReasons: observation.unknownReasons,
  };
}

interface ChromeFontDomNode {
  nodeId: number;
  nodeType: number;
  children?: ChromeFontDomNode[];
  shadowRoots?: ChromeFontDomNode[];
  pseudoElements?: ChromeFontDomNode[];
  contentDocument?: ChromeFontDomNode;
  templateContent?: ChromeFontDomNode;
}

export function chromeFontTextNodeIds(root: ChromeFontDomNode): number[] {
  const pending = [root];
  const visited = new Set<number>();
  const textNodes: number[] = [];
  while (pending.length > 0) {
    const node = pending.pop();
    if (!node || visited.has(node.nodeId)) continue;
    visited.add(node.nodeId);
    if (node.nodeType === 3) {
      if (!Number.isSafeInteger(node.nodeId) || node.nodeId <= 0)
        throw new Error("Chrome text node has no live DOM identity");
      textNodes.push(node.nodeId);
    }
    pending.push(...chromeFontDescendants(node));
  }
  return textNodes.sort((left, right) => left - right);
}

function chromeFontDescendants(node: ChromeFontDomNode): ChromeFontDomNode[] {
  return [
    ...(node.children ?? []),
    ...(node.shadowRoots ?? []),
    ...(node.pseudoElements ?? []),
    ...(node.contentDocument ? [node.contentDocument] : []),
    ...(node.templateContent ? [node.templateContent] : []),
  ];
}

function accumulateChromeFont(
  selected: Map<string, ChromeSelectedFont>,
  font: ChromeSelectedFont,
): string | undefined {
  if (
    !font.familyName ||
    !font.postScriptName ||
    !Number.isSafeInteger(font.glyphCount) ||
    font.glyphCount < 0
  )
    return "CDP returned an incomplete selected-font identity";
  const key = JSON.stringify([font.familyName, font.postScriptName, font.isCustomFont]);
  const glyphCount = (selected.get(key)?.glyphCount ?? 0) + font.glyphCount;
  if (!Number.isSafeInteger(glyphCount))
    return "CDP selected-font glyph count exceeds a safe integer";
  selected.set(key, { ...font, glyphCount });
  return undefined;
}

export async function captureChromePlatformFonts(page: Page): Promise<ChromeFontObservation> {
  const session = await page.context().newCDPSession(page);
  try {
    await session.send("DOM.enable");
    await session.send("CSS.enable");
    const { root } = await session.send("DOM.getDocument", { depth: -1, pierce: true });
    const selected = new Map<string, ChromeSelectedFont>();
    const unknownReasons: string[] = [];
    // Chromiumの祖先node照会はlayoutの2層までなので、実TextNodeを個別に観測する。
    for (const nodeId of chromeFontTextNodeIds(root)) {
      const { fonts } = await session.send("CSS.getPlatformFontsForNode", { nodeId });
      for (const font of fonts) {
        if (font.glyphCount === 0) continue;
        const reason = accumulateChromeFont(selected, font);
        if (reason) unknownReasons.push(reason);
      }
    }
    const selectedFonts = [...selected.entries()]
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([, font]) => font);
    if (selectedFonts.length === 0)
      unknownReasons.push("CDP reported no platform font with rendered glyphs");
    return { selectedFonts, unknownReasons };
  } catch (error) {
    return {
      selectedFonts: [],
      unknownReasons: [`CDP platform-font observation failed: ${String(error)}`],
    };
  } finally {
    await session.detach();
  }
}

export interface ChromeResourceProvenance {
  inventoryComplete: boolean;
  requests: ChromeResourceEntry[];
}

export class ChromeResourceBodyCollector {
  private readonly requests = new Map<Request, ChromeResourceRequest>();
  private unresolved = false;
  private readonly onRequest = (request: Request): void => {
    const mainDocument = mainFrameDocument(request, this.page);
    if (mainDocument === undefined) this.unresolved = true;
    this.requests.set(request, {
      url: request.url(),
      mainDocument: mainDocument === true,
      finished: false,
      failed: false,
    });
  };
  private readonly onResponse = (response: Response): void => {
    const request = this.requests.get(response.request());
    if (!request) this.unresolved = true;
    else request.response = response;
  };
  private readonly onRequestFinished = (request: Request): void => {
    const observed = this.requests.get(request);
    if (!observed) this.unresolved = true;
    else observed.finished = true;
  };
  private readonly onRequestFailed = (request: Request): void => {
    const observed = this.requests.get(request);
    if (!observed) this.unresolved = true;
    else observed.failed = true;
  };

  constructor(
    private readonly page: Page,
    private readonly conditionId: string,
    private readonly sourceSha256: string,
    private readonly capturePath: string,
  ) {
    if (!/^[a-f0-9]{64}$/.test(sourceSha256) || conditionId.length === 0)
      throw new Error("Chrome resource identity is invalid");
    page.on("request", this.onRequest);
    page.on("response", this.onResponse);
    page.on("requestfinished", this.onRequestFinished);
    page.on("requestfailed", this.onRequestFailed);
  }

  async finish(resourceUrls: string[]): Promise<ChromeResourceProvenance> {
    this.detach();
    return collectChromeResourceBodies({
      requests: [...this.requests.values()],
      resourceUrls,
      capturePath: this.capturePath,
      conditionId: this.conditionId,
      sourceSha256: this.sourceSha256,
      unresolved: this.unresolved,
    });
  }

  private detach(): void {
    this.page.removeListener("request", this.onRequest);
    this.page.removeListener("response", this.onResponse);
    this.page.removeListener("requestfinished", this.onRequestFinished);
    this.page.removeListener("requestfailed", this.onRequestFailed);
  }
}

function mainFrameDocument(request: Request, page: Page): boolean | undefined {
  if (!request.isNavigationRequest()) return false;
  try {
    return request.frame() === page.mainFrame();
  } catch {
    return undefined;
  }
}

function resourceBodyDirectory(
  capturePath: string,
  conditionId: string,
  sourceSha256: string,
): string {
  const label = conditionId.replace(/[^a-zA-Z0-9.-]+/g, "_");
  return path.join(path.dirname(capturePath), `${label}-resources-${sourceSha256}`);
}

function responseIsBodySuccess(
  request: ChromeResourceRequest,
): request is ChromeResourceRequest & { response: Response } {
  return (
    request.finished &&
    !request.failed &&
    request.response !== undefined &&
    request.response.status() >= 200 &&
    request.response.status() < 300
  );
}

async function persistChromeResourceBody(
  request: ChromeResourceRequest & { response: Response },
  directory: string,
  index: number,
): Promise<ChromeResourceEntry | undefined> {
  let body: Buffer;
  try {
    body = await request.response.body();
  } catch {
    return undefined;
  }
  const bytesSha256 = createHash("sha256").update(body).digest("hex");
  const bodyPath = path.join(directory, `${String(index).padStart(4, "0")}-${bytesSha256}.body`);
  fs.writeFileSync(bodyPath, body, { flag: "wx" });
  return { url: request.url, bodyPath, bytesSha256, loaded: true };
}

async function collectChromeResourceBodies(input: {
  requests: ChromeResourceRequest[];
  resourceUrls: string[];
  capturePath: string;
  conditionId: string;
  sourceSha256: string;
  unresolved: boolean;
}): Promise<ChromeResourceProvenance> {
  const output = resourceBodyDirectory(input.capturePath, input.conditionId, input.sourceSha256);
  const requests: ChromeResourceEntry[] = [];
  let inventoryComplete = !input.unresolved;
  let outputPrepared = false;
  for (const request of input.requests) {
    if (request.mainDocument) continue;
    if (!responseIsBodySuccess(request)) {
      inventoryComplete = false;
      continue;
    }
    if (!outputPrepared) {
      fs.mkdirSync(output, { recursive: false });
      outputPrepared = true;
    }
    const saved = await persistChromeResourceBody(request, output, requests.length);
    if (!saved) inventoryComplete = false;
    else requests.push(saved);
  }
  if (input.resourceUrls.some((url) => !requests.some((request) => request.url === url)))
    inventoryComplete = false;
  return { inventoryComplete, requests };
}

function visualExpression(selector: string): string {
  const quoted = JSON.stringify(selector);
  return `(()=>{const e=document.querySelector(${quoted});if(!e)return null;return {tag:e.tagName,text:e.textContent||"",id:e.id||"",role:e.getAttribute("role"),ariaSelected:e.getAttribute("aria-selected"),ariaExpanded:e.getAttribute("aria-expanded"),hidden:e.hidden,disabled:e.disabled===true,value:typeof e.value==="string"?e.value:null,children:Array.from(e.children).map(c=>c.tagName)}})()`;
}

function visualSpec(caseId: string, state: string, targetSelector: string): VisualStateSpec {
  const operationKind =
    caseId === "input-state"
      ? `input-state-${state === "filled" ? "filled-save" : state === "focus-visible" ? "focus-visible-tab" : state === "disabled" ? "disabled-observe" : state}`
      : caseId === "navigation"
        ? state === "all-toc-links"
          ? "navigation-all-pointer-clicks"
          : "navigation-pointer-click"
        : "load-and-observe-target";
  return {
    caseId,
    state,
    targetSelector,
    operationKind,
    expression: visualExpression(targetSelector),
  };
}

/** Fixed visual-state boundaries; input/script cases are supplied by the feature catalog. */
export const VISUAL_STATE_SPECS: VisualStateSpec[] = [
  visualSpec("typography", "ja-en-mixed", ".mixed"),
  visualSpec("typography", "fractional-metrics", ".mixed"),
  visualSpec("typography", "nowrap", ".nowrap"),
  visualSpec("typography", "long-line", "#long-line"),
  visualSpec("layout", "narrow-viewport", ".flex"),
  visualSpec("layout", "multi-column", ".grid"),
  visualSpec("layout", "min-content", "#min-content"),
  visualSpec("table", "long-status", ".badge"),
  visualSpec("table", "collapsed-border", "table"),
  visualSpec("table", "rich-cell", "tbody"),
  visualSpec("navigation", "top", "#toc-a"),
  visualSpec("navigation", "middle", "#toc-b"),
  visualSpec("navigation", "bottom", "#toc-c"),
  visualSpec("navigation", "all-toc-links", ".sidebar .toc-item[href^='#']"),
  visualSpec("input-state", "empty", "#name"),
  visualSpec("input-state", "filled", "#name"),
  visualSpec("input-state", "selected", "#tab-advanced"),
  visualSpec("input-state", "expanded", "#accordion"),
  visualSpec("input-state", "disabled", "#disabled"),
  visualSpec("input-state", "focus-visible", "#name"),
  visualSpec("decoration", "stacked", ".alert"),
  visualSpec("decoration", "clipped", ".overlay"),
  visualSpec("decoration", "translucent", ".overlay"),
];

function stableJson(value: JsonValue): string {
  if (Array.isArray(value)) return `[${value.map(stableJson).join(",")}]`;
  if (typeof value === "object" && value !== null) {
    return `{${Object.entries(value)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, item]) => `${JSON.stringify(key)}:${stableJson(item)}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
}

function captureReport(value: JsonValue): { capturePath: string; htmlSha256: string } {
  if (
    typeof value !== "object" ||
    value === null ||
    Array.isArray(value) ||
    typeof value.capturePath !== "string" ||
    typeof value.htmlSha256 !== "string"
  ) {
    throw new Error("invalid KRR visual capture report");
  }
  return { capturePath: value.capturePath, htmlSha256: value.htmlSha256 };
}

export interface VisualStateResult {
  operation: InputStateOperation;
  viewport: { width: number; height: number };
  score: ScoreReport;
  stateMatches: boolean;
  passed: boolean;
  chromePath: string;
  krrPath: string;
}

async function applyChromeOperation(page: Page, operation: InputStateOperation): Promise<void> {
  if (operation === "filled") {
    await page.locator("#name").click();
    await page.locator("#name").fill("架空の名称");
    await page.locator("#save").click();
  } else if (operation === "selected") {
    await page.locator("#tab-advanced").click();
  } else if (operation === "expanded") {
    await page.locator("#accordion").click();
  } else if (operation === "focus-visible") {
    await page.keyboard.press("Tab");
  }
}

async function captureChromeState(
  page: Page,
  operation: InputStateOperation,
  viewport: (typeof DYNAMIC_VIEWPORTS)[number],
  output: string,
): Promise<JsonValue> {
  await page.setViewportSize(viewport);
  await page.goto(`file://${INPUT_STATE_FIXTURE}`, { waitUntil: "networkidle" });
  await applyChromeOperation(page, operation);
  const state = await page.evaluate((expression) => {
    const script = document.createElement("script");
    script.textContent = `globalThis.__krrVisualState = (${expression});`;
    document.head.append(script);
    const stateGlobal = globalThis as typeof globalThis & { __krrVisualState?: JsonValue };
    const value = stateGlobal.__krrVisualState ?? null;
    delete stateGlobal.__krrVisualState;
    script.remove();
    return value;
  }, STATE_EXPRESSION);
  await page.screenshot({ path: output, fullPage: false });
  return state;
}

function assertInputStateCatalogCoverage(): void {
  const catalogCase = loadFeatureCatalog().visualAspects.find(
    (entry) => entry.id === "input-state",
  );
  if (!catalogCase) throw new Error("input-state is missing from the feature catalog");
  if (stableJson(catalogCase.boundaryStates ?? []) !== stableJson([...INPUT_STATE_OPERATIONS])) {
    throw new Error("input-state operations do not cover catalog boundary states");
  }
}

export async function runInputStateFidelity(
  binary: string,
  outputDirectory: string,
  minScore = 99,
  options: { phase: "interim" } | undefined = undefined,
): Promise<VisualStateResult[]> {
  const phase: FidelityPhase = options?.phase ?? "final";
  const threshold = fidelityThresholdForPhase(phase, minScore);
  assertInputStateCatalogCoverage();
  const binarySha = sha256File(binary);
  const fixtureSha = sha256File(INPUT_STATE_FIXTURE);
  fs.mkdirSync(outputDirectory, { recursive: true });
  const browser = await chromium.launch({ channel: "chrome", headless: true });
  const page = await browser.newPage();
  const results: VisualStateResult[] = [];
  try {
    for (const operation of INPUT_STATE_OPERATIONS) {
      for (const viewport of DYNAMIC_VIEWPORTS) {
        const label = `${operation}-${viewport.width}x${viewport.height}`;
        const chromePath = path.join(outputDirectory, `${label}-chrome.png`);
        const krrJson = path.join(outputDirectory, `${label}-krr.json`);
        const krrState = captureKrrState(
          binary,
          INPUT_STATE_FIXTURE,
          STATE_EXPRESSION,
          krrJson,
          `input-state:${operation}`,
          viewport,
        );
        const report = captureReport(JSON.parse(fs.readFileSync(krrJson, "utf8")) as JsonValue);
        if (report.htmlSha256 !== fixtureSha) {
          throw new Error(`input-state provenance mismatch: ${label}`);
        }
        const chromeState = await captureChromeState(page, operation, viewport, chromePath);
        const stateMatches = stableJson(chromeState) === stableJson(krrState);
        const score = compareImages(chromePath, report.capturePath, threshold);
        results.push({
          operation,
          viewport,
          score,
          stateMatches,
          passed: stateMatches && score.passed,
          chromePath,
          krrPath: report.capturePath,
        });
      }
    }
  } finally {
    await page.close();
    await browser.close();
  }
  if (sha256File(binary) !== binarySha) throw new Error("visual state binary changed");
  if (sha256File(INPUT_STATE_FIXTURE) !== fixtureSha)
    throw new Error("visual state fixture changed");
  const resultPath = path.join(
    outputDirectory,
    phase === "interim" ? "interim-fidelity-results.json" : "fidelity-results.json",
  );
  const result = phase === "interim" ? { phase, threshold, results } : results;
  fs.writeFileSync(resultPath, `${JSON.stringify(result, null, 2)}\n`);
  if (results.length !== INPUT_STATE_OPERATIONS.length * DYNAMIC_VIEWPORTS.length) {
    throw new Error("input-state visual state coverage is incomplete");
  }
  if (results.some((result) => !result.passed))
    throw new Error("input-state visual score gate failed");
  return results;
}

export async function runCatalogStage90Captures(
  binary: string,
  planPath: string,
  outputDirectory: string,
  resume?: CatalogStage90ResumeOptions,
): Promise<JsonValue> {
  outputDirectory = path.resolve(outputDirectory);
  const catalog = loadFeatureCatalog();
  assertCatalogComplete(catalog);
  const plan = assertCatalogStage90ConditionPlan(
    JSON.parse(fs.readFileSync(path.resolve(planPath), "utf8")) as JsonValue,
  );
  const binarySha256 = sha256File(binary);
  const catalogSha256 = sha256File("tests/fixtures/html/feature-catalog.json");
  if (fs.existsSync(outputDirectory))
    throw new Error("Catalog stage90 capture output directory already exists");
  fs.mkdirSync(outputDirectory);
  const producerContext = captureCatalogProducerContext(outputDirectory, "before");
  const server = await chromium.launchServer({ channel: "chrome", headless: true });
  let browser: Awaited<ReturnType<typeof chromium.launch>> | undefined;
  const conditions: JsonValue[] = [];
  const fontIdentityCaptures: ChromeFontIdentityCapture[] = [];
  let chromeRuntimeVersion = "";
  try {
    const chromeProcess = server.process();
    if (!chromeProcess.spawnfile)
      throw new Error("Chrome server process did not expose its executable path");
    const chromeExecutablePath = fs.realpathSync(chromeProcess.spawnfile);
    const chromeExecutableSha256 = sha256File(chromeExecutablePath);
    const connected = await chromium.connect(server.wsEndpoint());
    browser = connected;
    chromeRuntimeVersion = requireChromeRuntimeVersion(connected);
    const prior = resume
      ? readCatalogStage90Resume(resume, planPath, {
          binaryPath: path.resolve(binary),
          chromeExecutablePath,
          chromeExecutableSha256,
          producerContext,
          chromeRuntimeVersion,
        })
      : undefined;
    if (prior?.rejectionReason)
      fs.writeFileSync(
        path.join(outputDirectory, "catalog-stage90-resume-rejection.json"),
        `${JSON.stringify({ reason: prior.rejectionReason }, null, 2)}\n`,
        { flag: "wx" },
      );
    if (prior)
      fs.writeFileSync(
        path.join(outputDirectory, "catalog-stage90-resume-report.json"),
        `${JSON.stringify(
          {
            reusedConditionIds: [...prior.reusableConditionIds],
            recapturedConditionIds: plan.conditions
              .map((condition) => condition.conditionId)
              .filter((conditionId) => !prior.reusableConditionIds.has(conditionId)),
            rejected: prior.rejected ?? [],
            ...(prior.rejectionReason ? { rejectionReason: prior.rejectionReason } : {}),
          },
          null,
          2,
        )}\n`,
        { flag: "wx" },
      );
    const captured = await collectCatalogConditions(
      connected,
      catalog,
      plan,
      binary,
      binarySha256,
      outputDirectory,
      { path: chromeExecutablePath, sha256: chromeExecutableSha256 },
      prior,
    );
    conditions.push(...captured.conditions);
    fontIdentityCaptures.push(...captured.fontIdentityCaptures);
  } finally {
    try {
      if (browser) await browser.close();
    } finally {
      await server.close();
    }
  }
  if (sha256File(binary) !== binarySha256)
    throw new Error("Catalog probe binary changed during capture");
  if (sha256File("tests/fixtures/html/feature-catalog.json") !== catalogSha256)
    throw new Error("Feature catalog changed during capture");
  const producerContextAfter = captureCatalogProducerContext(outputDirectory, "after");
  assertCatalogProducerContextStable(producerContext, producerContextAfter);
  const request = createChromeFontIdentityRequest(
    fontIdentityCaptures,
    plan.conditions.map((condition) => condition.conditionId),
  );
  const requestPath = path.join(outputDirectory, "chrome-font-identity-request.json");
  const requestBytes = Buffer.from(`${JSON.stringify(request, null, 2)}\n`);
  fs.writeFileSync(requestPath, requestBytes, { flag: "wx" });
  const requestRef = {
    path: path.resolve(requestPath),
    sha256: createHash("sha256").update(requestBytes).digest("hex"),
    sizeBytes: requestBytes.length,
  };
  const conditionsWithRequest = conditions.map((condition, index) => {
    const row = jsonRecord(condition, "catalog condition capture");
    const requestCondition = request.captures[index];
    if (!requestCondition) throw new Error("Chrome font identity request row is missing");
    return {
      ...row,
      fontIdentityRequestConditionId: requestCondition.conditionId,
      fontIdentityRequestPath: requestRef.path,
      fontIdentityRequestSha256: requestRef.sha256,
      fontIdentityRequestSizeBytes: requestRef.sizeBytes,
    } as unknown as JsonValue;
  });
  const manifest = {
    profile: "catalog-stage90-captures" as const,
    threshold: 90 as const,
    acceptance: false as const,
    catalogPath: path.resolve("tests/fixtures/html/feature-catalog.json"),
    catalogSha256,
    planPath: path.resolve(planPath),
    planSha256: sha256File(planPath),
    binaryPath: path.resolve(binary),
    binarySha256,
    producerContext,
    producerContextAfter,
    chromeRuntimeVersion,
    fontIdentityRequest: requestRef,
    fontIdentityProbeBinary: { path: path.resolve(binary), sha256: binarySha256 },
    conditions: conditionsWithRequest,
  };
  const captureManifestPath = path.join(
    outputDirectory,
    "catalog-stage90-capture-observation.json",
  );
  fs.writeFileSync(captureManifestPath, `${JSON.stringify(manifest, null, 2)}\n`, { flag: "wx" });
  const responsePath = path.join(outputDirectory, "chrome-font-identity-response.json");
  runChromeFontIdentityProbe(binary, requestPath, responsePath, outputDirectory);
  const finalized = finalizeCatalogStage90FontIdentity(
    captureManifestPath,
    path.resolve(planPath),
    responsePath,
    path.join(outputDirectory, "catalog-stage90-captures.json"),
  );
  const finalizedManifest = jsonRecord(finalized, "final catalog stage90 manifest");
  const finalizedConditions = jsonArray(
    finalizedManifest.conditions,
    "final catalog stage90 conditions",
  );
  if (
    conditionsWithRequest.length !== 84 ||
    finalizedConditions.some((condition) => !isCapturedCondition(condition))
  )
    throw new Error("Catalog stage90 capture has unverified conditions; acceptance remains false");
  return finalized;
}

function requireChromeRuntimeVersion(browser: { version(): string }): string {
  const version = browser.version();
  if (!version) throw new Error("Chrome runtime version is empty");
  return version;
}

function assertCatalogProducerContextStable(
  before: CatalogCaptureProducerContext,
  after: CatalogCaptureProducerContext,
): void {
  if (!sameCatalogProducerContext(before, after))
    throw new Error("Catalog capture producer context changed during capture");
}

interface CatalogStage90ResumeState {
  reusableConditionIds: Set<string>;
  finalizedById: Map<string, JsonValue>;
  observationById: Map<string, JsonValue>;
  rejected?: Array<{ conditionId: string; reason: string }>;
  rejectionReason?: string;
}

async function collectCatalogConditions(
  browser: Awaited<ReturnType<typeof chromium.launch>>,
  catalog: ReturnType<typeof loadFeatureCatalog>,
  plan: CatalogStage90ConditionPlan,
  binary: string,
  binarySha256: string,
  outputDirectory: string,
  chromeExecutable: { path: string; sha256: string },
  prior: CatalogStage90ResumeState | undefined,
): Promise<{ conditions: JsonValue[]; fontIdentityCaptures: ChromeFontIdentityCapture[] }> {
  const conditions: JsonValue[] = [];
  const fontIdentityCaptures: ChromeFontIdentityCapture[] = [];
  for (const tuple of plan.conditions) {
    const reused = prior?.reusableConditionIds.has(tuple.conditionId) ?? false;
    const capture = reused
      ? reusedCatalogCondition(requireResumeState(prior), tuple.conditionId, outputDirectory)
      : await captureCatalogCondition(
          browser,
          catalog,
          tuple,
          binary,
          binarySha256,
          outputDirectory,
          chromeExecutable,
        );
    assertCatalogCaptureIdentity(
      tuple,
      jsonRecord(capture.condition, "catalog condition capture"),
      binarySha256,
    );
    conditions.push(capture.condition);
    fontIdentityCaptures.push(capture.fontIdentityRequest);
  }
  return { conditions, fontIdentityCaptures };
}

function reusedCatalogCondition(
  prior: CatalogStage90ResumeState,
  conditionId: string,
  outputDirectory: string,
): { condition: JsonValue; fontIdentityRequest: ChromeFontIdentityCapture } {
  const finalizedValue = prior.finalizedById.get(conditionId);
  const observationValue = prior.observationById.get(conditionId);
  if (!finalizedValue || !observationValue)
    throw new Error(`reusable catalog condition evidence is missing: ${conditionId}`);
  const finalized = jsonRecord(finalizedValue, "prior finalized condition");
  const observation = jsonRecord(observationValue, "prior observation condition");
  assertReusableRowsMatch(finalized, observation, conditionId);
  const oldReceiptPath = jsonText(observation.chromeReceiptPath, "prior Chrome receipt path");
  const label = conditionId.replace(/[^a-zA-Z0-9.-]+/g, "_");
  const freshReceiptPath = path.join(outputDirectory, `${label}-chrome-receipt.json`);
  const freshReceiptSha256 = copyCatalogStage90Receipt(
    oldReceiptPath,
    freshReceiptPath,
    jsonText(observation.chromeReceiptSha256, "prior Chrome receipt SHA"),
  );
  const condition = {
    ...observation,
    chromeReceiptPath: freshReceiptPath,
    chromeReceiptSha256: freshReceiptSha256,
    status: "unverified",
  } as unknown as JsonValue;
  return {
    condition,
    fontIdentityRequest: catalogFontIdentityCapture(observation, conditionId),
  };
}

function requireResumeState(
  state: CatalogStage90ResumeState | undefined,
): CatalogStage90ResumeState {
  if (!state) throw new Error("catalog resume state is missing");
  return state;
}

export function copyCatalogStage90Receipt(
  sourcePath: string,
  destinationPath: string,
  expectedSha256: string,
): string {
  const bytes = fs.readFileSync(sourcePath);
  const actualSha256 = sha256Bytes(bytes);
  if (actualSha256 !== expectedSha256) throw new Error("prior Chrome receipt changed");
  fs.writeFileSync(destinationPath, bytes, { flag: "wx" });
  return actualSha256;
}

function assertReusableRowsMatch(
  finalized: Record<string, JsonValue>,
  observation: Record<string, JsonValue>,
  conditionId: string,
): void {
  for (const key of [
    "conditionId",
    "caseId",
    "state",
    "operationKind",
    "targetSelector",
    "viewport",
    "sourceSha256",
    "binarySha256",
    "chromePath",
    "chromeSha256",
    "krrReportPath",
    "krrReportSha256",
    "resourceProvenance",
    "fontIdentityObservation",
  ])
    if (!sameJson(finalized[key] ?? null, observation[key] ?? null))
      throw new Error(`prior observation differs from finalized row: ${conditionId}`);
}

function sha256Bytes(bytes: Buffer): string {
  return createHash("sha256").update(bytes).digest("hex");
}

function readCatalogStage90Resume(
  resume: CatalogStage90ResumeOptions,
  currentPlanPath: string,
  context: {
    binaryPath: string;
    chromeExecutablePath: string;
    chromeExecutableSha256: string;
    producerContext: CatalogCaptureProducerContext;
    chromeRuntimeVersion: string;
  },
): CatalogStage90ResumeState {
  const priorManifestPath = fs.realpathSync(path.resolve(resume.finalizedManifestPath));
  const priorObservationPath = fs.realpathSync(path.resolve(resume.observationManifestPath));
  const priorManifest = jsonRecord(
    JSON.parse(fs.readFileSync(priorManifestPath, "utf8")) as JsonValue,
    "prior catalog capture manifest",
  );
  const priorPlanPath = absolutePath(priorManifest.planPath, "prior catalog plan path");
  const currentPlanRealPath = fs.realpathSync(path.resolve(currentPlanPath));
  if (sha256File(priorPlanPath) !== sha256File(currentPlanRealPath))
    return {
      reusableConditionIds: new Set(),
      finalizedById: new Map(),
      observationById: new Map(),
      rejected: rejectedPlanIds(
        currentPlanPath,
        "prior catalog plan differs from the current plan",
      ),
      rejectionReason: "prior catalog plan differs from the current plan",
    };
  const currentPlan = assertCatalogStage90ConditionPlan(
    JSON.parse(fs.readFileSync(currentPlanRealPath, "utf8")) as JsonValue,
  );
  const changedFixture = currentPlan.conditions.find(
    (condition) => sha256File(condition.fixturePath) !== condition.sourceSha256,
  );
  if (changedFixture)
    return {
      reusableConditionIds: new Set(),
      finalizedById: new Map(),
      observationById: new Map(),
      rejected: rejectedPlanIds(currentPlanPath, "current catalog fixture differs"),
      rejectionReason: `current catalog fixture differs: ${changedFixture.conditionId}`,
    };
  const reuse = inspectCatalogRawStage90Reuse(priorManifest, currentPlan as unknown as JsonValue, {
    planPath: priorPlanPath,
    binaryPath: context.binaryPath,
    chromeExecutablePath: context.chromeExecutablePath,
    chromeExecutableSha256: context.chromeExecutableSha256,
    producerContext: context.producerContext,
    chromeRuntimeVersion: context.chromeRuntimeVersion,
  });
  if (reuse.reusableConditionIds.length === 0)
    return {
      reusableConditionIds: new Set(),
      finalizedById: new Map(),
      observationById: new Map(),
      rejected: reuse.rejected,
      rejectionReason: "prior catalog captures cannot be reused",
    };
  const observation = jsonRecord(
    JSON.parse(fs.readFileSync(priorObservationPath, "utf8")) as JsonValue,
    "prior catalog observation",
  );
  assertObservationManifest(observation, priorManifest, currentPlan, priorPlanPath);
  const finalizedById = new Map(
    jsonArray(priorManifest.conditions, "prior finalized conditions").map((value) => {
      const row = jsonRecord(value, "prior finalized condition");
      return [jsonText(row.conditionId, "prior condition id"), row] as const;
    }),
  );
  const observationById = new Map(
    jsonArray(observation.conditions, "prior observation conditions").map((value) => {
      const row = jsonRecord(value, "prior observation condition");
      return [jsonText(row.conditionId, "prior observation condition id"), row] as const;
    }),
  );
  return {
    reusableConditionIds: new Set(reuse.reusableConditionIds),
    finalizedById,
    observationById,
    rejected: reuse.rejected,
  };
}

function rejectedPlanIds(
  planPath: string,
  reason: string,
): Array<{ conditionId: string; reason: string }> {
  const plan = assertCatalogStage90ConditionPlan(
    JSON.parse(fs.readFileSync(path.resolve(planPath), "utf8")) as JsonValue,
  );
  return plan.conditions.map((condition) => ({
    conditionId: condition.conditionId,
    reason,
  }));
}

function assertObservationManifest(
  observation: Record<string, JsonValue>,
  finalized: Record<string, JsonValue>,
  plan: CatalogStage90ConditionPlan,
  planPath: string,
): void {
  if (
    observation.profile !== "catalog-stage90-captures" ||
    observation.threshold !== 90 ||
    observation.acceptance !== false ||
    observation.catalogPath !== plan.catalogPath ||
    observation.catalogSha256 !== plan.catalogSha256 ||
    path.resolve(jsonText(observation.planPath, "observation plan path")) !==
      path.resolve(planPath) ||
    observation.planSha256 !== sha256File(planPath) ||
    observation.binaryPath !== finalized.binaryPath ||
    observation.binarySha256 !== finalized.binarySha256 ||
    observation.chromeRuntimeVersion !== finalized.chromeRuntimeVersion
  )
    throw new Error("prior catalog observation identity differs");
  if (
    !sameCatalogProducerContext(observation.producerContext, finalized.producerContext) ||
    (observation.producerContextAfter !== undefined &&
      !sameCatalogProducerContext(observation.producerContext, observation.producerContextAfter))
  )
    throw new Error("prior catalog producer context differs");
  const expected = plan.conditions.map((condition) => condition.conditionId);
  const actual = jsonArray(observation.conditions, "prior observation conditions").map((value) =>
    jsonText(jsonRecord(value, "prior observation condition").conditionId, "condition id"),
  );
  if (actual.length !== expected.length || new Set(actual).size !== actual.length)
    throw new Error("prior observation condition inventory is incomplete or duplicated");
  if (actual.some((conditionId, index) => conditionId !== expected[index]))
    throw new Error("prior observation condition order differs from the plan");
}

function catalogFontIdentityCapture(
  row: Record<string, JsonValue>,
  conditionId: string,
): ChromeFontIdentityCapture {
  const observation = jsonRecord(row.fontIdentityObservation, "prior font observation");
  const resources = jsonRecord(row.resourceProvenance, "prior resource provenance");
  return {
    conditionId,
    selectedFonts: jsonSelectedFonts(observation.selectedFonts),
    resourceFiles: jsonResourceFiles(resources.requests),
  };
}

function runChromeFontIdentityProbe(
  binary: string,
  requestPath: string,
  responsePath: string,
  outputDirectory: string,
): void {
  const absoluteBinary = path.resolve(binary);
  const absoluteRequest = path.resolve(requestPath);
  const absoluteResponse = path.resolve(responsePath);
  if (fs.existsSync(absoluteResponse))
    throw new Error("Chrome font identity probe output already exists");
  const args = [
    "--ignored",
    "--exact",
    "markdown::chrome_font_identity_probe_tests::chrome_font_identity_probe_cli",
    "--nocapture",
  ];
  fs.writeFileSync(
    path.join(outputDirectory, "chrome-font-identity-probe-command.json"),
    `${JSON.stringify({ binary: absoluteBinary, args, request: absoluteRequest, response: absoluteResponse }, null, 2)}\n`,
    { flag: "wx" },
  );
  const result = spawnSync(absoluteBinary, args, {
    env: {
      ...process.env,
      KRR_CHROME_FONT_IDENTITY_REQUEST: absoluteRequest,
      KRR_CHROME_FONT_IDENTITY_OUTPUT: absoluteResponse,
    },
    encoding: "buffer",
  });
  fs.writeFileSync(
    path.join(outputDirectory, "chrome-font-identity-probe.stdout.raw"),
    result.stdout ?? Buffer.alloc(0),
    { flag: "wx" },
  );
  fs.writeFileSync(
    path.join(outputDirectory, "chrome-font-identity-probe.stderr.raw"),
    result.stderr ?? Buffer.alloc(0),
    { flag: "wx" },
  );
  fs.writeFileSync(
    path.join(outputDirectory, "chrome-font-identity-probe-exit.json"),
    `${JSON.stringify({ status: result.status, signal: result.signal, error: result.error?.message ?? null }, null, 2)}\n`,
    { flag: "wx" },
  );
  if (result.error)
    throw new Error(`Chrome font identity probe could not start: ${result.error.message}`);
  if (result.status !== 0)
    throw new Error(`Chrome font identity probe exited unsuccessfully: ${String(result.status)}`);
}

export function finalizeCatalogStage90FontIdentity(
  captureManifestPath: string,
  conditionPlanPath: string,
  responsePath: string,
  outputManifestPath: string,
): JsonValue {
  const captureManifest = jsonRecord(
    JSON.parse(fs.readFileSync(captureManifestPath, "utf8")) as JsonValue,
    "catalog capture manifest",
  );
  const plan = assertCatalogStage90ConditionPlan(
    JSON.parse(fs.readFileSync(conditionPlanPath, "utf8")) as JsonValue,
  );
  if (
    captureManifest.profile !== "catalog-stage90-captures" ||
    captureManifest.threshold !== 90 ||
    captureManifest.acceptance !== false ||
    captureManifest.planPath !== path.resolve(conditionPlanPath) ||
    captureManifest.planSha256 !== sha256File(conditionPlanPath)
  )
    throw new Error("Catalog capture manifest does not match the stage90 plan");
  const requestRef = jsonRecord(captureManifest.fontIdentityRequest, "font request ref");
  const binaryRef = jsonRecord(captureManifest.fontIdentityProbeBinary, "font probe binary ref");
  const binaryPath = absolutePath(binaryRef.path, "font probe binary path");
  const binarySha256 = jsonText(binaryRef.sha256, "font probe binary SHA");
  if (
    captureManifest.binaryPath !== binaryPath ||
    captureManifest.binarySha256 !== binarySha256 ||
    sha256File(binaryPath) !== binarySha256
  )
    throw new Error("Chrome font identity probe binary differs from the capture manifest");
  const request = readVerifiedFontRequest(
    requestRef,
    plan.conditions.map((row) => row.conditionId),
  );
  const responseBytes = fs.readFileSync(responsePath);
  const responseSha256 = createHash("sha256").update(responseBytes).digest("hex");
  const response = verifyFontIdentityResponse(
    JSON.parse(responseBytes.toString("utf8")) as JsonValue,
    responseSha256,
    requestRef,
    plan.conditions.map((row) => row.conditionId),
  );
  const conditions = jsonArray(captureManifest.conditions, "catalog capture conditions");
  if (conditions.length !== plan.conditions.length)
    throw new Error("catalog capture manifest condition count differs from the plan");
  const responseRef = {
    path: path.resolve(responsePath),
    sha256: responseSha256,
    sizeBytes: responseBytes.length,
  };
  const finalized = conditions.map((value, index) => {
    const tuple = plan.conditions[index];
    const requestCapture = request.captures[index];
    const responseCapture = response.captures[index];
    if (!tuple || !requestCapture || !responseCapture)
      throw new Error(`Chrome font identity row is missing at index ${index}`);
    const row = jsonRecord(value, "catalog capture condition");
    if (row.binarySha256 !== binarySha256)
      throw new Error(
        `Catalog condition binary differs from the probe binary: ${tuple.conditionId}`,
      );
    return finalizeFontIdentityCondition(
      row,
      tuple,
      requestCapture,
      responseCapture,
      requestRef,
      responseRef,
    );
  });
  if (
    sha256File(binaryPath) !== binarySha256 ||
    sha256File(jsonText(requestRef.path, "font request path")) !==
      jsonText(requestRef.sha256, "request SHA") ||
    sha256File(responseRef.path) !== responseRef.sha256
  )
    throw new Error("Chrome font identity evidence changed during finalization");
  const manifest = {
    ...captureManifest,
    fontIdentityProbe: {
      request: requestRef,
      response: responseRef,
      binary: binaryRef,
      inventoryMethod: response.inventoryMethod,
      inventoryLimit: response.inventoryLimit,
    },
    conditions: finalized,
  };
  fs.writeFileSync(outputManifestPath, `${JSON.stringify(manifest, null, 2)}\n`, { flag: "wx" });
  return manifest as unknown as JsonValue;
}

interface ChromeFontIdentityResponseCapture {
  conditionId: string;
  status: string;
  fontProvenance: JsonValue[];
  unknownReasons: string[];
}

interface ChromeFontIdentityResponse {
  inventoryMethod: string;
  inventoryLimit: string;
  captures: ChromeFontIdentityResponseCapture[];
}

function readVerifiedFontRequest(
  reference: Record<string, JsonValue>,
  expectedConditionIds: readonly string[],
): ChromeFontIdentityRequest {
  const requestPath = absolutePath(reference.path, "font request path");
  const bytes = fs.readFileSync(requestPath);
  if (
    createHash("sha256").update(bytes).digest("hex") !==
      jsonText(reference.sha256, "request SHA") ||
    bytes.length !== jsonNumber(reference.sizeBytes, "request size")
  )
    throw new Error("Chrome font identity request bytes differ from the capture manifest");
  const value = jsonRecord(
    JSON.parse(bytes.toString("utf8")) as JsonValue,
    "font identity request",
  );
  if (value.schema !== "krr-chrome-font-identity-request-v1")
    throw new Error("Chrome font identity request schema is invalid");
  const captures = jsonArray(value.captures, "font identity request captures").map((capture) => {
    const row = jsonRecord(capture, "font identity request capture");
    return {
      conditionId: jsonText(row.conditionId, "request condition id"),
      selectedFonts: jsonSelectedFonts(row.selectedFonts),
      resourceFiles: jsonResourceFiles(row.resourceFiles),
    };
  });
  return createChromeFontIdentityRequest(captures, expectedConditionIds);
}

function verifyFontIdentityResponse(
  value: JsonValue,
  responseSha256: string,
  requestRef: Record<string, JsonValue>,
  expectedConditionIds: readonly string[],
): ChromeFontIdentityResponse {
  const response = jsonRecord(value, "font identity response");
  if (
    response.schema !== "krr-chrome-font-identity-response-v1" ||
    response.requestSha256 !== requestRef.sha256 ||
    !/^[a-f0-9]{64}$/.test(responseSha256)
  )
    throw new Error("Chrome font identity response does not match its request");
  const inventoryMethod = jsonText(response.inventoryMethod, "font inventory method");
  const inventoryLimit = jsonText(response.inventoryLimit, "font inventory limit");
  const captures = jsonArray(response.captures, "font identity response captures").map(
    (capture) => {
      const row = jsonRecord(capture, "font identity response capture");
      if (!Array.isArray(row.fontProvenance))
        throw new Error("Chrome font identity response provenance is invalid");
      const unknownReasons = jsonArray(row.unknownReasons, "font identity unknown reasons");
      if (!unknownReasons.every((reason) => typeof reason === "string"))
        throw new Error("Chrome font identity unknown reason is invalid");
      return {
        conditionId: jsonText(row.conditionId, "response condition id"),
        status: jsonText(row.status, "response condition status"),
        fontProvenance: row.fontProvenance,
        unknownReasons: unknownReasons as string[],
      };
    },
  );
  if (
    captures.length !== expectedConditionIds.length ||
    captures.some((capture, index) => capture.conditionId !== expectedConditionIds[index])
  )
    throw new Error("Chrome font identity response condition coverage differs from the plan");
  return { inventoryMethod, inventoryLimit, captures };
}

function finalizeFontIdentityCondition(
  row: Record<string, JsonValue>,
  tuple: CatalogStage90ConditionPlan["conditions"][number],
  request: ChromeFontIdentityCapture,
  result: ChromeFontIdentityResponseCapture,
  requestRef: Record<string, JsonValue>,
  responseRef: { path: string; sha256: string; sizeBytes: number },
): JsonValue {
  const observation = jsonRecord(row.fontIdentityObservation, "font identity observation");
  const reasons = [
    ...stringArray(observation.unknownReasons, "CDP unknown reasons"),
    ...result.unknownReasons,
  ];
  const resourceProvenance = jsonRecord(row.resourceProvenance, "resource provenance");
  const observedResources = jsonResourceFiles(resourceProvenance.requests);
  const observedFonts = jsonSelectedFonts(observation.selectedFonts);
  if (
    result.conditionId !== tuple.conditionId ||
    row.fontIdentityRequestConditionId !== tuple.conditionId ||
    row.fontIdentityRequestPath !== requestRef.path ||
    row.fontIdentityRequestSha256 !== requestRef.sha256 ||
    row.fontIdentityRequestSizeBytes !== requestRef.sizeBytes ||
    !sameJson(observedResources, request.resourceFiles) ||
    !sameJson(
      observedFonts.map((font) => ({ ...font })) as unknown as JsonValue,
      request.selectedFonts.map((font) => ({ ...font })) as unknown as JsonValue,
    ) ||
    resourceProvenance.inventoryComplete !== true ||
    observation.status !== "observed" ||
    result.status !== "resolved" ||
    reasons.length > 0 ||
    result.fontProvenance.length === 0 ||
    result.fontProvenance.length !== request.selectedFonts.length ||
    !resourceBodiesStillMatch(request.resourceFiles)
  )
    return {
      ...row,
      status: "unverified",
      fontIdentityResponsePath: responseRef.path,
      fontIdentityResponseSha256: responseRef.sha256,
      fontIdentityResponseSizeBytes: responseRef.sizeBytes,
      fontIdentityUnknownReasons: reasons,
    } as unknown as JsonValue;

  try {
    const verifiedFonts = validateFontProvenance(result.fontProvenance);
    assertCoreTextPostScriptOrder(
      verifiedFonts,
      request.selectedFonts.map((font) => font.postScriptName),
    );
    const expected = conditionIdentity(tuple, row);
    const krr = assertStrictHtml84KrrReceipt(
      jsonText(row.krrReportPath, "KRR report path"),
      jsonText(row.krrReportSha256, "KRR report SHA"),
      expected,
    );
    const receiptPath = jsonText(row.chromeReceiptPath, "unknown Chrome receipt path");
    const receipt = jsonRecord(
      JSON.parse(fs.readFileSync(receiptPath, "utf8")) as JsonValue,
      "unknown Chrome receipt",
    );
    if (receipt.status !== "unknown" || receipt.resourceProvenance === undefined)
      throw new Error("prior Chrome receipt is not the expected unknown receipt");
    if (
      receipt.capturePath !== row.chromePath ||
      receipt.captureSha256 !== row.chromeSha256 ||
      stableJson(jsonRecord(receipt.resourceProvenance, "receipt resource provenance")) !==
        stableJson(resourceProvenance)
    )
      throw new Error("Chrome capture row differs from its original receipt");
    const identifiedReceipt = {
      ...receipt,
      status: "observed",
      fontProvenance: result.fontProvenance,
    } as Record<string, JsonValue>;
    delete identifiedReceipt.unknownReason;
    const identifiedPath = `${receiptPath.slice(0, -5)}-font-provenance-candidate.json`;
    fs.writeFileSync(identifiedPath, `${JSON.stringify(identifiedReceipt, null, 2)}\n`, {
      flag: "wx",
    });
    const identifiedSha = sha256File(identifiedPath);
    const chrome = assertStrictHtml84ChromeReceipt(identifiedPath, identifiedSha, expected);
    if (stableJson(chrome.state) !== stableJson(krr.state))
      throw new Error("Chrome and KRR state differ after font provenance resolution");
    const captured = {
      ...row,
      status: "captured",
      chromeCapturePath: chrome.capturePath,
      chromeCaptureSha256: chrome.captureSha256,
      chromeReceiptPath: identifiedPath,
      chromeReceiptSha256: identifiedSha,
      fontIdentityResponsePath: responseRef.path,
      fontIdentityResponseSha256: responseRef.sha256,
      fontIdentityResponseSizeBytes: responseRef.sizeBytes,
      krrCapturePath: krr.capturePath,
      krrCaptureSha256: krr.captureSha256,
      krrState: krr.state,
      chromeState: chrome.state,
      fontIdentityUnknownReasons: [],
    } as Record<string, JsonValue>;
    delete captured.unknownReason;
    return captured;
  } catch (error) {
    return {
      ...row,
      status: "unverified",
      unknownReason: String(error),
      fontIdentityResponsePath: responseRef.path,
      fontIdentityResponseSha256: responseRef.sha256,
      fontIdentityResponseSizeBytes: responseRef.sizeBytes,
      fontIdentityUnknownReasons: [String(error)],
    } as unknown as JsonValue;
  }
}

function sameJson(left: JsonValue, right: JsonValue): boolean {
  return stableJson(left) === stableJson(right);
}

function conditionIdentity(
  tuple: CatalogStage90ConditionPlan["conditions"][number],
  row: Record<string, JsonValue>,
): Html84ConditionIdentity {
  const viewport = jsonRecord(row.viewport, "capture viewport");
  const binarySha256 = jsonText(row.binarySha256, "capture binary SHA");
  return {
    conditionId: tuple.conditionId,
    caseId: tuple.caseId,
    state: tuple.state,
    operationKind: tuple.operationKind,
    ...(tuple.targetSelector ? { targetSelector: tuple.targetSelector } : {}),
    viewport: {
      width: jsonNumber(viewport.width, "viewport width"),
      height: jsonNumber(viewport.height, "viewport height"),
      scale: jsonNumber(viewport.scale, "viewport scale"),
    },
    sourceSha256: tuple.sourceSha256,
    binarySha256,
  };
}

function resourceBodiesStillMatch(resources: ChromeFontIdentityCapture["resourceFiles"]): boolean {
  return resources.every(
    (resource) =>
      fs.existsSync(resource.bodyPath) && sha256File(resource.bodyPath) === resource.bytesSha256,
  );
}

function jsonArray(value: JsonValue | undefined, label: string): JsonValue[] {
  if (!Array.isArray(value)) throw new Error(`${label} is not an array`);
  return value;
}

function stringArray(value: JsonValue | undefined, label: string): string[] {
  const items = jsonArray(value, label);
  if (!items.every((item) => typeof item === "string")) throw new Error(`${label} is invalid`);
  return items as string[];
}

function jsonText(value: JsonValue | undefined, label: string): string {
  if (typeof value !== "string" || value.length === 0) throw new Error(`${label} is invalid`);
  return value;
}

function jsonNumber(value: JsonValue | undefined, label: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) throw new Error(`${label} is invalid`);
  return value;
}

function absolutePath(value: JsonValue | undefined, label: string): string {
  const resolved = jsonText(value, label);
  if (!path.isAbsolute(resolved)) throw new Error(`${label} must be absolute`);
  return resolved;
}

async function captureCatalogCondition(
  browser: Awaited<ReturnType<typeof chromium.launch>>,
  catalog: ReturnType<typeof loadFeatureCatalog>,
  tuple: CatalogStage90ConditionPlan["conditions"][number],
  binary: string,
  binarySha256: string,
  outputDirectory: string,
  chromeExecutable: { path: string; sha256: string },
): Promise<{ condition: JsonValue; fontIdentityRequest: ChromeFontIdentityCapture }> {
  const entry = [...catalog.visualAspects, ...catalog.scriptCases].find(
    (candidate) => candidate.id === tuple.caseId,
  );
  if (!entry) throw new Error(`Missing catalog case: ${tuple.caseId}`);
  const fixtureSha = sha256File(tuple.fixturePath);
  if (fixtureSha !== tuple.sourceSha256)
    throw new Error(`Changed catalog fixture: ${tuple.conditionId}`);
  const label = tuple.conditionId.replace(/[^a-zA-Z0-9.-]+/g, "_");
  const chromePath = path.join(outputDirectory, `${label}-chrome.png`);
  const reportPath = path.join(outputDirectory, `${label}-krr.json`);
  const chromeReceiptPath = path.join(outputDirectory, `${label}-chrome-receipt.json`);
  try {
    const condition = await executeCatalogCondition(
      browser,
      entry,
      tuple,
      binary,
      binarySha256,
      fixtureSha,
      chromePath,
      reportPath,
      chromeReceiptPath,
      chromeExecutable,
    );
    const row = jsonRecord(condition, "catalog condition capture");
    const observation = jsonRecord(row.fontIdentityObservation, "font identity observation");
    const resourceProvenance = jsonRecord(row.resourceProvenance, "resource provenance");
    return {
      condition,
      fontIdentityRequest: {
        conditionId: tuple.conditionId,
        selectedFonts: jsonSelectedFonts(observation.selectedFonts),
        resourceFiles: jsonResourceFiles(resourceProvenance.requests),
      },
    };
  } catch (error) {
    return {
      condition: {
        ...catalogConditionIdentityFields(tuple, fixtureSha, binarySha256),
        viewport: { ...tuple.viewport },
        status: "unverified",
        error: String(error),
        fontIdentityObservation: {
          status: "unknown",
          selectedFonts: [],
          unknownReasons: [String(error)],
        },
        resourceProvenance: { inventoryComplete: false, requests: [] },
      } as unknown as JsonValue,
      fontIdentityRequest: { conditionId: tuple.conditionId, selectedFonts: [], resourceFiles: [] },
    };
  }
}

async function executeCatalogCondition(
  browser: Awaited<ReturnType<typeof chromium.launch>>,
  entry: CatalogCase,
  tuple: CatalogStage90ConditionPlan["conditions"][number],
  binary: string,
  binarySha256: string,
  fixtureSha: string,
  chromePath: string,
  reportPath: string,
  chromeReceiptPath: string,
  chromeExecutable: { path: string; sha256: string },
): Promise<JsonValue> {
  const chrome = await captureChromeCatalogCondition(
    browser,
    entry,
    tuple.state,
    chromePath,
    tuple.viewport,
    tuple.conditionId,
    fixtureSha,
  );
  const spec = catalogOperationSpec(entry, tuple.state);
  if (spec.operationKind !== tuple.operationKind || spec.targetSelector !== tuple.targetSelector)
    throw new Error(`Catalog operation definition differs: ${tuple.conditionId}`);
  const krr = captureKrrCatalogCondition(binary, tuple.fixturePath, spec.expression, reportPath, {
    conditionId: tuple.conditionId,
    caseId: tuple.caseId,
    state: tuple.state,
    operationKind: tuple.operationKind,
    ...(tuple.targetSelector ? { targetSelector: tuple.targetSelector } : {}),
    viewport: { ...tuple.viewport },
  });
  writeUnknownChromeReceipt(
    chromeReceiptPath,
    tuple,
    fixtureSha,
    binarySha256,
    chrome,
    chromePath,
    chromeExecutable,
    chrome.resourceProvenance,
  );
  if (krr.status === "unknown")
    return unknownCatalogCondition(
      tuple,
      fixtureSha,
      binarySha256,
      chrome,
      chromePath,
      reportPath,
      chromeReceiptPath,
      krr,
    );
  return unverifiedChromeCatalogCondition(
    tuple,
    fixtureSha,
    binarySha256,
    chrome,
    chromePath,
    reportPath,
    chromeReceiptPath,
    krr,
  );
}

function unknownCatalogCondition(
  tuple: CatalogStage90ConditionPlan["conditions"][number],
  fixtureSha: string,
  binarySha256: string,
  chrome: Awaited<ReturnType<typeof captureChromeCatalogCondition>>,
  chromePath: string,
  reportPath: string,
  chromeReceiptPath: string,
  krr: Extract<ReturnType<typeof captureKrrCatalogCondition>, { status: "unknown" }>,
): JsonValue {
  return {
    ...catalogConditionIdentityFields(tuple, fixtureSha, binarySha256),
    viewport: { ...tuple.viewport },
    chromePath,
    chromeSha256: sha256File(chromePath),
    chromeReceiptPath,
    chromeReceiptSha256: sha256File(chromeReceiptPath),
    chromeState: chrome.state,
    chromeBoundary: chrome.boundary,
    resourceProvenance: chrome.resourceProvenance as unknown as JsonValue,
    fontIdentityObservation: fontObservationJson(chrome.fontIdentityObservation),
    krrReportPath: reportPath,
    krrReportSha256: sha256File(reportPath),
    ...(krr.capturePath ? { krrCapturePath: krr.capturePath } : {}),
    ...(krr.captureSha256 ? { krrCaptureSha256: krr.captureSha256 } : {}),
    ...(krr.stateValue === undefined ? {} : { krrState: krr.stateValue }),
    status: "unverified",
    unknownReason: krr.unknownReason,
  } as unknown as JsonValue;
}

function writeUnknownChromeReceipt(
  receiptPath: string,
  tuple: CatalogStage90ConditionPlan["conditions"][number],
  sourceSha256: string,
  binarySha256: string,
  chrome: { state: JsonValue; boundary: JsonValue },
  capturePath: string,
  chromeExecutable: { path: string; sha256: string },
  resourceProvenance: ChromeResourceProvenance,
): void {
  const receipt = {
    schema: "krr-html84-chrome-condition-v1",
    status: "unknown",
    conditionId: tuple.conditionId,
    caseId: tuple.caseId,
    boundaryState: tuple.state,
    operationKind: tuple.operationKind,
    sourceSha256,
    binarySha256,
    viewport: { ...tuple.viewport },
    browserExecutablePath: chromeExecutable.path,
    browserExecutableSha256: chromeExecutable.sha256,
    unknownReason: resourceProvenance.inventoryComplete
      ? "Chrome adapter has no independent physical selected-face/file-byte binding"
      : "Chrome adapter has unknown selected-face identity and incomplete resource body observation",
    capturePath: path.resolve(capturePath),
    captureSha256: sha256File(capturePath),
    state: chrome.state,
    boundary: chrome.boundary,
    resourceProvenance,
    fontProvenance: [],
  };
  fs.writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
}

function unverifiedChromeCatalogCondition(
  tuple: CatalogStage90ConditionPlan["conditions"][number],
  sourceSha256: string,
  binarySha256: string,
  chrome: Awaited<ReturnType<typeof captureChromeCatalogCondition>>,
  chromePath: string,
  reportPath: string,
  chromeReceiptPath: string,
  krr: Extract<ReturnType<typeof captureKrrCatalogCondition>, { status: "observed" }>,
): JsonValue {
  return {
    ...catalogConditionIdentityFields(tuple, sourceSha256, binarySha256),
    viewport: { ...tuple.viewport },
    chromePath,
    chromeSha256: sha256File(chromePath),
    chromeReceiptPath,
    chromeReceiptSha256: sha256File(chromeReceiptPath),
    chromeState: chrome.state,
    chromeBoundary: chrome.boundary,
    resourceProvenance: chrome.resourceProvenance as unknown as JsonValue,
    fontIdentityObservation: fontObservationJson(chrome.fontIdentityObservation),
    krrReportPath: reportPath,
    krrReportSha256: sha256File(reportPath),
    krrCapturePath: krr.capturePath,
    krrCaptureSha256: krr.captureSha256,
    krrState: krr.stateValue,
    operationEvidence: krr.operationEvidence as unknown as JsonValue,
    unknownReason:
      "Independent Chrome raw receipt has unknown physical font/browser binary provenance",
    status: "unverified",
  } as unknown as JsonValue;
}

export function catalogConditionIdentityFields(
  tuple: CatalogStage90ConditionPlan["conditions"][number],
  sourceSha256: string,
  binarySha256: string,
): Record<string, JsonValue> {
  return {
    conditionId: tuple.conditionId,
    caseId: tuple.caseId,
    state: tuple.state,
    operationKind: tuple.operationKind,
    ...(tuple.targetSelector ? { targetSelector: tuple.targetSelector } : {}),
    sourceSha256,
    binarySha256,
  };
}

function catalogOperationSpec(entry: CatalogCase, state: string): VisualStateSpec {
  if (entry.mode === "visual") return visualCatalogOperationSpec(entry, state);
  const dynamic = specForDynamicCase(entry.id);
  const operationKind: Partial<Record<string, string>> = {
    "dynamic-dom": "script-load-observe",
    events: "script-real-events",
    async: "script-await-timer-microtask",
    observers: "script-await-mutation-delivery",
    resources: "script-await-resource-font",
  };
  return {
    caseId: entry.id,
    state,
    operationKind: operationKind[entry.id] ?? "",
    expression: dynamic.expression,
  };
}

function visualCatalogOperationSpec(entry: CatalogCase, state: string): VisualStateSpec {
  const spec = VISUAL_STATE_SPECS.find(
    (candidate) => candidate.caseId === entry.id && candidate.state === state,
  );
  if (!spec) throw new Error(`Missing visual state definition: ${entry.id}/${state}`);
  if (entry.id === "input-state") return { ...spec, expression: STATE_EXPRESSION };
  if (entry.id === "navigation") return navigationCatalogOperationSpec(spec, state);
  return spec;
}

function navigationCatalogOperationSpec(spec: VisualStateSpec, state: string): VisualStateSpec {
  if (state === "all-toc-links") {
    return {
      ...spec,
      expression: `({links:[...document.querySelectorAll('.toc-item[href^="#"]')].map(a=>({hash:a.hash,text:a.textContent,target:document.querySelector(a.hash)?.textContent||""}))})`,
    };
  }
  const destination = state === "top" ? "a" : state === "middle" ? "b" : "c";
  return {
    ...spec,
    expression: `({hash:location.hash,target:document.querySelector(location.hash)?.textContent||"",destination:${JSON.stringify(destination)}})`,
  };
}

async function captureChromeCatalogCondition(
  browser: Awaited<ReturnType<typeof chromium.launch>>,
  entry: CatalogCase,
  state: string,
  output: string,
  viewport: DynamicViewport & { scale: number },
  conditionId: string,
  sourceSha256: string,
): Promise<{
  state: JsonValue;
  boundary: JsonValue;
  resourceProvenance: ChromeResourceProvenance;
  fontIdentityObservation: ChromeFontObservation;
}> {
  const page = await browser.newPage({
    viewport: { width: viewport.width, height: viewport.height },
    deviceScaleFactor: viewport.scale,
  });
  const resources = new ChromeResourceBodyCollector(page, conditionId, sourceSha256, output);
  try {
    const fixture = path.resolve("tests/fixtures/html", entry.fixture);
    await page.goto(`file://${fixture}`, { waitUntil: "networkidle" });
    await performChromeCatalogOperation(page, entry, state);
    const fontIdentityObservation = await captureChromePlatformFonts(page);
    const spec = catalogOperationSpec(entry, state);
    const actualState = await readChromeCatalogState(page, spec.expression);
    const boundary = await inspectChromeCatalogBoundary(page, entry.id, state, viewport);
    await page.screenshot({ path: output, fullPage: false });
    const resourceUrls = await page.evaluate(() =>
      performance.getEntriesByType("resource").map((entry) => entry.name),
    );
    const resourceProvenance = await resources.finish(resourceUrls);
    return {
      state: actualState as JsonValue,
      boundary,
      resourceProvenance,
      fontIdentityObservation,
    };
  } finally {
    await page.close();
  }
}

async function performChromeCatalogOperation(
  page: Page,
  entry: CatalogCase,
  state: string,
): Promise<void> {
  if (entry.mode === "script") {
    if (entry.id === "events") await applyEventsInteraction(page);
    await waitForCatalogDynamicState(page, entry.id);
    return;
  }
  if (entry.id === "input-state") {
    await applyChromeOperation(page, state as InputStateOperation);
    return;
  }
  if (entry.id !== "navigation") return;
  const destinations =
    state === "all-toc-links"
      ? ["a", "b", "c"]
      : [state === "top" ? "a" : state === "middle" ? "b" : "c"];
  for (const destination of destinations) {
    await page.locator(`.toc-item[href="#${destination}"]`).click();
    await page.evaluate(
      () =>
        new Promise<void>((resolve) =>
          requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
        ),
    );
  }
}

async function readChromeCatalogState(page: Page, expression: string): Promise<JsonValue> {
  return (await page.evaluate((stateExpression) => {
    const script = document.createElement("script");
    script.textContent = `globalThis.__krrCatalogState = (${stateExpression});`;
    document.head.append(script);
    const value =
      (globalThis as typeof globalThis & { __krrCatalogState?: JsonValue }).__krrCatalogState ??
      null;
    delete (globalThis as typeof globalThis & { __krrCatalogState?: JsonValue }).__krrCatalogState;
    script.remove();
    return value;
  }, expression)) as JsonValue;
}

async function waitForCatalogDynamicState(page: Page, caseId: string): Promise<void> {
  await page.waitForFunction(() => document.readyState === "complete");
  if (caseId === "resources") await page.evaluate(() => document.fonts.ready);
  if (["async", "observers", "resources"].includes(caseId)) {
    await page.waitForFunction(() => {
      const state = document.querySelector("#state")?.textContent ?? "";
      return state !== "" && state !== "initial" && state !== "waiting";
    });
  }
}

export async function inspectChromeCatalogBoundary(
  page: Page,
  caseId: string,
  state: string,
  viewport: DynamicViewport,
): Promise<JsonValue> {
  const args = { state, width: viewport.width };
  switch (caseId) {
    case "typography":
      return page.evaluate(inspectTypographyBoundary, args);
    case "layout":
      return page.evaluate(inspectLayoutBoundary, args);
    case "table":
      return page.evaluate(inspectTableBoundary, args);
    case "decoration":
      return page.evaluate(inspectDecorationBoundary, args);
    case "navigation":
      return page.evaluate(inspectNavigationBoundary, args);
    case "input-state":
      return page.evaluate(inspectInputBoundary, args);
    case "dynamic-dom":
      return page.evaluate(inspectDynamicDomBoundary, args);
    case "events":
      return page.evaluate(inspectEventsBoundary, args);
    case "async":
      return page.evaluate(inspectAsyncBoundary, args);
    case "observers":
      return page.evaluate(inspectObserversBoundary, args);
    case "resources":
      return page.evaluate(inspectResourcesBoundary, args);
    default:
      throw new Error(`No Chrome boundary inspector for ${caseId}`);
  }
}

function inspectDynamicDomBoundary({ state }: { state: string; width: number }): JsonValue {
  if (state !== "expected-state") throw new Error(`Unknown dynamic DOM state: ${state}`);
  const removed = document.querySelector("#old");
  const item = document.querySelector<HTMLParagraphElement>("#root p.ready");
  if (
    removed ||
    !item ||
    item.textContent !== "追加" ||
    item.dataset.state !== "done" ||
    getComputedStyle(item).color !== "rgb(20, 80, 140)" ||
    !item.isConnected
  )
    throw new Error("Dynamic DOM mutation boundary missing");
  return {
    removed: true,
    text: item.textContent,
    state: item.dataset.state,
    color: getComputedStyle(item).color,
  };
}

function inspectEventsBoundary({ state }: { state: string; width: number }): JsonValue {
  if (state !== "expected-state") throw new Error(`Unknown events state: ${state}`);
  const field = document.querySelector<HTMLInputElement>("#field");
  const result = document.querySelector("#result")?.textContent;
  const key = document.querySelector("#keyboard")?.textContent;
  const expected = "capture>bubble>input>capture>bubble>submit";
  if (field?.value !== "入力" || result !== expected || key !== "Shift")
    throw new Error("Real event delivery boundary missing");
  return { inputValue: field.value, eventOrder: result, key };
}

function inspectAsyncBoundary({ state }: { state: string; width: number }): JsonValue {
  if (state !== "expected-state") throw new Error(`Unknown async state: ${state}`);
  const output = document.querySelector("#state")?.textContent ?? "";
  const microtask = output.indexOf("microtask");
  const ready = output.indexOf("ready");
  const timer = output.indexOf("timer");
  if (microtask < 0 || ready <= microtask || timer <= ready)
    throw new Error("Microtask, DOMContentLoaded, and timer order boundary missing");
  return { microtaskBeforeReady: true, readyBeforeTimer: true, output };
}

function inspectObserversBoundary({ state }: { state: string; width: number }): JsonValue {
  if (state !== "expected-state") throw new Error(`Unknown observers state: ${state}`);
  const output = document.querySelector("#state")?.textContent ?? "";
  let callbacks: unknown;
  try {
    callbacks = JSON.parse(output);
  } catch {
    throw new Error("Observer callback log is not valid JSON");
  }
  const observed = document.querySelector("#observed");
  const bounds = observed?.getBoundingClientRect();
  if (
    !Array.isArray(callbacks) ||
    callbacks.join(",") !== "takeRecords,mutation,resize,intersection" ||
    !observed ||
    !bounds ||
    bounds.width <= 0 ||
    bounds.height <= 0
  )
    throw new Error("Mutation, resize, and intersection observer boundary missing");
  return { callbacks, observedWidth: bounds.width, observedHeight: bounds.height };
}

function inspectResourcesBoundary({ state }: { state: string; width: number }): JsonValue {
  if (state !== "expected-state") throw new Error(`Unknown resources state: ${state}`);
  const image = document.querySelector<HTMLImageElement>("#pixel");
  const output = document.querySelector("#state")?.textContent;
  const fonts = document.documentElement.dataset.fonts;
  if (
    image?.complete !== true ||
    image.naturalWidth !== 1 ||
    image.naturalHeight !== 1 ||
    !image.currentSrc.startsWith("data:image/gif;base64,") ||
    output !== "loaded" ||
    fonts !== "ready" ||
    document.fonts.status !== "loaded"
  )
    throw new Error("Image resource and font-ready boundary missing");
  return { imageLoaded: true, naturalWidth: image.naturalWidth, fonts };
}

function inspectTypographyBoundary({ state }: { state: string; width: number }): JsonValue {
  const fractional = (): JsonValue => {
    const element = document.querySelector(".mixed");
    if (!element) throw new Error("Typography boundary target missing");
    const style = getComputedStyle(element);
    if (style.fontSize !== "13.1px" || !style.lineHeight.includes("px"))
      throw new Error("fractional typography boundary missing");
    return { fontSize: style.fontSize, lineHeight: style.lineHeight };
  };
  const nowrap = (): JsonValue => {
    const element = document.querySelector(".nowrap");
    if (!element) throw new Error("Typography boundary target missing");
    if (
      getComputedStyle(element).whiteSpace !== "nowrap" ||
      element.scrollWidth <= element.clientWidth
    )
      throw new Error("nowrap clipping boundary missing");
    return { whiteSpace: "nowrap", overflow: element.scrollWidth - element.clientWidth };
  };
  const longLine = (): JsonValue => {
    const element = document.querySelector("#long-line");
    if (!element) throw new Error("Typography boundary target missing");
    const range = document.createRange();
    range.selectNodeContents(element);
    const lineCount = range.getClientRects().length;
    if (lineCount < 2) throw new Error("authored long-line does not wrap");
    return { lineCount };
  };
  const mixed = (): JsonValue => {
    const element = document.querySelector(".mixed");
    if (!element) throw new Error("Typography boundary target missing");
    const text = element.textContent ?? "";
    if (
      !/[\u3040-\u30ff\u4e00-\u9fff]/.test(text) ||
      !/[A-Za-z]/.test(text) ||
      !element.querySelector("strong")
    )
      throw new Error("mixed typography boundary missing");
    return { mixedScripts: true, strong: true, fontReady: document.fonts.status === "loaded" };
  };
  switch (state) {
    case "fractional-metrics":
      return fractional();
    case "nowrap":
      return nowrap();
    case "long-line":
      return longLine();
    case "ja-en-mixed":
      return mixed();
    default:
      throw new Error(`Unknown typography state: ${state}`);
  }
}

function inspectLayoutBoundary({ state, width }: { state: string; width: number }): JsonValue {
  if (state === "narrow-viewport") {
    const cards = [...document.querySelectorAll(".flex>.card")];
    const first = cards.at(0);
    const second = cards.at(1);
    if (!first || !second || cards.length !== 2)
      throw new Error("responsive flex boundary targets missing");
    const firstTop = first.getBoundingClientRect().top;
    const secondTop = second.getBoundingClientRect().top;
    const wrapped = secondTop > firstTop;
    if (wrapped !== width <= 700) throw new Error(`responsive flex wrapping differs at ${width}px`);
    return {
      width,
      wrapped,
      rows: new Set(cards.map((card) => Math.round(card.getBoundingClientRect().top))).size,
    };
  }
  if (state === "multi-column") {
    const cards = [...document.querySelectorAll(".grid>.card")];
    const grid = document.querySelector(".grid");
    const first = cards.at(0);
    const second = cards.at(1);
    if (
      !grid ||
      !first ||
      !second ||
      getComputedStyle(grid).display !== "grid" ||
      cards.length !== 2 ||
      second.getBoundingClientRect().left <= first.getBoundingClientRect().left
    )
      throw new Error("grid columns boundary missing");
    return { columns: 2 };
  }
  const element = document.querySelector("#min-content");
  if (!element) throw new Error("min-content target missing");
  const range = document.createRange();
  range.selectNodeContents(element);
  const lineCount = range.getClientRects().length;
  if (lineCount < 2 || !element.textContent?.includes("unbreakabletoken"))
    throw new Error("min-content wrapping boundary missing");
  return { wraps: true, lineCount, unbreakableToken: true };
}

function inspectTableBoundary({ state }: { state: string; width: number }): JsonValue {
  const table = document.querySelector("table");
  const badge = document.querySelector(".badge");
  if (!table || !badge) throw new Error("Table boundary target missing");
  if (state === "collapsed-border" && getComputedStyle(table).borderCollapse !== "collapse")
    throw new Error("collapsed border boundary missing");
  if (state === "long-status" && (badge.textContent?.length ?? 0) < 10)
    throw new Error("long status text missing");
  if (
    state === "rich-cell" &&
    (!document.querySelector("tbody strong") ||
      !document.querySelector("tbody small") ||
      document.querySelector("td[rowspan='2']")?.getAttribute("rowspan") !== "2")
  )
    throw new Error("rich table boundary missing");
  return {
    borderCollapse: getComputedStyle(table).borderCollapse,
    longStatus: (badge.textContent?.length ?? 0) >= 10,
    richCell: Boolean(document.querySelector("tbody strong, tbody small")),
  };
}

function inspectDecorationBoundary({ state }: { state: string; width: number }): JsonValue {
  const alert = document.querySelector(".alert");
  const overlay = document.querySelector(".overlay");
  if (!alert || !overlay) throw new Error("Decoration boundary target missing");
  const a = alert.getBoundingClientRect();
  const o = overlay.getBoundingClientRect();
  const overlaps = a.left < o.right && a.right > o.left && a.top < o.bottom && a.bottom > o.top;
  const alertStyle = getComputedStyle(alert);
  const overlayStyle = getComputedStyle(overlay);
  if (!overlaps) throw new Error("decoration paint regions do not overlap");
  if (state === "stacked" && Number(alertStyle.zIndex) <= Number(overlayStyle.zIndex))
    throw new Error("alert stacking order missing");
  if (state === "clipped" && !overlayStyle.clipPath.startsWith("inset("))
    throw new Error("clip-path boundary missing");
  if (state === "translucent" && overlayStyle.opacity !== "0.8")
    throw new Error("translucent opacity boundary missing");
  return {
    overlaps,
    alertZ: Number(alertStyle.zIndex),
    overlayOpacity: Number(overlayStyle.opacity),
    clipPath: overlayStyle.clipPath,
  };
}

function inspectNavigationBoundary({ state }: { state: string; width: number }): JsonValue {
  const links = document.querySelectorAll(".toc-item[href^='#']").length;
  if (state === "all-toc-links" && links !== 3) throw new Error("TOC inventory is incomplete");
  if (state !== "all-toc-links") {
    const destination = state === "top" ? "a" : state === "middle" ? "b" : "c";
    const target = document.querySelector(`#${destination}`);
    if (
      location.hash !== `#${destination}` ||
      !target ||
      target.getBoundingClientRect().height <= 0
    )
      throw new Error("authored TOC navigation did not settle at its destination");
  }
  return { hash: location.hash, scrollY: window.scrollY, allLinks: links };
}

function inspectInputBoundary({ state }: { state: string; width: number }): JsonValue {
  const summary = JSON.parse(document.querySelector("#state-summary")?.textContent ?? "{}");
  if (state === "filled" && summary.name !== "架空の名称")
    throw new Error("input operation did not fill authored value");
  if (state === "selected" && summary.tab !== "advanced")
    throw new Error("tab operation did not select advanced tab");
  if (state === "expanded" && summary.expanded !== true)
    throw new Error("accordion operation did not expand");
  if (state === "disabled" && summary.disabled !== true)
    throw new Error("disabled control boundary missing");
  if (state === "focus-visible" && document.activeElement?.id !== "name")
    throw new Error("Tab operation did not focus the name input");
  return summary;
}

function isCapturedCondition(value: JsonValue): boolean {
  return (
    typeof value === "object" &&
    value !== null &&
    !Array.isArray(value) &&
    value.status === "captured"
  );
}
