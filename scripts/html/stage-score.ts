import fs from "node:fs";
import path from "node:path";
import { assertExpectedStateMatches, type JsonValue } from "./dynamic-state";
import { assertCatalogComplete, type FeatureCatalog, loadFeatureCatalog } from "./feature-catalog";
import { compareImages, sha256File } from "./reference-capture";

export const STAGE90_THRESHOLD = 90;
const REQUIRED_PHASES = ["cold", "warm", "re-display", "full-Y", "post-operation"] as const;
export type StageImageKind = "viewport" | "scroll" | "toc" | "state";
export interface StageViewport {
  width: number;
  height: number;
  scale: number;
}
export interface TrustedImageExpectation {
  key: string;
  caseId: string;
  viewport: StageViewport;
  state: string;
  kind: StageImageKind;
  actualScrollY: number;
  maxScrollY: number;
  viewportHeight: number;
  sourceSha256: string;
  referenceSha256: string;
  expectedState?: JsonValue;
}
export interface PerformancePhase {
  name: string;
  observedMs: number;
  baselineMs: number;
  observedMemoryBytes: number;
  baselineMemoryBytes: number;
  measurementPairs: number;
}
export interface TrustedPerformance {
  environment: string;
  conditionId: string;
  phases: PerformancePhase[];
}
export interface TrustedStage90Expectations {
  profile: "stage90";
  threshold: 90;
  sourceSha256: string;
  binarySha256: string;
  images: TrustedImageExpectation[];
  tocKeys: string[];
  performance: TrustedPerformance;
}
export interface StageImageResult extends TrustedImageExpectation {
  rmseScore: number;
  maeScore: number;
  binarySha256: string;
  targetVisible: boolean;
  operationState: "matched";
  evidenceId: string;
  observedState?: JsonValue;
}
export interface Stage90Manifest {
  profile: "stage90";
  threshold: 90;
  sourceSha256: string;
  binarySha256: string;
  images: StageImageResult[];
  performance: { environment: string; conditionId: string; phases: PerformancePhase[] };
}

/** Strict evidence-bound stage gate. Unlike the diagnostic loader, this recomputes PNG scores. */
export function assertStrictStage90Manifest(value: JsonValue, trustedValue: JsonValue): void {
  assertStage90Manifest(value, trustedValue);
  const manifest = strictRecord(value, "stage manifest");
  const trusted = strictRecord(trustedValue, "trusted stage expectation");
  if (manifest.profile !== "stage90" || manifest.threshold !== STAGE90_THRESHOLD)
    throw new Error("strict stage profile or threshold differs");
  if (
    manifest.sourceSha256 !== trusted.sourceSha256 ||
    manifest.binarySha256 !== trusted.binarySha256
  )
    throw new Error("strict stage provenance differs");
  const actualImages = strictArray(manifest.images, "stage images");
  const trustedImages = strictArray(trusted.images, "trusted images");
  const expected = new Map(trustedImages.map((image) => [strictText(image.key, "key"), image]));
  if (expected.size !== trustedImages.length || actualImages.length !== expected.size)
    throw new Error("strict stage image coverage is incomplete or duplicated");
  const seen = new Set<string>();
  for (const image of actualImages) {
    const key = strictText(image.key, "image key");
    if (seen.has(key)) throw new Error(`duplicate strict stage image: ${key}`);
    seen.add(key);
    const expectation = expected.get(key);
    if (!expectation) throw new Error(`unexpected strict stage image: ${key}`);
    assertStrictImage(image, expectation, strictText(trusted.binarySha256, "binary SHA"));
  }
  if (seen.size !== expected.size) throw new Error("missing strict stage image");
}

function assertStrictImage(
  image: { [key: string]: JsonValue },
  expectation: { [key: string]: JsonValue },
  binarySha256: string,
): void {
  const referencePath = strictPath(expectation.referencePath, "reference PNG");
  const candidatePath = strictPath(image.candidatePath, "candidate PNG");
  const evidencePath = strictPath(image.operationEvidencePath, "operation evidence");
  const candidateSha = assertStrictPng(
    image,
    expectation,
    referencePath,
    candidatePath,
    binarySha256,
  );
  assertStrictEvidence(image, expectation, evidencePath, candidateSha, binarySha256);
}

function assertStrictPng(
  image: { [key: string]: JsonValue },
  expectation: { [key: string]: JsonValue },
  referencePath: string,
  candidatePath: string,
  binarySha256: string,
): string {
  const referenceSha = sha256File(referencePath);
  const candidateSha = sha256File(candidatePath);
  if (referenceSha !== strictText(expectation.referenceSha256, "reference SHA"))
    throw new Error(`reference PNG changed: ${strictText(image.key, "image key")}`);
  if (candidateSha !== strictText(image.candidateSha256, "candidate SHA"))
    throw new Error(`candidate PNG SHA differs: ${strictText(image.key, "image key")}`);
  if (candidateSha !== strictText(expectation.candidateSha256, "expected candidate SHA"))
    throw new Error(
      `candidate PNG is not the trusted image: ${strictText(image.key, "image key")}`,
    );
  if (
    strictText(image.sourceSha256, "image source SHA") !==
      strictText(expectation.sourceSha256, "expected source SHA") ||
    strictText(image.binarySha256, "image binary SHA") !== binarySha256
  )
    throw new Error(`strict image provenance differs: ${strictText(image.key, "image key")}`);
  assertStrictImageDimensions(image, expectation, referencePath, candidatePath);
  const score = compareImages(referencePath, candidatePath, STAGE90_THRESHOLD);
  if (score.rmseScore < STAGE90_THRESHOLD || score.maeScore < STAGE90_THRESHOLD)
    throw new Error(`strict stage score below 90: ${strictText(image.key, "image key")}`);
  assertReportedScore(image, score.rmseScore, score.maeScore);
  return candidateSha;
}

function assertStrictImageDimensions(
  image: { [key: string]: JsonValue },
  expectation: { [key: string]: JsonValue },
  referencePath: string,
  candidatePath: string,
): void {
  const expectedViewport = strictViewport(expectation.viewport, "trusted viewport");
  const actualViewport = strictViewport(image.viewport, "image viewport");
  if (
    actualViewport.width !== expectedViewport.width ||
    actualViewport.height !== expectedViewport.height ||
    actualViewport.scale !== expectedViewport.scale
  )
    throw new Error(`image viewport differs: ${strictText(image.key, "image key")}`);
  const expectedWidth = expectedViewport.width * expectedViewport.scale;
  const expectedHeight = expectedViewport.height * expectedViewport.scale;
  if (!Number.isInteger(expectedWidth) || !Number.isInteger(expectedHeight))
    throw new Error(
      `viewport pixel dimensions are not integral: ${strictText(image.key, "image key")}`,
    );
  for (const [label, pathname] of [
    ["reference", referencePath],
    ["candidate", candidatePath],
  ] as const) {
    const dimensions = pngDimensions(pathname);
    if (dimensions.width !== expectedWidth || dimensions.height !== expectedHeight)
      throw new Error(
        `${label} PNG dimensions differ from viewport: ${strictText(image.key, "image key")}`,
      );
  }
}

function strictViewport(value: JsonValue | undefined, name: string): StageViewport {
  if (!record(value)) throw new Error(`invalid ${name}`);
  return {
    width: positiveNumber(value.width, `${name} width`),
    height: positiveNumber(value.height, `${name} height`),
    scale: positiveNumber(value.scale, `${name} scale`),
  };
}

function pngDimensions(pathname: string): { width: number; height: number } {
  const result = Bun.spawnSync(["magick", "identify", "-format", "%w %h", pathname]);
  if (result.exitCode !== 0) throw new Error(`PNG dimensions unavailable: ${pathname}`);
  const values = new TextDecoder().decode(result.stdout).trim().split(/\s+/).map(Number);
  const width = values.at(0);
  const height = values.at(1);
  if (
    width === undefined ||
    height === undefined ||
    !Number.isInteger(width) ||
    !Number.isInteger(height)
  )
    throw new Error(`invalid PNG dimensions: ${pathname}`);
  return { width, height };
}

function assertStrictEvidence(
  image: { [key: string]: JsonValue },
  expectation: { [key: string]: JsonValue },
  evidencePath: string,
  candidateSha: string,
  binarySha256: string,
): void {
  const evidence = strictRecord(
    JSON.parse(fs.readFileSync(evidencePath, "utf8")),
    "operation evidence",
  );
  if (
    sha256File(evidencePath) !== strictText(image.operationEvidenceSha256, "operation evidence SHA")
  )
    throw new Error(`operation evidence changed: ${strictText(image.key, "image key")}`);
  if (
    evidence.status !== "matched" ||
    evidence.sourceSha256 !== expectation.sourceSha256 ||
    evidence.binarySha256 !== binarySha256 ||
    evidence.candidateSha256 !== candidateSha
  )
    throw new Error(`operation evidence provenance differs: ${strictText(image.key, "image key")}`);
  assertEvidenceTuple(evidence, expectation, strictText(image.key, "image key"));
  assertEvidenceIdentity(evidence, expectation, strictText(image.key, "image key"));
  if (expectation.kind === "state") {
    if (expectation.expectedState === undefined || evidence.observedState === undefined)
      throw new Error(
        `operation evidence state is incomplete: ${strictText(image.key, "image key")}`,
      );
    assertExpectedStateMatches(expectation.expectedState, evidence.observedState);
  }
  if (
    evidence.kind === "scroll" &&
    finite(evidence.requestedY, "requested scroll") !==
      finite(expectation.requestedY, "expected requested scroll")
  )
    throw new Error(
      `operation evidence requested scroll differs: ${strictText(image.key, "image key")}`,
    );
  if (evidence.kind === "toc") {
    if (
      strictText(evidence.destination, "TOC destination") !==
        strictText(expectation.destination, "expected TOC destination") ||
      strictText(evidence.hash, "TOC hash") !== strictText(expectation.hash, "expected TOC hash")
    )
      throw new Error(
        `operation evidence destination differs: ${strictText(image.key, "image key")}`,
      );
  }
}

function assertEvidenceTuple(
  evidence: { [key: string]: JsonValue },
  expectation: { [key: string]: JsonValue },
  key: string,
): void {
  for (const field of [
    "caseId",
    "state",
    "actualScrollY",
    "maxScrollY",
    "targetVisible",
    "nonce",
  ]) {
    if (
      evidence[field] === undefined ||
      expectation[field] === undefined ||
      evidence[field] !== expectation[field]
    )
      throw new Error(`operation evidence tuple differs: ${key}`);
  }
}

function assertEvidenceIdentity(
  evidence: { [key: string]: JsonValue },
  expectation: { [key: string]: JsonValue },
  key: string,
): void {
  if (
    evidence.key !== expectation.key ||
    evidence.kind !== expectation.kind ||
    !sameViewport(evidence.viewport, expectation.viewport) ||
    strictText(evidence.observedHash, "observed hash") !==
      strictText(expectation.observedHash, "expected observed hash")
  )
    throw new Error(`operation evidence identity differs: ${key}`);
}

function sameViewport(left: JsonValue | undefined, right: JsonValue | undefined): boolean {
  if (!record(left) || !record(right)) return false;
  return left.width === right.width && left.height === right.height && left.scale === right.scale;
}

function assertReportedScore(image: { [key: string]: JsonValue }, rmse: number, mae: number): void {
  const reportedRmse = finite(image.rmseScore, "reported RMSE score");
  const reportedMae = finite(image.maeScore, "reported MAE score");
  if (Math.abs(reportedRmse - rmse) > 1e-9 || Math.abs(reportedMae - mae) > 1e-9)
    throw new Error(
      `reported score differs from PNG recomputation: ${strictText(image.key, "image key")}`,
    );
}

function strictPath(value: JsonValue | undefined, name: string): string {
  const file = path.resolve(strictText(value, name));
  if (!fs.existsSync(file) || fs.lstatSync(file).isSymbolicLink() || hasSymlinkAncestor(file))
    throw new Error(`${name} is missing or symlinked: ${file}`);
  return file;
}

function hasSymlinkAncestor(file: string): boolean {
  let current = path.dirname(file);
  while (current !== path.dirname(current)) {
    if (fs.existsSync(current) && fs.lstatSync(current).isSymbolicLink()) return true;
    current = path.dirname(current);
  }
  return false;
}

function strictRecord(value: JsonValue | undefined, name: string): { [key: string]: JsonValue } {
  if (!record(value)) throw new Error(`invalid ${name}`);
  return value;
}

function strictArray(
  value: JsonValue | undefined,
  name: string,
): Array<{ [key: string]: JsonValue }> {
  if (!Array.isArray(value) || value.some((item) => !record(item)))
    throw new Error(`invalid ${name}`);
  return value as Array<{ [key: string]: JsonValue }>;
}

function strictText(value: JsonValue | undefined, name: string): string {
  if (typeof value !== "string" || value.length === 0) throw new Error(`invalid ${name}`);
  return value;
}

export interface AnonymousDocumentExpectations {
  profile: "stage90-anonymous-document";
  sourceSha256: string;
  binarySha256: string;
  images: TrustedImageExpectation[];
  viewports: Array<{
    viewport: StageViewport;
    maxScrollY: number;
    scrollKeys: string[];
    tocKeys: string[];
  }>;
  performance?: TrustedPerformance;
}

export interface AnonymousFidelityFailure {
  kind: "inventory" | "provenance" | "reference" | "score" | "scroll";
  key: string;
  message: string;
}

export async function loadAnonymousDocumentExpectations(
  scrollManifestPaths: string[],
  binarySha256: string,
): Promise<AnonymousDocumentExpectations> {
  const required = new Set(["1280x900@1", "960x720@1", "620x900@1"]);
  if (scrollManifestPaths.length !== required.size)
    throw new Error("anonymous document requires three viewport manifests");
  const reports = scrollManifestPaths.map(readCanonicalManifest);
  const seen = new Set(reports.map((report) => viewportKey(report.viewport)));
  if (seen.size !== required.size || [...required].some((key) => !seen.has(key)))
    throw new Error("anonymous document viewport set is incomplete");
  const sourceSha256 = canonicalSourceSha(reports, []);
  const tocTargets = await extractSidebarTargets(reports[0]?.htmlPath ?? "");
  const firstEntries = reports[0]?.entries ?? [];
  if (firstEntries.length !== tocTargets.size)
    throw new Error(
      `anonymous document TOC coverage is incomplete: ${firstEntries.length}/${tocTargets.size}`,
    );
  const tocKeys = new Set(firstEntries.map((entry) => `${entry.destination}:${entry.hash}`));
  if (tocKeys.size !== tocTargets.size || [...tocTargets].some((target) => !tocKeys.has(target)))
    throw new Error("anonymous document TOC entries differ from DOM");
  const viewports = reports.map((report) => {
    const entries = report.entries ?? [];
    if (
      entries.length !== tocTargets.size ||
      new Set(entries.map((entry) => `${entry.destination}:${entry.hash}`)).size !== tocTargets.size
    )
      throw new Error("anonymous document TOC coverage differs");
    if (entries.some((entry) => !tocKeys.has(`${entry.destination}:${entry.hash}`)))
      throw new Error("anonymous document TOC destination differs");
    if (!report.scrolls || report.scrolls.length === 0)
      throw new Error("anonymous document scroll coverage is empty");
    assertCanonicalScrollPlan(report);
    return {
      viewport: report.viewport,
      maxScrollY: report.maxScrollY,
      scrollKeys: report.scrolls.map((scroll) => scroll.name),
      tocKeys: entries.map((entry) => entry.destination),
    };
  });
  return {
    profile: "stage90-anonymous-document",
    sourceSha256,
    binarySha256: sha(binarySha256, "binary SHA"),
    images: reports.flatMap((report) => anonymousImages(report, sourceSha256)),
    viewports,
  };
}

export function anonymousExpectationsAsTrusted(
  expectations: AnonymousDocumentExpectations,
): TrustedStage90Expectations {
  return {
    profile: "stage90",
    threshold: STAGE90_THRESHOLD,
    sourceSha256: expectations.sourceSha256,
    binarySha256: expectations.binarySha256,
    images: expectations.images,
    tocKeys: expectations.images.filter((image) => image.kind === "toc").map((image) => image.key),
    performance: expectations.performance ?? missingAnonymousPerformance(),
  };
}

export function assertAnonymousFidelityResults(
  resultsValue: JsonValue,
  expectations: AnonymousDocumentExpectations,
): void {
  const failures = collectAnonymousFidelityFailures(resultsValue, expectations);
  if (failures.length > 0)
    throw new Error(
      `anonymous fidelity rejected:\n${failures.map((failure) => failure.message).join("\n")}`,
    );
}

export function collectAnonymousFidelityFailures(
  resultsValue: JsonValue,
  expectations: AnonymousDocumentExpectations,
): AnonymousFidelityFailure[] {
  const failures: AnonymousFidelityFailure[] = [];
  if (!record(resultsValue) || !Array.isArray(resultsValue.results)) {
    return [{ kind: "inventory", key: "results", message: "invalid anonymous fidelity results" }];
  }
  if (resultsValue.htmlSha256 !== expectations.sourceSha256)
    failures.push({
      kind: "provenance",
      key: "htmlSha256",
      message: "anonymous source SHA differs",
    });
  if (resultsValue.binarySha256 !== expectations.binarySha256)
    failures.push({
      kind: "provenance",
      key: "binarySha256",
      message: "anonymous binary SHA differs",
    });

  const expected = expectations.images.filter((image) => image.kind === "scroll");
  const seen = new Set<string>();
  if (resultsValue.results.length !== expected.length)
    failures.push({
      kind: "inventory",
      key: "results",
      message: `anonymous fidelity scroll results incomplete: ${resultsValue.results.length}/${expected.length}`,
    });
  for (const result of resultsValue.results)
    failures.push(...collectAnonymousResultFailures(result, expected, seen));
  for (const image of expected)
    if (!seen.has(image.key))
      failures.push({
        kind: "inventory",
        key: image.key,
        message: `missing anonymous result: ${image.key}`,
      });
  return failures;
}

function collectAnonymousResultFailures(
  result: JsonValue,
  expected: TrustedImageExpectation[],
  seen: Set<string>,
): AnonymousFidelityFailure[] {
  if (!record(result) || !record(result.viewport) || !record(result.score))
    return [
      { kind: "inventory", key: "invalid-result", message: "invalid anonymous fidelity result" },
    ];
  const viewportValue = result.viewport;
  if (!record(viewportValue))
    return [
      { kind: "inventory", key: "invalid-viewport", message: "invalid anonymous result viewport" },
    ];
  const viewport = viewportValue;
  const actualY = typeof result.chromeScrollY === "number" ? result.chromeScrollY : Number.NaN;
  const requested = typeof result.requested === "number" ? result.requested : Number.NaN;
  const name =
    requested === 1e9
      ? "bottom"
      : Number.isFinite(requested)
        ? String(requested)
        : "invalid-request";
  const image = expected.find(
    (entry) =>
      entry.viewport.width === viewport.width &&
      entry.viewport.height === viewport.height &&
      entry.key.endsWith(`/scroll/${name}`),
  );
  const key = image?.key ?? `${viewport.width}x${viewport.height}/scroll/${name}`;
  if (!image || image.actualScrollY !== actualY)
    return [{ kind: "inventory", key, message: `anonymous result has no canonical image: ${key}` }];
  return collectAnonymousImageFailures(result, image, actualY, seen);
}

function missingAnonymousPerformance(): TrustedPerformance {
  throw new Error("anonymous expectations have no measured performance evidence");
}

function collectAnonymousImageFailures(
  result: { [key: string]: JsonValue },
  image: TrustedImageExpectation,
  actualY: number,
  seen: Set<string>,
): AnonymousFidelityFailure[] {
  const failures: AnonymousFidelityFailure[] = [];
  if (seen.has(image.key))
    failures.push({
      kind: "inventory",
      key: image.key,
      message: `duplicate anonymous result: ${image.key}`,
    });
  seen.add(image.key);
  if (result.referenceSha256 !== image.referenceSha256)
    failures.push({
      kind: "reference",
      key: image.key,
      message: `anonymous reference differs: ${image.key}`,
    });
  const score =
    record(result.score) && typeof result.score.score === "number"
      ? result.score.score
      : Number.NaN;
  if (!Number.isFinite(score) || score < STAGE90_THRESHOLD)
    failures.push({
      kind: "score",
      key: image.key,
      message: `anonymous fidelity score below 90: ${image.key} (${score})`,
    });
  if (result.krrScrollY !== actualY)
    failures.push({
      kind: "scroll",
      key: image.key,
      message: `anonymous scroll mismatch: ${image.key} (Chrome ${actualY}, KRR ${String(result.krrScrollY)})`,
    });
  return failures;
}

function anonymousImages(report: CanonicalReport, sourceSha256: string): TrustedImageExpectation[] {
  const viewportName = viewportKey(report.viewport);
  const prefix = `anonymous-document/${viewportName}/initial`;
  const scrollImages = (report.scrolls ?? []).map((scroll) => ({
    key: `${prefix}/scroll/${scroll.name}`,
    caseId: "anonymous-document",
    viewport: report.viewport,
    state: "initial",
    kind: "scroll" as const,
    actualScrollY: scroll.actual,
    maxScrollY: report.maxScrollY,
    viewportHeight: report.viewport.height,
    sourceSha256,
    referenceSha256: sha256File(resolveArtifact(scroll.capturePath)),
  }));
  const tocImages = (report.entries ?? []).map((entry) => ({
    key: `${prefix}/toc/${entry.destination}`,
    caseId: "anonymous-document",
    viewport: report.viewport,
    state: "initial",
    kind: "toc" as const,
    actualScrollY: entry.scrollY,
    maxScrollY: report.maxScrollY,
    viewportHeight: report.viewport.height,
    sourceSha256,
    referenceSha256: sha256File(resolveArtifact(entry.capturePath)),
  }));
  return [...scrollImages, ...tocImages];
}

export async function extractSidebarTargets(htmlPath: string): Promise<Set<string>> {
  const html = fs.readFileSync(path.resolve(htmlPath), "utf8");
  const targets = new Set<string>();
  await new HTMLRewriter()
    .on("nav.sidebar a.toc-item", {
      element(element) {
        const hash = element.getAttribute("href");
        if (hash?.startsWith("#") && hash.length > 1) targets.add(`${hash.slice(1)}:${hash}`);
      },
    })
    .transform(new Response(html))
    .text();
  if (targets.size === 0) throw new Error("anonymous document sidebar TOC is empty");
  return targets;
}

function assertCanonicalScrollPlan(report: CanonicalReport): void {
  const positions = (report.scrolls ?? [])
    .map((scroll) => scroll.actual)
    .sort((left, right) => left - right);
  if (
    positions[0] !== 0 ||
    positions.at(-1) !== report.maxScrollY ||
    report.scrolls?.at(-1)?.name !== "bottom"
  )
    throw new Error("anonymous document scroll plan lacks top or bottom");
  const gapLimit = report.viewport.height * 0.8 + 0.01;
  for (let index = 1; index < positions.length; index += 1)
    if ((positions[index] ?? 0) - (positions[index - 1] ?? 0) > gapLimit)
      throw new Error("anonymous document scroll coverage gap");
}

export function loadCanonicalTrustedExpectations(
  catalogPath: string,
  scrollManifestPaths: string[],
  tocManifestPaths: string[],
  binarySha256: string,
  performanceReportPath: string,
): TrustedStage90Expectations {
  const catalog = loadFeatureCatalog(catalogPath);
  assertCatalogComplete(catalog);
  if (scrollManifestPaths.length === 0) throw new Error("canonical scroll manifests are required");
  const scrollReports = scrollManifestPaths.map((file) => readCanonicalManifest(file));
  const tocReports = tocManifestPaths.map((file) => readCanonicalManifest(file));
  const sourceSha256 = canonicalSourceSha(scrollReports, tocReports);
  const images = [
    ...scrollReports.flatMap((report) => canonicalScrollImages(report, catalog)),
    ...tocReports.flatMap((report) => canonicalTocImages(report, catalog)),
  ];
  if (images.length === 0) throw new Error("canonical manifests contain no images");
  const performance = readCanonicalPerformance(performanceReportPath);
  return {
    profile: "stage90",
    threshold: STAGE90_THRESHOLD,
    sourceSha256,
    binarySha256: sha(binarySha256, "binary SHA"),
    images,
    tocKeys: images.filter((image) => image.kind === "toc").map((image) => image.key),
    performance,
  };
}

interface CanonicalReport {
  htmlPath: string;
  htmlSha256: string;
  viewport: { width: number; height: number; scale: number };
  maxScrollY: number;
  scrollHeight?: number;
  scrolls?: Array<{ name: string; actual: number; capturePath: string }>;
  entries?: Array<{
    destination: string;
    hash: string;
    capturePath: string;
    targetVisible: boolean;
    scrollY: number;
  }>;
}

function readCanonicalManifest(file: string): CanonicalReport {
  const value = JSON.parse(fs.readFileSync(path.resolve(file), "utf8")) as JsonValue;
  if (
    !record(value) ||
    typeof value.htmlPath !== "string" ||
    typeof value.htmlSha256 !== "string" ||
    !record(value.viewport)
  )
    throw new Error(`invalid canonical manifest: ${file}`);
  const viewport = value.viewport;
  if (
    typeof viewport.width !== "number" ||
    typeof viewport.height !== "number" ||
    typeof viewport.scale !== "number"
  )
    throw new Error(`invalid canonical viewport: ${file}`);
  return {
    htmlPath: value.htmlPath,
    htmlSha256: sha(value.htmlSha256, "manifest source SHA"),
    viewport: { width: viewport.width, height: viewport.height, scale: viewport.scale },
    maxScrollY: finite(value.maxScrollY, "manifest maxScrollY"),
    ...(Array.isArray(value.scrolls) ? { scrolls: value.scrolls.map(parseCanonicalScroll) } : {}),
    ...(Array.isArray(value.entries) ? { entries: value.entries.map(parseCanonicalEntry) } : {}),
  };
}

function parseCanonicalScroll(value: JsonValue): {
  name: string;
  actual: number;
  capturePath: string;
} {
  if (!record(value)) throw new Error("invalid canonical scroll");
  return {
    name: text(value.name, "scroll name"),
    actual: finite(value.actual, "actual scroll"),
    capturePath: text(value.capturePath, "scroll capture"),
  };
}

function parseCanonicalEntry(value: JsonValue): {
  destination: string;
  hash: string;
  capturePath: string;
  targetVisible: boolean;
  scrollY: number;
} {
  if (!record(value) || value.targetVisible !== true)
    throw new Error("invalid canonical TOC entry");
  const destination = text(value.destination, "TOC destination");
  const hash = text(value.hash, "TOC hash");
  if (hash !== `#${destination}`) throw new Error("canonical TOC hash differs");
  return {
    destination,
    hash,
    capturePath: text(value.capturePath, "TOC capture"),
    targetVisible: true,
    scrollY: finite(value.scrollY, "TOC scroll"),
  };
}

function canonicalSourceSha(reports: CanonicalReport[], tocReports: CanonicalReport[]): string {
  const all = [...reports, ...tocReports];
  const source = all[0]?.htmlSha256;
  if (!source || all.some((report) => report.htmlSha256 !== source))
    throw new Error("canonical source SHA differs");
  if (all.some((report) => sha256File(report.htmlPath) !== source))
    throw new Error("canonical source file changed");
  return source;
}

function canonicalCaseId(catalog: FeatureCatalog, htmlPath: string): string {
  const fixture = path.resolve(htmlPath);
  const entry = [...catalog.visualAspects, ...catalog.scriptCases].find(
    (item) => path.resolve("tests/fixtures/html", item.fixture) === fixture,
  );
  if (!entry) throw new Error(`canonical manifest fixture is absent from catalog: ${htmlPath}`);
  return entry.id;
}

function canonicalScrollImages(
  report: CanonicalReport,
  catalog: FeatureCatalog,
): TrustedImageExpectation[] {
  if (!report.scrolls || report.scrolls.length === 0)
    throw new Error("canonical scroll manifest has no scrolls");
  const caseId = canonicalCaseId(catalog, report.htmlPath);
  return report.scrolls.map((scroll) => ({
    key: `${caseId}/${report.viewport.width}x${report.viewport.height}@${report.viewport.scale}/initial/scroll/${scroll.name}`,
    caseId,
    viewport: report.viewport,
    state: "initial",
    kind: "scroll",
    actualScrollY: scroll.actual,
    maxScrollY: report.maxScrollY,
    viewportHeight: report.viewport.height,
    sourceSha256: report.htmlSha256,
    referenceSha256: sha256File(resolveArtifact(scroll.capturePath)),
  }));
}

function canonicalTocImages(
  report: CanonicalReport,
  catalog: FeatureCatalog,
): TrustedImageExpectation[] {
  if (!report.entries || report.entries.length === 0)
    throw new Error("canonical TOC manifest has no entries");
  const caseId = canonicalCaseId(catalog, report.htmlPath);
  return report.entries.map((entry) => ({
    key: `${caseId}/${report.viewport.width}x${report.viewport.height}@${report.viewport.scale}/initial/toc/${entry.destination}`,
    caseId,
    viewport: report.viewport,
    state: "initial",
    kind: "toc",
    actualScrollY: entry.scrollY,
    maxScrollY: report.maxScrollY,
    viewportHeight: report.viewport.height,
    sourceSha256: report.htmlSha256,
    referenceSha256: sha256File(resolveArtifact(entry.capturePath)),
  }));
}

function resolveArtifact(file: string): string {
  const resolved = path.resolve(file);
  if (!fs.existsSync(resolved)) throw new Error(`canonical capture is missing: ${file}`);
  return resolved;
}

function readCanonicalPerformance(file: string): TrustedPerformance {
  const value = JSON.parse(fs.readFileSync(path.resolve(file), "utf8")) as JsonValue;
  if (!record(value) || !record(value.performance))
    throw new Error("canonical performance report is missing performance");
  return parseTrustedPerformance(value.performance);
}

export function assertStage90Manifest(value: JsonValue, trustedValue: JsonValue): void {
  const manifest = parseStage90(value);
  const trusted = parseTrusted(trustedValue);
  assertIdentity(manifest, trusted);
  assertImages(manifest.images, trusted);
  assertScrollGroups(manifest.images, trusted.images);
  assertPerformance(manifest.performance, trusted.performance);
}

function assertIdentity(manifest: Stage90Manifest, trusted: TrustedStage90Expectations): void {
  if (
    manifest.sourceSha256 !== trusted.sourceSha256 ||
    manifest.binarySha256 !== trusted.binarySha256
  )
    throw new Error("stage provenance differs from trusted expectation");
}

function assertImages(images: StageImageResult[], trusted: TrustedStage90Expectations): void {
  const expected = new Map(trusted.images.map((image) => [image.key, image]));
  const actual = new Set(images.map((image) => image.key));
  if (expected.size === 0 || expected.size !== trusted.images.length)
    throw new Error("trusted image coverage is empty or duplicated");
  if (actual.size !== images.length || actual.size !== expected.size)
    throw new Error("stage90 image coverage is incomplete or duplicated");
  for (const image of images) {
    const reference = expected.get(image.key);
    if (!reference) throw new Error(`unexpected stage90 image: ${image.key}`);
    compareImageContract(image, reference, trusted.binarySha256);
  }
  const toc = new Set(trusted.tocKeys);
  if (toc.size !== trusted.tocKeys.length) throw new Error("duplicate trusted TOC key");
  for (const key of toc) {
    const image = images.find((entry) => entry.key === key);
    if (image?.kind !== "toc" || !image.targetVisible) throw new Error(`missing TOC image: ${key}`);
  }
}

function compareImageContract(
  image: StageImageResult,
  expected: TrustedImageExpectation,
  binarySha: string,
): void {
  if (
    image.caseId !== expected.caseId ||
    image.state !== expected.state ||
    image.kind !== expected.kind ||
    image.viewport.width !== expected.viewport.width ||
    image.viewport.height !== expected.viewport.height ||
    image.viewport.scale !== expected.viewport.scale ||
    image.actualScrollY !== expected.actualScrollY ||
    image.maxScrollY !== expected.maxScrollY ||
    image.viewportHeight !== expected.viewportHeight
  )
    throw new Error(`stage image contract differs: ${image.key}`);
  if (image.actualScrollY < 0 || image.actualScrollY > image.maxScrollY)
    throw new Error(`invalid scroll position: ${image.key}`);
  if (
    image.rmseScore < STAGE90_THRESHOLD ||
    image.maeScore < STAGE90_THRESHOLD ||
    image.rmseScore > 100 ||
    image.maeScore > 100
  )
    throw new Error(`stage90 image score invalid: ${image.key}`);
  if (!image.targetVisible || image.operationState !== "matched")
    throw new Error(`stage operation state is not matched: ${image.key}`);
  if (
    image.sourceSha256 !== expected.sourceSha256 ||
    image.referenceSha256 !== expected.referenceSha256 ||
    image.binarySha256 !== binarySha
  )
    throw new Error(`stage90 provenance differs: ${image.key}`);
}

function assertScrollGroups(images: StageImageResult[], expected: TrustedImageExpectation[]): void {
  const groups = new Map<string, StageImageResult[]>();
  for (const image of images.filter((entry) => entry.kind === "scroll")) {
    const key = `${image.caseId}/${viewportKey(image.viewport)}/${image.state}`;
    const group = groups.get(key) ?? [];
    group.push(image);
    groups.set(key, group);
  }
  const expectedGroups = new Set(
    expected
      .filter((entry) => entry.kind === "scroll")
      .map((entry) => `${entry.caseId}/${viewportKey(entry.viewport)}/${entry.state}`),
  );
  if (groups.size !== expectedGroups.size)
    throw new Error("scroll case/viewport/state coverage is incomplete");
  for (const group of groups.values()) assertOneScrollGroup(group);
}

function assertOneScrollGroup(group: StageImageResult[]): void {
  const sorted = [...group].sort((left, right) => left.actualScrollY - right.actualScrollY);
  let end = 0;
  for (const image of sorted) {
    if (image.actualScrollY > end + 0.01) throw new Error(`scroll coverage gap: ${image.key}`);
    end = Math.max(end, image.actualScrollY + image.viewportHeight);
  }
  const bottom = sorted.at(-1);
  if (!bottom || bottom.actualScrollY < bottom.maxScrollY - 0.01)
    throw new Error("bottom scroll coverage is missing");
}

function assertPerformance(
  actual: Stage90Manifest["performance"],
  trusted: TrustedPerformance,
): void {
  if (actual.environment !== trusted.environment || actual.conditionId !== trusted.conditionId)
    throw new Error("performance measurement conditions differ");
  if (
    actual.phases.length !== REQUIRED_PHASES.length ||
    new Set(actual.phases.map((phase) => phase.name)).size !== actual.phases.length
  )
    throw new Error("performance phases incomplete or duplicated");
  for (const name of REQUIRED_PHASES) {
    const phase = actual.phases.find((item) => item.name === name);
    const expected = trusted.phases.find((item) => item.name === name);
    if (
      !phase ||
      !expected ||
      !Number.isInteger(phase.measurementPairs) ||
      phase.measurementPairs < 1
    )
      throw new Error(`performance phase missing: ${name}`);
    if (
      phase.baselineMs !== expected.baselineMs ||
      phase.baselineMemoryBytes !== expected.baselineMemoryBytes ||
      phase.observedMs !== expected.observedMs ||
      phase.observedMemoryBytes !== expected.observedMemoryBytes ||
      phase.measurementPairs !== expected.measurementPairs
    )
      throw new Error(`performance differs from measured evidence: ${name}`);
    if (
      !positive(phase.observedMs) ||
      !positive(phase.baselineMs) ||
      !positive(phase.observedMemoryBytes) ||
      !positive(phase.baselineMemoryBytes) ||
      phase.observedMs > phase.baselineMs ||
      phase.observedMemoryBytes > phase.baselineMemoryBytes
    )
      throw new Error(`performance regressed: ${name}`);
  }
}

function parseStage90(value: JsonValue): Stage90Manifest {
  if (
    !record(value) ||
    value.profile !== "stage90" ||
    value.threshold !== STAGE90_THRESHOLD ||
    !Array.isArray(value.images) ||
    !record(value.performance)
  )
    throw new Error("invalid stage90 profile or threshold");
  return {
    profile: "stage90",
    threshold: 90,
    sourceSha256: sha(value.sourceSha256, "source SHA"),
    binarySha256: sha(value.binarySha256, "binary SHA"),
    images: value.images.map(parseImage),
    performance: parsePerformance(value.performance),
  };
}
function parseTrusted(value: JsonValue): TrustedStage90Expectations {
  if (
    !record(value) ||
    value.profile !== "stage90" ||
    value.threshold !== 90 ||
    !Array.isArray(value.images) ||
    !Array.isArray(value.tocKeys) ||
    !record(value.performance)
  )
    throw new Error("invalid trusted expectation");
  return {
    profile: "stage90",
    threshold: 90,
    sourceSha256: sha(value.sourceSha256, "trusted source SHA"),
    binarySha256: sha(value.binarySha256, "trusted binary SHA"),
    images: value.images.map(parseTrustedImage),
    tocKeys: value.tocKeys.map((key) => text(key, "TOC key")),
    performance: parseTrustedPerformance(value.performance),
  };
}
function parseImage(value: JsonValue): StageImageResult {
  if (!record(value) || value.operationState !== "matched" || value.targetVisible !== true)
    throw new Error("invalid stage image operation state");
  return {
    ...parseTrustedImage(value),
    rmseScore: score(value.rmseScore),
    maeScore: score(value.maeScore),
    binarySha256: sha(value.binarySha256, "binary SHA"),
    targetVisible: true,
    operationState: "matched",
    evidenceId: text(value.evidenceId, "evidence ID"),
  };
}
function parseTrustedImage(value: JsonValue): TrustedImageExpectation {
  if (!record(value)) throw new Error("invalid trusted image");
  const kind = value.kind;
  if (kind !== "viewport" && kind !== "scroll" && kind !== "toc" && kind !== "state")
    throw new Error("invalid image kind");
  return {
    key: text(value.key, "image key"),
    caseId: text(value.caseId, "case ID"),
    viewport: parseViewport(value.viewport),
    state: text(value.state, "state"),
    kind,
    actualScrollY: finite(value.actualScrollY, "actual scroll Y"),
    maxScrollY: finite(value.maxScrollY, "max scroll Y"),
    viewportHeight: positiveNumber(value.viewportHeight, "viewport height"),
    sourceSha256: sha(value.sourceSha256, "source SHA"),
    referenceSha256: sha(value.referenceSha256, "reference SHA"),
  };
}
function parseViewport(value: JsonValue | undefined): StageViewport {
  if (!record(value)) throw new Error("invalid viewport");
  return {
    width: positiveNumber(value.width, "viewport width"),
    height: positiveNumber(value.height, "viewport height"),
    scale: positiveNumber(value.scale, "viewport scale"),
  };
}
function parsePerformance(value: { [key: string]: JsonValue }): Stage90Manifest["performance"] {
  if (!Array.isArray(value.phases)) throw new Error("invalid performance phases");
  return {
    environment: text(value.environment, "environment"),
    conditionId: text(value.conditionId, "condition"),
    phases: value.phases.map((item) => {
      if (!record(item)) throw new Error("invalid performance phase");
      return {
        name: text(item.name, "phase"),
        observedMs: positiveNumber(item.observedMs, "observed time"),
        baselineMs: positiveNumber(item.baselineMs, "baseline time"),
        observedMemoryBytes: positiveNumber(item.observedMemoryBytes, "observed memory"),
        baselineMemoryBytes: positiveNumber(item.baselineMemoryBytes, "baseline memory"),
        measurementPairs: positiveNumber(item.measurementPairs, "measurement pairs"),
      };
    }),
  };
}
function parseTrustedPerformance(value: { [key: string]: JsonValue }): TrustedPerformance {
  if (!Array.isArray(value.phases)) throw new Error("invalid trusted performance phases");
  const phases = value.phases.map((item) => {
    if (!record(item)) throw new Error("invalid trusted performance phase");
    return {
      name: text(item.name, "trusted phase"),
      baselineMs: positiveNumber(item.baselineMs, "trusted baseline time"),
      baselineMemoryBytes: positiveNumber(item.baselineMemoryBytes, "trusted baseline memory"),
      observedMs: positiveNumber(item.observedMs, "trusted observed time"),
      observedMemoryBytes: positiveNumber(item.observedMemoryBytes, "trusted observed memory"),
      measurementPairs: positiveNumber(item.measurementPairs, "trusted measurement pairs"),
    };
  });
  if (
    phases.length !== REQUIRED_PHASES.length ||
    new Set(phases.map((phase) => phase.name)).size !== phases.length ||
    REQUIRED_PHASES.some((name) => !phases.some((phase) => phase.name === name))
  )
    throw new Error("trusted performance phases incomplete or duplicated");
  return {
    environment: text(value.environment, "trusted environment"),
    conditionId: text(value.conditionId, "trusted condition"),
    phases,
  };
}
function viewportKey(viewport: StageViewport): string {
  return `${viewport.width}x${viewport.height}@${viewport.scale}`;
}
function record(value: JsonValue | undefined): value is { [key: string]: JsonValue } {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
function text(value: JsonValue | undefined, name: string): string {
  if (typeof value !== "string" || value.length === 0) throw new Error(`invalid ${name}`);
  return value;
}
function sha(value: JsonValue | undefined, name: string): string {
  const result = text(value, name);
  if (!/^[a-f0-9]{64}$/i.test(result)) throw new Error(`invalid ${name}`);
  return result;
}
function finite(value: JsonValue | undefined, name: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) throw new Error(`invalid ${name}`);
  return value;
}
function positiveNumber(value: JsonValue | undefined, name: string): number {
  const result = finite(value, name);
  if (result <= 0) throw new Error(`invalid ${name}`);
  return result;
}
function score(value: JsonValue | undefined): number {
  const result = finite(value, "score");
  if (result < 0 || result > 100) throw new Error("invalid score");
  return result;
}
function positive(value: number): boolean {
  return Number.isFinite(value) && value > 0;
}
