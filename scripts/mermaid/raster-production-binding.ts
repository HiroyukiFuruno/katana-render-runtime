import { createHash } from "node:crypto";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";
import type { JsonValue } from "./mermaid-pair-manifest";
import type { JsonFields, JsonInput, JsonObject } from "./raster-json-input";

export type RasterRole = "official" | "candidate";
export type RasterIdentityInteger = number | string;
export interface RasterFileIdentity {
  path: string;
  canonical: string;
  sha256: string;
  size: number;
  dev: RasterIdentityInteger;
  ino: RasterIdentityInteger;
  mode: number;
}
export interface RasterEndpoint {
  name: string;
  kind: "producer" | "browser" | "playwright" | "lock" | "runtime" | "font" | "config";
  version: string;
  before: RasterFileIdentity;
  end: RasterFileIdentity;
}
export interface RasterCaptureState {
  domSha256Before: string;
  domSha256After: string;
  svg: {
    policy: "browser-domparser-html-svg-outerhtml-v1";
    inputSha256: string;
    expectedDomSha256: string;
    assignedDomSha256: string;
    beforeDomSha256: string;
    endDomSha256: string;
  };
  bounds: { x: number; y: number; width: number; height: number };
  viewport: { width: number; height: number };
  deviceScaleFactor: number;
  theme: string;
  resized: boolean;
  fontsReady: true;
  imagesDecoded: true;
}
export interface RasterBindingRow {
  slug: string;
  sourceSvgSha256: string;
  renderedSvgSha256: string;
  loadedSvgSha256: string;
  rawSvgSha256: string;
  stageSvgSha256: string;
  canonicalSourceSha256: string;
  provenanceSha256: string;
  pngSha256: string;
  publishedPngSha256: string;
  png: { width: number; height: number };
  configSha256: string;
  capture: RasterCaptureState;
}
export interface RasterProductionReceipt {
  schema: "katana-mermaid-raster-production-v1";
  diagnosticOnly: true;
  accepted: false;
  producerSucceeded: true;
  role: RasterRole;
  attempt: string;
  nonce: string;
  inventory: string[];
  endpoints: RasterEndpoint[];
  rows: RasterBindingRow[];
}
export interface TrustedRasterInput {
  receiptSha256: string;
  role: RasterRole;
  attempt: string;
  nonce: string;
  inventory: string[];
}
export interface RasterPairObservation {
  slug: string;
  role: RasterRole;
  sourceSvg: Buffer;
  renderedSvg: Buffer;
  png: Buffer;
  publishedPng: Buffer;
  pngDimensions: { width: number; height: number };
  config: JsonValue;
  manifest: Pick<
    RasterBindingRow,
    "sourceSvgSha256" | "renderedSvgSha256" | "pngSha256" | "configSha256"
  >;
  rawSvgSha256: string;
  stageSvgSha256: string;
  canonicalSourceSha256: string;
  provenanceSha256: string;
  loadedSvgSha256: string;
  capture: RasterCaptureState;
}

export function rasterSha256(bytes: Buffer): string {
  return createHash("sha256").update(bytes).digest("hex");
}

function record(value: JsonInput, keys: string[], label: string): JsonObject {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`Invalid ${label}: object required`);
  }
  const result = value as JsonObject;
  if (Object.keys(result).sort().join("\0") !== [...keys].sort().join("\0")) {
    throw new Error(`Invalid ${label}: exact fields required`);
  }
  return result;
}
function text(value: JsonInput, label: string): string {
  if (typeof value !== "string" || value.trim() === "") throw new Error(`Invalid ${label}`);
  return value;
}
function digest(value: JsonInput): string {
  const result = text(value, "digest");
  if (!/^[a-f0-9]{64}$/.test(result)) throw new Error("Invalid digest");
  return result;
}
function number(value: JsonInput, minimum: number): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum) {
    throw new Error("Invalid capture number");
  }
  return value;
}
function integer(value: JsonInput, minimum = 0): number {
  const result = number(value, minimum);
  if (!Number.isSafeInteger(result)) throw new Error("Invalid identity integer");
  return result;
}
function identityInteger(value: JsonInput, minimum: bigint, label: string): RasterIdentityInteger {
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value) || value < Number(minimum))
      throw new Error(`Invalid ${label}`);
    return value;
  }
  if (typeof value !== "string" || !/^(0|[1-9][0-9]*)$/.test(value))
    throw new Error(`Invalid ${label}`);
  const parsed = BigInt(value);
  if (parsed < minimum) throw new Error(`Invalid ${label}`);
  return value;
}
function list(value: JsonInput): JsonInput[] {
  if (!Array.isArray(value)) throw new Error("Invalid inventory: array required");
  return value;
}
function role(value: JsonInput): RasterRole {
  if (value !== "official" && value !== "candidate") throw new Error("Invalid raster role");
  return value;
}
function absolute(value: JsonInput): string {
  const result = text(value, "absolute path");
  if (!path.isAbsolute(result) || path.normalize(result) !== result)
    throw new Error("Invalid absolute path");
  return result;
}
function dimensions(value: JsonInput): { width: number; height: number } {
  const row = record(value, ["width", "height"], "dimensions");
  return { width: integer(row.width, 1), height: integer(row.height, 1) };
}
export function parseRasterFileIdentity(value: JsonInput): RasterFileIdentity {
  const row = record(
    value,
    ["path", "canonical", "sha256", "size", "dev", "ino", "mode"],
    "file identity",
  );
  return {
    path: absolute(row.path),
    canonical: absolute(row.canonical),
    sha256: digest(row.sha256),
    size: integer(row.size, 1),
    dev: identityInteger(row.dev, 0n, "identity device"),
    ino: identityInteger(row.ino, 1n, "identity inode"),
    mode: integer(row.mode),
  };
}
export function sameRasterFileIdentity(
  left: RasterFileIdentity,
  right: RasterFileIdentity,
): boolean {
  return (
    left.path === right.path &&
    left.canonical === right.canonical &&
    left.sha256 === right.sha256 &&
    left.size === right.size &&
    identityIntegerKey(left.dev, 0n) === identityIntegerKey(right.dev, 0n) &&
    identityIntegerKey(left.ino, 1n) === identityIntegerKey(right.ino, 1n) &&
    left.mode === right.mode
  );
}
function identityIntegerKey(value: RasterIdentityInteger, minimum: bigint): string {
  return String(identityInteger(value, minimum, "identity integer"));
}
function capture(value: JsonInput | JsonFields<RasterCaptureState>): RasterCaptureState {
  const row = record(
    value,
    [
      "domSha256Before",
      "domSha256After",
      "svg",
      "bounds",
      "viewport",
      "deviceScaleFactor",
      "theme",
      "resized",
      "fontsReady",
      "imagesDecoded",
    ],
    "capture state",
  );
  const svg = record(
    row.svg,
    [
      "policy",
      "inputSha256",
      "expectedDomSha256",
      "assignedDomSha256",
      "beforeDomSha256",
      "endDomSha256",
    ],
    "SVG DOM binding",
  );
  if (svg.policy !== "browser-domparser-html-svg-outerhtml-v1")
    throw new Error("Unsupported SVG DOM serialization policy");
  const dom = digest(svg.expectedDomSha256);
  equal(
    [digest(svg.assignedDomSha256), digest(svg.beforeDomSha256), digest(svg.endDomSha256)],
    [dom, dom, dom],
    "Expected/assigned/captured SVG DOM differs",
  );
  const bounds = record(row.bounds, ["x", "y", "width", "height"], "bounds");
  if (typeof row.resized !== "boolean" || row.fontsReady !== true || row.imagesDecoded !== true) {
    throw new Error("Capture fonts/images/state not ready");
  }
  const result: RasterCaptureState = {
    domSha256Before: digest(row.domSha256Before),
    domSha256After: digest(row.domSha256After),
    svg: {
      policy: "browser-domparser-html-svg-outerhtml-v1",
      inputSha256: digest(svg.inputSha256),
      expectedDomSha256: dom,
      assignedDomSha256: dom,
      beforeDomSha256: dom,
      endDomSha256: dom,
    },
    bounds: {
      x: number(bounds.x, -Number.MAX_SAFE_INTEGER),
      y: number(bounds.y, -Number.MAX_SAFE_INTEGER),
      width: number(bounds.width, Number.MIN_VALUE),
      height: number(bounds.height, Number.MIN_VALUE),
    },
    viewport: dimensions(row.viewport),
    deviceScaleFactor: number(row.deviceScaleFactor, Number.MIN_VALUE),
    theme: text(row.theme, "theme"),
    resized: row.resized,
    fontsReady: true,
    imagesDecoded: true,
  };
  if (result.domSha256Before !== result.domSha256After)
    throw new Error("Capture DOM endpoint drift");
  return result;
}
function parseRow(value: JsonInput): RasterBindingRow {
  const row = record(
    value,
    [
      "slug",
      "sourceSvgSha256",
      "renderedSvgSha256",
      "loadedSvgSha256",
      "rawSvgSha256",
      "stageSvgSha256",
      "canonicalSourceSha256",
      "provenanceSha256",
      "pngSha256",
      "publishedPngSha256",
      "png",
      "configSha256",
      "capture",
    ],
    "raster row",
  );
  const result: RasterBindingRow = {
    slug: text(row.slug, "slug"),
    sourceSvgSha256: digest(row.sourceSvgSha256),
    renderedSvgSha256: digest(row.renderedSvgSha256),
    loadedSvgSha256: digest(row.loadedSvgSha256),
    rawSvgSha256: digest(row.rawSvgSha256),
    stageSvgSha256: digest(row.stageSvgSha256),
    canonicalSourceSha256: digest(row.canonicalSourceSha256),
    provenanceSha256: digest(row.provenanceSha256),
    pngSha256: digest(row.pngSha256),
    publishedPngSha256: digest(row.publishedPngSha256),
    png: dimensions(row.png),
    configSha256: digest(row.configSha256),
    capture: capture(row.capture),
  };
  if (
    result.sourceSvgSha256 !== result.renderedSvgSha256 ||
    result.sourceSvgSha256 !== result.loadedSvgSha256
  ) {
    throw new Error("Strict source/rendered/loaded SVG differs");
  }
  if (result.capture.svg.inputSha256 !== result.sourceSvgSha256)
    throw new Error("Capture SVG input differs from source bytes");
  if (result.pngSha256 !== result.publishedPngSha256)
    throw new Error("Published PNG differs from screenshot bytes");
  return result;
}
function inventory(values: JsonInput): string[] {
  const result = list(values).map((value) => text(value, "fixture slug"));
  if (
    result.length === 0 ||
    new Set(result).size !== result.length ||
    result.some(
      (slug) => !/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(slug) || slug === "." || slug === "..",
    )
  ) {
    throw new Error("Invalid exact fixture inventory");
  }
  return result;
}
function equal<Actual, Expected>(actual: Actual, expected: Expected, label: string): void {
  if (!isDeepStrictEqual(actual, expected)) throw new Error(label);
}
export function parseRasterProductionReceipt(bytes: Buffer): RasterProductionReceipt {
  const parsed: JsonInput = JSON.parse(bytes.toString("utf8"));
  const root = record(
    parsed,
    [
      "schema",
      "diagnosticOnly",
      "accepted",
      "producerSucceeded",
      "role",
      "attempt",
      "nonce",
      "inventory",
      "endpoints",
      "rows",
    ],
    "raster receipt",
  );
  if (
    root.schema !== "katana-mermaid-raster-production-v1" ||
    root.diagnosticOnly !== true ||
    root.accepted !== false ||
    root.producerSucceeded !== true
  ) {
    throw new Error("Unsupported or incomplete diagnostic raster receipt");
  }
  const endpoints = list(root.endpoints).map((value): RasterEndpoint => {
    const row = record(
      value,
      ["name", "kind", "version", "before", "end"],
      "producer/tool endpoint",
    );
    const kind = row.kind;
    if (
      kind !== "producer" &&
      kind !== "browser" &&
      kind !== "playwright" &&
      kind !== "lock" &&
      kind !== "runtime" &&
      kind !== "font" &&
      kind !== "config"
    ) {
      throw new Error("Invalid producer/tool kind");
    }
    const endpoint: RasterEndpoint = {
      name: text(row.name, "endpoint name"),
      kind,
      version: text(row.version, "tool version"),
      before: parseRasterFileIdentity(row.before),
      end: parseRasterFileIdentity(row.end),
    };
    if (!sameRasterFileIdentity(endpoint.before, endpoint.end))
      throw new Error("Producer/tool endpoint drift");
    return endpoint;
  });
  if (
    new Set(endpoints.map((row) => row.name)).size !== endpoints.length ||
    ["producer", "browser", "playwright", "lock", "runtime", "font", "config"].some(
      (kind) => !endpoints.some((row) => row.kind === kind),
    )
  ) {
    throw new Error("Incomplete or duplicate producer/tool endpoints");
  }
  const result: RasterProductionReceipt = {
    schema: "katana-mermaid-raster-production-v1",
    diagnosticOnly: true,
    accepted: false,
    producerSucceeded: true,
    role: role(root.role),
    attempt: absolute(root.attempt),
    nonce: text(root.nonce, "nonce"),
    inventory: inventory(root.inventory),
    endpoints,
    rows: list(root.rows).map(parseRow),
  };
  equal(
    result.rows.map((row) => row.slug),
    result.inventory,
    "Receipt exact fixture inventory mismatch",
  );
  return result;
}

// 生成者が観測した証跡を直列化するだけであり、信頼済み pin の取得は呼出元の責務。
export function createRasterProductionReceipt(receipt: RasterProductionReceipt): Buffer {
  const bytes = Buffer.from(`${JSON.stringify(receipt, null, 2)}\n`);
  parseRasterProductionReceipt(bytes);
  return bytes;
}

export function verifyRasterProductionBinding(
  receiptBytes: Buffer,
  trusted: TrustedRasterInput,
  pairs: RasterPairObservation[],
  observedEndpoints: RasterEndpoint[],
): RasterProductionReceipt {
  if (rasterSha256(receiptBytes) !== digest(trusted.receiptSha256))
    throw new Error("Trusted raster receipt SHA mismatch");
  const receipt = parseRasterProductionReceipt(receiptBytes);
  equal(
    [receipt.role, receipt.attempt, receipt.nonce],
    [role(trusted.role), absolute(trusted.attempt), text(trusted.nonce, "trusted nonce")],
    "Raster role/attempt/nonce mismatch",
  );
  equal(
    receipt.inventory,
    inventory(trusted.inventory),
    "Trusted exact fixture inventory mismatch",
  );
  equal(
    pairs.map((pair) => pair.slug),
    receipt.inventory,
    "Observed exact fixture inventory mismatch",
  );
  if (
    observedEndpoints.length !== receipt.endpoints.length ||
    observedEndpoints.some((endpoint, index) => {
      const expected = receipt.endpoints[index];
      return (
        expected === undefined ||
        endpoint.name !== expected.name ||
        endpoint.kind !== expected.kind ||
        endpoint.version !== expected.version ||
        !sameRasterFileIdentity(endpoint.before, expected.before) ||
        !sameRasterFileIdentity(endpoint.end, expected.end)
      );
    })
  ) {
    throw new Error("Observed producer/tool endpoints mismatch");
  }
  for (const [index, row] of receipt.rows.entries()) {
    const pair = pairs[index];
    if (pair === undefined || pair.role !== receipt.role)
      throw new Error("Observed raster role mismatch");
    const source = rasterSha256(pair.sourceSvg),
      rendered = rasterSha256(pair.renderedSvg),
      png = rasterSha256(pair.png);
    equal(
      [source, rendered, pair.loadedSvgSha256],
      [row.sourceSvgSha256, row.renderedSvgSha256, row.loadedSvgSha256],
      "SVG differs from trusted raster input",
    );
    equal(
      [png, rasterSha256(pair.publishedPng)],
      [row.pngSha256, row.publishedPngSha256],
      "PNG differs from trusted raster output",
    );
    equal(
      pair.manifest,
      {
        sourceSvgSha256: source,
        renderedSvgSha256: rendered,
        pngSha256: png,
        configSha256: row.configSha256,
      },
      "Pair manifest differs from trusted raster binding",
    );
    equal(
      rasterSha256(Buffer.from(JSON.stringify(pair.config))),
      row.configSha256,
      "Capture config differs from trusted raster input",
    );
    equal(pair.pngDimensions, row.png, "PNG dimensions differ from trusted raster output");
    equal(
      [pair.rawSvgSha256, pair.stageSvgSha256, pair.canonicalSourceSha256, pair.provenanceSha256],
      [row.rawSvgSha256, row.stageSvgSha256, row.canonicalSourceSha256, row.provenanceSha256],
      "Source/stage/provenance differs from trusted raster input",
    );
    equal(capture(pair.capture), row.capture, "Capture state differs from trusted raster receipt");
    rasterCaptureContext(pair.config, pair.capture);
  }
  return receipt;
}

// 宣言configと実測captureを結合し、role間比較では描画内容に依存するbounds/DOMを除く。
export function rasterCaptureContext(config: JsonValue, actual: RasterCaptureState) {
  if (config === null || typeof config !== "object" || Array.isArray(config)) {
    throw new Error("Strict raster capture config required");
  }
  const viewport = config.viewport;
  if (viewport === null || typeof viewport !== "object" || Array.isArray(viewport)) {
    throw new Error("Strict raster capture viewport required");
  }
  const expectedViewport = dimensions({ width: viewport.width, height: viewport.height });
  const expectedScale = number(viewport.deviceScaleFactor, Number.MIN_VALUE);
  if (
    config.referenceMode !== "strictVendorOriginal" ||
    !["capture-box-only-raw-svg-v1", "raw-svg-viewbox-viewport-before-assignment-v1"].includes(
      String(config.captureResize),
    ) ||
    !isDeepStrictEqual(config.sourceFontUrls, []) ||
    config.externalImageWaitPolicy !== "svg-image-load-decode-positive-dimensions-and-frame-v2"
  ) {
    throw new Error("Unsupported strict raster capture policy");
  }
  equal(
    [actual.theme, actual.viewport, actual.deviceScaleFactor, actual.resized],
    [text(config.theme, "capture theme"), expectedViewport, expectedScale, false],
    "Actual raster capture context differs from manifest config",
  );
  if (config.captureResize === "raw-svg-viewbox-viewport-before-assignment-v1") {
    equal(
      actual.viewport,
      { width: actual.bounds.width, height: actual.bounds.height },
      "Prepared viewport differs from raw SVG capture bounds",
    );
  }
  if (actual.fontsReady !== true || actual.imagesDecoded !== true) {
    throw new Error("Actual raster capture readiness differs from strict policy");
  }
  return {
    theme: actual.theme,
    viewport: actual.viewport,
    deviceScaleFactor: actual.deviceScaleFactor,
    resized: actual.resized,
    fontsReady: actual.fontsReady,
    imagesDecoded: actual.imagesDecoded,
    svgSerializationPolicy: actual.svg.policy,
    captureResize: config.captureResize,
    sourceFontUrls: config.sourceFontUrls,
    externalImageWaitPolicy: config.externalImageWaitPolicy,
  };
}
