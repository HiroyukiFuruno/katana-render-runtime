import fs from "node:fs";
import path from "node:path";
import { parseArgs } from "node:util";
import { z } from "zod";
import { fixturePath, parseMarkerInventory } from "./svg-marker-browser-capture";
import {
  countRgbaDifferences,
  pixelsDiffer,
  type Rgba,
  readStableRgbaPng,
  rgbaAt,
  sha256,
} from "./svg-marker-image";
import {
  type NativeFailureArtifact,
  readNativeFailureArtifacts,
} from "./svg-marker-native-artifact";

export { countRgbaDifferences, pixelsDiffer } from "./svg-marker-image";

const WIDTH = 300;
const MAX_HEIGHT = 300;
const INVENTORY_RELATIVE =
  "tests/fixtures/svg-engine/browser-corrections/marker/capture-cases.json";
const FIXTURE_RELATIVE = "tests/fixtures/svg-engine/upstream/resvg/tests";
const sha = z.string().regex(/^[a-f0-9]{64}$/);
const byte = z.number().int().min(0).max(255);
const rgba = z.tuple([byte, byte, byte, byte]);
const relativeSvg = z.string().regex(/^tests\/painting\/(?:context|marker|overflow)\/[^/]+\.svg$/);
const diagnosticPixel = z.strictObject({
  x: z
    .number()
    .int()
    .min(0)
    .max(WIDTH - 1),
  y: z
    .number()
    .int()
    .min(0)
    .max(MAX_HEIGHT - 1),
  actual: rgba,
  expected: rgba,
});
const capture = z.strictObject({
  input: relativeSvg,
  inputSha256: sha,
  upstreamPngSha256: sha,
  requestedWidth: z.literal(WIDTH),
  requestedHeight: z.number().int().positive().max(MAX_HEIGHT),
  png: z.strictObject({ path: z.string(), sha256: sha }),
  resources: z.array(z.strictObject({ path: z.string(), sha256: sha })),
  font: z.strictObject({ path: z.string(), sha256: sha }),
});
const captureManifestSchema = z.strictObject({
  schema: z.literal("krr-svg-marker-browser-capture-v1"),
  diagnosticOnly: z.literal(true),
  requestedWidth: z.literal(WIDTH),
  inventorySha256: sha,
  browserVersion: z.string().min(1),
  tool: z.strictObject({ executable: z.string().min(1), sha256: sha }),
  captures: z.array(capture).length(48),
});
type MarkerInventory = ReturnType<typeof parseMarkerInventory>;
type MarkerInput = MarkerInventory["cases"][number];
type CaptureManifest = z.infer<typeof captureManifestSchema>;
type NativePixel = z.infer<typeof diagnosticPixel>;

export interface NativeDiagnosticCase {
  input: string;
  samples: NativePixel[];
}

export function parseNativeDiagnostics(
  text: string,
  crateRoot: string,
  allowedInputs: readonly MarkerInput[],
): NativeDiagnosticCase[] {
  const allowed = new Map(allowedInputs.map((entry) => [entry.input, entry]));
  if (allowed.size !== 48) throw new Error("invalid marker input set");
  const groups: NativeDiagnosticCase[] = [];
  let pending: NativePixel[] = [];
  for (const line of text.split(/\r?\n/u)) {
    appendNativePixel(line, pending);
    const inputPath = nativeAssertionPath(line);
    if (inputPath === undefined) continue;
    groups.push(finishNativeCase(inputPath, pending, crateRoot, allowed, groups));
    pending = [];
  }
  requireCompleteNativeInventory(pending, groups, allowed.size);
  return groups;
}

function appendNativePixel(line: string, pending: NativePixel[]): void {
  if (!line.includes("pixel (") || !line.includes("differs:")) return;
  pending.push(parsePixelLine(line));
  if (pending.length > 8) throw new Error("too many logged native pixels");
}

function nativeAssertionPath(line: string): string | undefined {
  return line.match(/assertion `left == right` failed: upstream fixture (.+)$/u)?.[1];
}

function finishNativeCase(
  inputPath: string,
  samples: NativePixel[],
  crateRoot: string,
  allowed: ReadonlyMap<string, MarkerInput>,
  existing: readonly NativeDiagnosticCase[],
): NativeDiagnosticCase {
  if (samples.length === 0) throw new Error("native fixture has no logged pixels");
  const input = normalizeFixturePath(inputPath, crateRoot);
  const dimensions = allowed.get(input);
  if (!dimensions || existing.some((entry) => entry.input === input)) {
    throw new Error("native fixture path is unknown or duplicated");
  }
  validateNativeSamples(samples, dimensions.requestedWidth, dimensions.requestedHeight);
  return { input, samples };
}

function validateNativeSamples(
  samples: readonly NativePixel[],
  width: number,
  height: number,
): void {
  const positions = samples.map(({ x, y }) => `${x},${y}`);
  const hasDuplicatePosition = new Set(positions).size !== samples.length;
  const hasMatchingPixel = samples.some(({ actual, expected }) => !pixelsDiffer(actual, expected));
  const hasOutOfBoundsPixel = samples.some(({ x, y }) => x >= width || y >= height);
  if (hasDuplicatePosition || hasMatchingPixel || hasOutOfBoundsPixel) {
    throw new Error("native pixel diagnostics are duplicated, matching, or outside case bounds");
  }
}

function requireCompleteNativeInventory(
  pending: readonly NativePixel[],
  groups: readonly NativeDiagnosticCase[],
  expectedCount: number,
): void {
  if (pending.length !== 0 || groups.length !== expectedCount) {
    throw new Error("native diagnostics do not cover the fixed marker inventory");
  }
}

function parsePixelLine(line: string): NativePixel {
  const match = line.match(
    /pixel \((\d+),(\d+)\) differs: actual=Some\(\[(\d+), (\d+), (\d+), (\d+)\]\) expected=Some\(\[(\d+), (\d+), (\d+), (\d+)\]\)/u,
  );
  if (!match) throw new Error("native pixel diagnostic is malformed or incomplete");
  const values = match.slice(1).map(Number);
  try {
    return diagnosticPixel.parse({
      x: values[0],
      y: values[1],
      actual: values.slice(2, 6),
      expected: values.slice(6, 10),
    });
  } catch {
    throw new Error("native pixel diagnostic is outside the supported RGBA8 bounds");
  }
}

function normalizeFixturePath(value: string, crateRoot: string): string {
  if (!value || value.includes("\\") || value.includes("\0")) {
    throw new Error("native fixture path is invalid");
  }
  const fixtureRoot = path.resolve(path.resolve(crateRoot, "../.."), FIXTURE_RELATIVE);
  const resolved = path.isAbsolute(value) ? path.resolve(value) : path.resolve(crateRoot, value);
  const relative = path.relative(fixtureRoot, resolved);
  if (relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative) || relative === "..") {
    throw new Error("native fixture path escapes the upstream fixture root");
  }
  return relative.split(path.sep).join("/");
}

export function parseCaptureManifest(
  value: unknown,
  inventory: MarkerInventory,
  inventorySha256: string,
): CaptureManifest {
  let manifest: CaptureManifest;
  try {
    manifest = captureManifestSchema.parse(value);
  } catch {
    throw new Error("capture manifest schema is invalid");
  }
  if (manifest.inventorySha256 !== inventorySha256) {
    throw new Error("capture manifest does not match the fixed inventory");
  }
  if (manifest.captures.length !== inventory.cases.length) {
    throw new Error("capture set is incomplete");
  }
  manifest.captures.forEach((entry, index) => {
    const expected = inventory.cases[index];
    if (
      !expected ||
      entry.input !== expected.input ||
      entry.inputSha256 !== expected.inputSha256 ||
      entry.upstreamPngSha256 !== expected.upstreamPngSha256 ||
      entry.requestedWidth !== expected.requestedWidth ||
      entry.requestedHeight !== expected.requestedHeight ||
      entry.png.path !== `${String(index).padStart(2, "0")}.png`
    ) {
      throw new Error("capture entry differs from its inventory pin");
    }
  });
  return manifest;
}

function digest(bytes: Uint8Array): string {
  return sha256(bytes);
}

function category(input: string): "context" | "marker" | "overflow" {
  const value = input.split("/")[2];
  if (value === "context" || value === "marker" || value === "overflow") return value;
  throw new Error("marker fixture category is invalid");
}

function inventoryCategories(inventory: MarkerInventory): Record<string, number> {
  const counts = { context: 0, marker: 0, overflow: 0 };
  for (const entry of inventory.cases) counts[category(entry.input)] += 1;
  if (counts.context !== 3 || counts.marker !== 44 || counts.overflow !== 1) {
    throw new Error("marker inventory category counts differ");
  }
  return counts;
}

function resolvePinnedCapturePath(captureRoot: string, capturePath: string): string {
  if (!/^\d{2}\.png$/u.test(capturePath)) throw new Error("capture PNG path is invalid");
  const file = path.join(captureRoot, capturePath);
  try {
    if (fs.realpathSync(file) !== file || !fs.statSync(file).isFile()) {
      throw new Error("capture PNG is not canonical");
    }
    return file;
  } catch {
    throw new Error("capture PNG is missing or does not match its pin");
  }
}

export interface SvgMarkerBrowserDiagnosticOptions {
  repoRoot: string;
  nativeStderrPath: string;
  captureManifestPath: string;
  nativeFailureRoot?: string;
}

interface DiagnosticSample extends NativePixel {
  browser: Rgba;
  actualMatchesBrowser: boolean;
  expectedMatchesBrowser: boolean;
  nativeFailureArtifact?: Rgba;
  actualMatchesNativeFailureArtifact?: boolean;
}

interface DiagnosticCase {
  input: string;
  requestedWidth: number;
  requestedHeight: number;
  category: "context" | "marker" | "overflow";
  inputSha256: string;
  upstreamPngSha256: string;
  browserPngSha256: string;
  nativeFailurePngObservedSha256?: string;
  nativeFailureMetadataDifferentPixels?: number;
  nativeFailureVsReferenceDifferingPixels?: number;
  nativeFailureVsBrowserDifferingPixels?: number;
  nativeFailureMatchedLoggedSamples?: number;
  nativeFailureMismatchedLoggedSamples?: number;
  nativeLoggedSampleCount: number;
  nativeBrowserMatchedSamples: number;
  nativeBrowserMismatchedSamples: number;
  browserUpstreamDifferingPixels: number;
  samples: DiagnosticSample[];
}

interface DiagnosticInputs {
  fixtureRoot: string;
  inventory: MarkerInventory;
  categoryCounts: Record<string, number>;
  manifest: CaptureManifest;
  nativeByInput: Map<string, NativePixel[]>;
  nativeFailureArtifacts: Map<string, NativeFailureArtifact> | undefined;
  captureRoot: string;
}

interface CaseImages {
  upstream: Uint8Array;
  browser: Uint8Array;
}

interface NativeFailureComparison {
  vsReference: number;
  vsBrowser: number;
}

interface DiagnosticTotals {
  nativeFailureVsReferencePixels: number;
  nativeFailureVsBrowserPixels: number;
  nativeFailureMatchedLoggedSamples: number;
  nativeFailureMismatchedLoggedSamples: number;
}

function readMarkerInventory(repoRoot: string): { bytes: Uint8Array; value: MarkerInventory } {
  const bytes = fs.readFileSync(path.join(repoRoot, INVENTORY_RELATIVE));
  try {
    return {
      bytes,
      value: parseMarkerInventory(JSON.parse(bytes.toString("utf8")) as unknown),
    };
  } catch {
    throw new Error("marker inventory is invalid");
  }
}

function verifyInventoryFixturePins(fixtureRoot: string, inventory: MarkerInventory): void {
  for (const entry of inventory.cases) {
    const input = fixturePath(fixtureRoot, entry.input);
    const reference = fixturePath(fixtureRoot, entry.input.replace(/\.svg$/u, ".png"));
    if (
      digest(fs.readFileSync(input)) !== entry.inputSha256 ||
      digest(fs.readFileSync(reference)) !== entry.upstreamPngSha256
    ) {
      throw new Error("upstream fixture differs from its inventory pin");
    }
  }
}

function readCanonicalDiagnosticText(file: string): string {
  if (fs.realpathSync(file) !== file || !fs.statSync(file).isFile()) {
    throw new Error("diagnostic inputs must be canonical regular files");
  }
  return fs.readFileSync(file, "utf8");
}

function readCaptureManifest(
  file: string,
  inventory: MarkerInventory,
  inventorySha256: string,
): CaptureManifest {
  const text = readCanonicalDiagnosticText(file);
  try {
    return parseCaptureManifest(JSON.parse(text) as unknown, inventory, inventorySha256);
  } catch {
    throw new Error("capture manifest does not match the marker inventory");
  }
}

function loadDiagnosticInputs(options: SvgMarkerBrowserDiagnosticOptions): DiagnosticInputs {
  const repoRoot = fs.realpathSync(options.repoRoot);
  const fixtureRoot = fs.realpathSync(path.join(repoRoot, FIXTURE_RELATIVE));
  const { bytes, value: inventory } = readMarkerInventory(repoRoot);
  const inventorySha256 = digest(bytes);
  const captureManifestPath = path.resolve(repoRoot, options.captureManifestPath);
  const nativeStderrPath = path.resolve(repoRoot, options.nativeStderrPath);
  const manifest = readCaptureManifest(captureManifestPath, inventory, inventorySha256);
  const nativeText = readCanonicalDiagnosticText(nativeStderrPath);
  verifyInventoryFixturePins(fixtureRoot, inventory);
  const nativeCases = parseNativeDiagnostics(
    nativeText,
    path.join(repoRoot, "crates/katana-render-runtime"),
    inventory.cases,
  );
  const nativeFailureArtifacts =
    options.nativeFailureRoot === undefined
      ? undefined
      : readNativeFailureArtifacts(
          path.resolve(repoRoot, options.nativeFailureRoot),
          repoRoot,
          inventory,
        );
  return {
    fixtureRoot,
    inventory,
    categoryCounts: inventoryCategories(inventory),
    manifest,
    nativeByInput: new Map(nativeCases.map((entry) => [entry.input, entry.samples])),
    nativeFailureArtifacts,
    captureRoot: fs.realpathSync(path.dirname(captureManifestPath)),
  };
}

function readCaseImages(
  entry: MarkerInput,
  capture: CaptureManifest["captures"][number],
  inputs: DiagnosticInputs,
): CaseImages {
  const upstreamPath = fixturePath(inputs.fixtureRoot, entry.input.replace(/\.svg$/u, ".png"));
  return {
    upstream: readStableRgbaPng(
      upstreamPath,
      entry.requestedWidth,
      entry.requestedHeight,
      entry.upstreamPngSha256,
    ).pixels,
    browser: readStableRgbaPng(
      resolvePinnedCapturePath(inputs.captureRoot, capture.png.path),
      entry.requestedWidth,
      entry.requestedHeight,
      capture.png.sha256,
    ).pixels,
  };
}

function compareNativeFailureImages(
  artifact: NativeFailureArtifact | undefined,
  images: CaseImages,
  width: number,
  height: number,
): NativeFailureComparison | undefined {
  if (!artifact) return undefined;
  const vsReference = countRgbaDifferences(artifact.pixels, images.upstream, width, height);
  if (vsReference !== artifact.differentPixels) {
    throw new Error("native failure PNG difference count differs from artifact metadata");
  }
  return {
    vsReference,
    vsBrowser: countRgbaDifferences(artifact.pixels, images.browser, width, height),
  };
}

function compareLoggedSample(
  sample: NativePixel,
  images: CaseImages,
  width: number,
  nativeFailure: NativeFailureArtifact | undefined,
): DiagnosticSample {
  const browser = rgbaAt(images.browser, sample.x, sample.y, width);
  const upstream = rgbaAt(images.upstream, sample.x, sample.y, width);
  if (pixelsDiffer(sample.expected, upstream)) {
    throw new Error("native expected pixel differs from the pinned upstream PNG");
  }
  const base = {
    ...sample,
    browser,
    actualMatchesBrowser: !pixelsDiffer(sample.actual, browser),
    expectedMatchesBrowser: !pixelsDiffer(sample.expected, browser),
  };
  if (!nativeFailure) return base;
  const nativePixel = rgbaAt(nativeFailure.pixels, sample.x, sample.y, width);
  return {
    ...base,
    nativeFailureArtifact: nativePixel,
    actualMatchesNativeFailureArtifact: !pixelsDiffer(sample.actual, nativePixel),
  };
}

function buildDiagnosticCase(
  index: number,
  entry: MarkerInput,
  inputs: DiagnosticInputs,
): DiagnosticCase {
  const capture = inputs.manifest.captures[index];
  const nativeSamples = inputs.nativeByInput.get(entry.input);
  if (!capture || !nativeSamples) throw new Error("marker evidence set is incomplete");
  const nativeFailure = inputs.nativeFailureArtifacts?.get(entry.input);
  if (inputs.nativeFailureArtifacts && !nativeFailure) {
    throw new Error("native failure artifact case is missing");
  }
  const { requestedWidth: width, requestedHeight: height } = entry;
  const images = readCaseImages(entry, capture, inputs);
  const nativeFailureComparison = compareNativeFailureImages(nativeFailure, images, width, height);
  const samples = nativeSamples.map((sample) =>
    compareLoggedSample(sample, images, width, nativeFailure),
  );
  const browserUpstreamDifferingPixels = countRgbaDifferences(
    images.browser,
    images.upstream,
    width,
    height,
  );
  const nativeBrowserMatchedSamples = samples.filter(
    (sample) => sample.actualMatchesBrowser,
  ).length;
  const nativeFailureMatchedSamples = samples.filter(
    (sample) => sample.actualMatchesNativeFailureArtifact,
  ).length;
  return {
    input: entry.input,
    category: category(entry.input),
    requestedWidth: width,
    requestedHeight: height,
    inputSha256: entry.inputSha256,
    upstreamPngSha256: entry.upstreamPngSha256,
    browserPngSha256: capture.png.sha256,
    ...(nativeFailure && nativeFailureComparison
      ? {
          nativeFailurePngObservedSha256: nativeFailure.pngSha256,
          nativeFailureMetadataDifferentPixels: nativeFailure.differentPixels,
          nativeFailureVsReferenceDifferingPixels: nativeFailureComparison.vsReference,
          nativeFailureVsBrowserDifferingPixels: nativeFailureComparison.vsBrowser,
          nativeFailureMatchedLoggedSamples: nativeFailureMatchedSamples,
          nativeFailureMismatchedLoggedSamples: samples.length - nativeFailureMatchedSamples,
        }
      : {}),
    nativeLoggedSampleCount: samples.length,
    nativeBrowserMatchedSamples,
    nativeBrowserMismatchedSamples: samples.length - nativeBrowserMatchedSamples,
    browserUpstreamDifferingPixels,
    samples,
  };
}

function createCategorySummary(): Record<
  "context" | "marker" | "overflow",
  {
    cases: number;
    browserUpstreamDifferingPixels: number;
    nativeSamples: number;
    nativeBrowserMatches: number;
  }
> {
  const row = () => ({
    cases: 0,
    browserUpstreamDifferingPixels: 0,
    nativeSamples: 0,
    nativeBrowserMatches: 0,
  });
  return { context: row(), marker: row(), overflow: row() };
}

function addCaseToSummary(
  summary: ReturnType<typeof createCategorySummary>,
  entry: DiagnosticCase,
): void {
  const row = summary[entry.category];
  row.cases += 1;
  row.browserUpstreamDifferingPixels += entry.browserUpstreamDifferingPixels;
  row.nativeSamples += entry.nativeLoggedSampleCount;
  row.nativeBrowserMatches += entry.nativeBrowserMatchedSamples;
}

function addNativeFailureTotals(totals: DiagnosticTotals, entry: DiagnosticCase): void {
  totals.nativeFailureVsReferencePixels += entry.nativeFailureVsReferenceDifferingPixels ?? 0;
  totals.nativeFailureVsBrowserPixels += entry.nativeFailureVsBrowserDifferingPixels ?? 0;
  totals.nativeFailureMatchedLoggedSamples += entry.nativeFailureMatchedLoggedSamples ?? 0;
  totals.nativeFailureMismatchedLoggedSamples += entry.nativeFailureMismatchedLoggedSamples ?? 0;
}

function createDiagnosticReport(
  inputs: DiagnosticInputs,
  cases: DiagnosticCase[],
  categorySummary: ReturnType<typeof createCategorySummary>,
  totals: DiagnosticTotals,
): Record<string, unknown> {
  const nativeLoggedSampleCount = cases.reduce(
    (count, entry) => count + entry.nativeLoggedSampleCount,
    0,
  );
  const nativeBrowserMatchedSamples = cases.reduce(
    (count, entry) => count + entry.nativeBrowserMatchedSamples,
    0,
  );
  const browserUpstreamDifferingPixels = cases.reduce(
    (count, entry) => count + entry.browserUpstreamDifferingPixels,
    0,
  );
  return {
    schema: "krr-svg-marker-browser-diagnostic-v1",
    diagnosticOnly: true,
    fullNativePngComparison: inputs.nativeFailureArtifacts !== undefined,
    fullBrowserPngComparison: true,
    coverage: "logged-first-eight-differing-pixels-only",
    requestedWidth: WIDTH,
    caseCount: cases.length,
    categoryCounts: inputs.categoryCounts,
    nativeLoggedSamples: {
      compared: nativeLoggedSampleCount,
      matchingBrowser: nativeBrowserMatchedSamples,
      differingFromBrowser: nativeLoggedSampleCount - nativeBrowserMatchedSamples,
      casesWithAllSamplesMatchingBrowser: cases.filter(
        (entry) => entry.nativeBrowserMatchedSamples === entry.nativeLoggedSampleCount,
      ).length,
    },
    browserComparedWithUpstream: {
      scope: "full-image-rgba8",
      differingPixels: browserUpstreamDifferingPixels,
    },
    ...(inputs.nativeFailureArtifacts
      ? {
          nativeFailurePngComparison: {
            scope: "full-image-rgba8",
            caseCount: inputs.nativeFailureArtifacts.size,
            comparedWithUpstreamDifferingPixels: totals.nativeFailureVsReferencePixels,
            comparedWithBrowserDifferingPixels: totals.nativeFailureVsBrowserPixels,
            loggedSamplesCompared:
              totals.nativeFailureMatchedLoggedSamples +
              totals.nativeFailureMismatchedLoggedSamples,
            loggedSamplesMatching: totals.nativeFailureMatchedLoggedSamples,
            loggedSamplesDiffering: totals.nativeFailureMismatchedLoggedSamples,
            loggedStderrSamplesRemainLimited: true,
          },
        }
      : {}),
    categorySummary,
    cases,
  };
}

export function createSvgMarkerBrowserDiagnostic(
  options: SvgMarkerBrowserDiagnosticOptions,
): Record<string, unknown> {
  const inputs = loadDiagnosticInputs(options);
  const cases: DiagnosticCase[] = [];
  const categorySummary = createCategorySummary();
  const totals: DiagnosticTotals = {
    nativeFailureVsReferencePixels: 0,
    nativeFailureVsBrowserPixels: 0,
    nativeFailureMatchedLoggedSamples: 0,
    nativeFailureMismatchedLoggedSamples: 0,
  };
  for (const [index, entry] of inputs.inventory.cases.entries()) {
    const result = buildDiagnosticCase(index, entry, inputs);
    cases.push(result);
    addCaseToSummary(categorySummary, result);
    addNativeFailureTotals(totals, result);
  }
  return createDiagnosticReport(inputs, cases, categorySummary, totals);
}

export function writeDiagnosticExclusive(
  repoRoot: string,
  outputPath: string,
  report: unknown,
): void {
  const tmpRoot = fs.realpathSync(path.join(repoRoot, "tmp"));
  const resolved = path.resolve(repoRoot, outputPath);
  const parent = fs.realpathSync(path.dirname(resolved));
  const relativeParent = path.relative(tmpRoot, parent);
  if (
    !resolved.startsWith(`${tmpRoot}${path.sep}`) ||
    relativeParent.startsWith(`..${path.sep}`) ||
    relativeParent === ".." ||
    path.isAbsolute(relativeParent)
  ) {
    throw new Error("diagnostic output must be under tmp");
  }
  try {
    fs.writeFileSync(resolved, `${JSON.stringify(report, null, 2)}\n`, { flag: "wx" });
  } catch {
    throw new Error("diagnostic output already exists or cannot be created");
  }
}

if (import.meta.main) {
  try {
    const args = parseArgs({
      args: process.argv.slice(2),
      options: {
        repo: { type: "string", default: process.cwd() },
        "native-stderr": { type: "string" },
        "capture-manifest": { type: "string" },
        "native-failure-root": { type: "string" },
        output: { type: "string" },
      },
      strict: true,
    });
    const {
      repo,
      "native-stderr": nativeStderrPath,
      "capture-manifest": captureManifestPath,
      "native-failure-root": nativeFailureRoot,
      output,
    } = args.values;
    if (!nativeStderrPath || !captureManifestPath || !output) {
      throw new Error("missing arguments");
    }
    const report = createSvgMarkerBrowserDiagnostic({
      repoRoot: repo,
      nativeStderrPath,
      captureManifestPath,
      ...(nativeFailureRoot === undefined ? {} : { nativeFailureRoot }),
    });
    writeDiagnosticExclusive(fs.realpathSync(repo), output, report);
  } catch {
    console.error("SVG marker browser diagnostic failed");
    process.exitCode = 1;
  }
}
