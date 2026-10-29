import fs from "node:fs";
import path from "node:path";
import { assertCatalogManifestBinaryProof } from "./catalog-capture-binary";
import {
  assertCatalogProducerContext,
  type CatalogCaptureProducerContext,
  sameCatalogProducerContext,
} from "./catalog-capture-producer-context";
import { stableJson } from "./dynamic-fidelity";
import type { JsonValue } from "./dynamic-state";
import { assertCatalogComplete, type CatalogCase, loadFeatureCatalog } from "./feature-catalog";
import { ImageComparisonSession } from "./image-comparison-session";
import { sha256File } from "./reference-capture";
import {
  assertOnlyStrictKeys,
  assertStrictHtml84ChromeReceipt,
  assertStrictHtml84KrrReceipt,
  assertStrictStage90Manifest,
  type Html84ConditionIdentity,
  STAGE90_THRESHOLD,
  type StageViewport,
} from "./stage-score";

export interface StageScoreSuiteCondition {
  conditionId: string;
  caseId: string;
  state: string;
  viewport: StageViewport;
  sourceSha256: string;
  binarySha256: string;
  binaryPath: string;
  operationKind: string;
  targetSelector?: string;
  rawKrrReportPath: string;
  rawKrrReportSha256: string;
  candidate: JsonValue;
}

interface TrustedSuiteCondition
  extends Omit<StageScoreSuiteCondition, "candidate" | "rawKrrReportPath" | "rawKrrReportSha256"> {
  operationKind: string;
  targetSelector?: string;
  chromeReceiptPath: string;
  chromeReceiptSha256: string;
  trusted: JsonValue;
}

export interface StageScoreSuiteManifest {
  profile: "stage90-suite";
  threshold: 90;
  acceptance: false;
  catalogPath: string;
  catalogSha256: string;
  conditions: StageScoreSuiteCondition[];
}

export interface CatalogStage90ConditionPlan {
  profile: "krr-html84-condition-plan-v1";
  acceptance: false;
  catalogPath: string;
  catalogSha256: string;
  conditions: Array<{
    conditionId: string;
    caseId: string;
    state: string;
    operationKind: string;
    targetSelector?: string;
    viewport: StageViewport;
    fixturePath: string;
    sourceSha256: string;
  }>;
}

export interface CatalogRawStage90Result {
  conditionId: string;
  krrCapturePath: string;
  krrCaptureSha256: string;
  chromeCapturePath: string;
  chromeCaptureSha256: string;
  chromeReceiptPath: string;
  chromeReceiptSha256: string;
  score: number;
  rmseScore: number;
  maeScore: number;
  state: JsonValue;
}

export interface CatalogRawStage90ReuseContext {
  planPath: string;
  binaryPath: string;
  chromeExecutablePath: string;
  chromeExecutableSha256: string;
  chromeRuntimeVersion: string;
  producerContext: CatalogCaptureProducerContext;
}

export interface CatalogRawStage90ReuseResult {
  reusableConditionIds: string[];
  rejected: Array<{ conditionId: string; reason: string }>;
}

export function validateCatalogRawStage90Captures(
  manifestValue: JsonValue,
  planValue: JsonValue,
  planPathValue: string,
  catalogPath = "tests/fixtures/html/feature-catalog.json",
): CatalogRawStage90Result[] {
  const manifest = record(manifestValue, "catalog stage90 capture manifest");
  const plan = assertCatalogStage90ConditionPlan(planValue, catalogPath);
  assertRawCaptureManifest(manifest, plan, planPathValue);
  const captures = records(manifest.conditions, "catalog raw captures");
  const byId = new Map(
    captures.map((capture) => [text(capture.conditionId, "capture condition id"), capture]),
  );
  if (captures.length !== plan.conditions.length || byId.size !== captures.length)
    throw new Error("catalog raw capture coverage is incomplete or duplicated");
  const nonces = new Set<string>();
  const comparisonSession = new ImageComparisonSession();
  return plan.conditions.map((condition) => {
    const capture = byId.get(condition.conditionId);
    if (!capture) throw new Error(`catalog raw capture is missing: ${condition.conditionId}`);
    return validateRawCaptureCondition(condition, capture, manifest, nonces, comparisonSession);
  });
}

/** Diagnostic only: identifies captured conditions safe to re-use for the current run. */
export function inspectCatalogRawStage90Reuse(
  manifestValue: JsonValue,
  planValue: JsonValue,
  context: CatalogRawStage90ReuseContext,
  catalogPath = "tests/fixtures/html/feature-catalog.json",
): CatalogRawStage90ReuseResult {
  const plan = assertCatalogStage90ConditionPlan(planValue, catalogPath);
  const planIds = plan.conditions.map((condition) => condition.conditionId);
  const rejectAll = (reason: string): CatalogRawStage90ReuseResult => ({
    reusableConditionIds: [],
    rejected: planIds.map((conditionId) => ({ conditionId, reason })),
  });
  try {
    const manifest = record(manifestValue, "catalog stage90 capture manifest");
    const capturedPlanPath = text(manifest.planPath, "capture plan path");
    assertRawCaptureManifest(manifest, plan, capturedPlanPath);
    const capturedContext = assertCatalogProducerContext(manifest.producerContext);
    assertReusableFontEnvironment(capturedContext, "captured");
    assertReusableFontEnvironment(context.producerContext, "current");
    if (!sameCatalogProducerContext(manifest.producerContext, context.producerContext))
      return rejectAll("capture producer context differs");
    if (
      sha256File(fs.realpathSync(context.planPath)) !==
      sha256File(fs.realpathSync(capturedPlanPath))
    )
      return rejectAll("current plan identity differs");
    assertCurrentBinaryIdentity(manifest, context.binaryPath);
    const currentChromePath = fs.realpathSync(context.chromeExecutablePath);
    const currentChromeSha256 = sha256File(currentChromePath);
    if (currentChromeSha256 !== context.chromeExecutableSha256)
      return rejectAll("current Chrome executable identity differs");
    const capturedChromeRuntimeVersion = text(
      manifest.chromeRuntimeVersion,
      "capture Chrome runtime version",
    );
    if (capturedChromeRuntimeVersion !== context.chromeRuntimeVersion)
      return rejectAll("current Chrome runtime version differs");
    const captures = records(manifest.conditions, "catalog raw captures");
    const byId = new Map(
      captures.map((capture) => [text(capture.conditionId, "capture condition id"), capture]),
    );
    if (captures.length !== plan.conditions.length || byId.size !== captures.length)
      return rejectAll("capture inventory is incomplete or duplicated");
    return inspectRawCaptureConditions(
      plan,
      manifest,
      byId,
      currentChromePath,
      context.chromeExecutableSha256,
    );
  } catch (error) {
    return rejectAll(error instanceof Error ? error.message : String(error));
  }
}

function assertReusableFontEnvironment(value: CatalogCaptureProducerContext, label: string): void {
  if (!value.fontEnvironment?.cacheable)
    throw new Error(`${label} font environment is unavailable for cache reuse`);
}

function inspectRawCaptureConditions(
  plan: CatalogStage90ConditionPlan,
  manifest: { [key: string]: JsonValue },
  captures: Map<string, { [key: string]: JsonValue }>,
  currentChromePath: string,
  currentChromeSha256: string,
): CatalogRawStage90ReuseResult {
  const nonces = new Set<string>();
  const comparisonSession = new ImageComparisonSession();
  const reusableConditionIds: string[] = [];
  const rejected: Array<{ conditionId: string; reason: string }> = [];
  for (const condition of plan.conditions) {
    const capture = captures.get(condition.conditionId);
    if (!capture) {
      rejected.push({ conditionId: condition.conditionId, reason: "capture is missing" });
      continue;
    }
    try {
      validateRawCaptureCondition(condition, capture, manifest, nonces, comparisonSession);
      assertCurrentChromeIdentity(capture, currentChromePath, currentChromeSha256);
      reusableConditionIds.push(condition.conditionId);
    } catch (error) {
      rejected.push({
        conditionId: condition.conditionId,
        reason: error instanceof Error ? error.message : String(error),
      });
    }
  }
  return { reusableConditionIds, rejected };
}

function assertCurrentBinaryIdentity(
  manifest: { [key: string]: JsonValue },
  currentBinaryPathValue: string,
): void {
  const manifestPath = fs.realpathSync(text(manifest.binaryPath, "capture binary path"));
  const currentPath = fs.realpathSync(currentBinaryPathValue);
  if (
    manifestPath !== currentPath ||
    sha256File(currentPath) !== sha(text(manifest.binarySha256, "capture binary SHA"))
  )
    throw new Error("current binary identity differs");
}

function assertCurrentChromeIdentity(
  capture: { [key: string]: JsonValue },
  currentChromePath: string,
  currentChromeSha256: string,
): void {
  const receiptPath = text(capture.chromeReceiptPath, "Chrome receipt path");
  const receipt = record(JSON.parse(fs.readFileSync(receiptPath, "utf8")), "Chrome raw receipt");
  const receiptPathValue = fs.realpathSync(
    text(receipt.browserExecutablePath, "Chrome executable"),
  );
  if (
    receiptPathValue !== currentChromePath ||
    text(receipt.browserExecutableSha256, "Chrome executable SHA") !== currentChromeSha256
  )
    throw new Error("current Chrome executable identity differs");
}

function assertRawCaptureManifest(
  manifest: { [key: string]: JsonValue },
  plan: CatalogStage90ConditionPlan,
  planPathValue: string,
): void {
  const planPath = fs.realpathSync(planPathValue);
  const binaryPath = path.resolve(text(manifest.binaryPath, "capture binary path"));
  assertCatalogManifestBinaryProof(manifest);
  if (
    manifest.profile !== "catalog-stage90-captures" ||
    manifest.threshold !== STAGE90_THRESHOLD ||
    manifest.acceptance !== false ||
    path.resolve(text(manifest.catalogPath, "capture catalog path")) !== plan.catalogPath ||
    manifest.catalogSha256 !== plan.catalogSha256 ||
    path.resolve(text(manifest.planPath, "capture plan path")) !== planPath ||
    manifest.planSha256 !== sha256File(planPath) ||
    path.resolve(text(manifest.catalogPath, "capture catalog path")) !==
      path.resolve("tests/fixtures/html/feature-catalog.json") ||
    fs.realpathSync(binaryPath) !== binaryPath ||
    sha256File(binaryPath) !== sha(text(manifest.binarySha256, "capture binary SHA"))
  )
    throw new Error("catalog raw capture manifest identity differs");
}

function validateRawCaptureCondition(
  condition: CatalogStage90ConditionPlan["conditions"][number],
  capture: { [key: string]: JsonValue },
  manifest: { [key: string]: JsonValue },
  nonces: Set<string>,
  comparisonSession: ImageComparisonSession,
): CatalogRawStage90Result {
  assertRawCaptureTuple(condition, capture, manifest);
  const identity = rawCaptureIdentity(condition, manifest);
  const krr = assertStrictHtml84KrrReceipt(
    text(capture.krrReportPath, "KRR raw report path"),
    sha(text(capture.krrReportSha256, "KRR raw report SHA")),
    identity,
  );
  if (nonces.has(krr.executionNonce))
    throw new Error(`reused KRR execution nonce: ${condition.conditionId}`);
  nonces.add(krr.executionNonce);
  const chromeReceiptPath = text(capture.chromeReceiptPath, "Chrome receipt path");
  const chromeReceiptSha256 = sha(text(capture.chromeReceiptSha256, "Chrome receipt SHA"));
  const chrome = assertStrictHtml84ChromeReceipt(chromeReceiptPath, chromeReceiptSha256, identity);
  if (stableJson(krr.state) !== stableJson(chrome.state))
    throw new Error(`independent raw semantic state differs: ${condition.conditionId}`);
  assertCaptureBindings(capture, krr, chrome, condition.conditionId);
  const score = compareRawImagePair(chrome, krr, comparisonSession);
  if (!score.passed)
    throw new Error(`independent raw image score is below 90: ${condition.conditionId}`);
  return {
    conditionId: condition.conditionId,
    krrCapturePath: krr.capturePath,
    krrCaptureSha256: krr.captureSha256,
    chromeCapturePath: chrome.capturePath,
    chromeCaptureSha256: chrome.captureSha256,
    chromeReceiptPath: path.resolve(chromeReceiptPath),
    chromeReceiptSha256,
    score: score.score,
    rmseScore: score.rmseScore,
    maeScore: score.maeScore,
    state: chrome.state,
  };
}

function compareRawImagePair(
  chrome: ReturnType<typeof assertStrictHtml84ChromeReceipt>,
  krr: ReturnType<typeof assertStrictHtml84KrrReceipt>,
  comparisonSession: ImageComparisonSession,
) {
  return comparisonSession.compare(chrome.capturePath, krr.capturePath, STAGE90_THRESHOLD);
}

function assertRawCaptureTuple(
  condition: CatalogStage90ConditionPlan["conditions"][number],
  capture: { [key: string]: JsonValue },
  manifest: { [key: string]: JsonValue },
): void {
  if (capture.status !== "captured")
    throw new Error(`catalog raw capture is not verified: ${condition.conditionId}`);
  assertCatalogCaptureIdentity(
    condition,
    capture,
    text(manifest.binarySha256, "capture binary SHA"),
  );
  const fontObservation = record(capture.fontIdentityObservation, "font identity observation");
  if (
    capture.fontIdentityRequestConditionId !== condition.conditionId ||
    fontObservation.status !== "observed" ||
    records(fontObservation.unknownReasons, "font identity unknown reasons").length !== 0
  )
    throw new Error(`catalog raw capture tuple differs: ${condition.conditionId}`);
}

export function assertCatalogCaptureIdentity(
  condition: CatalogStage90ConditionPlan["conditions"][number],
  capture: { [key: string]: JsonValue },
  binarySha256: string,
): void {
  if (
    capture.conditionId !== condition.conditionId ||
    capture.caseId !== condition.caseId ||
    capture.state !== condition.state ||
    capture.operationKind !== condition.operationKind ||
    capture.targetSelector !== condition.targetSelector ||
    capture.sourceSha256 !== condition.sourceSha256 ||
    capture.binarySha256 !== binarySha256 ||
    stableJson(capture.viewport as JsonValue) !==
      stableJson({
        width: condition.viewport.width,
        height: condition.viewport.height,
        scale: condition.viewport.scale,
      })
  )
    throw new Error(`catalog raw capture identity differs: ${condition.conditionId}`);
}

function rawCaptureIdentity(
  condition: CatalogStage90ConditionPlan["conditions"][number],
  manifest: { [key: string]: JsonValue },
): Html84ConditionIdentity {
  return {
    conditionId: condition.conditionId,
    caseId: condition.caseId,
    state: condition.state,
    operationKind: condition.operationKind,
    ...(condition.targetSelector ? { targetSelector: condition.targetSelector } : {}),
    viewport: condition.viewport,
    sourceSha256: condition.sourceSha256,
    binarySha256: sha(text(manifest.binarySha256, "capture binary SHA")),
  };
}

function assertCaptureBindings(
  capture: { [key: string]: JsonValue },
  krr: ReturnType<typeof assertStrictHtml84KrrReceipt>,
  chrome: ReturnType<typeof assertStrictHtml84ChromeReceipt>,
  conditionId: string,
): void {
  if (
    path.resolve(text(capture.krrCapturePath, "KRR capture path")) !== krr.capturePath ||
    sha(text(capture.krrCaptureSha256, "KRR capture SHA")) !== krr.captureSha256 ||
    path.resolve(text(capture.chromeCapturePath, "Chrome capture path")) !== chrome.capturePath ||
    sha(text(capture.chromeCaptureSha256, "Chrome capture SHA")) !== chrome.captureSha256 ||
    stableJson(capture.chromeState as JsonValue) !== stableJson(chrome.state)
  )
    throw new Error(`catalog raw capture receipt binding differs: ${conditionId}`);
}

export function createCatalogStage90ConditionPlan(
  catalogPath = "tests/fixtures/html/feature-catalog.json",
): CatalogStage90ConditionPlan {
  const catalog = loadFeatureCatalog(catalogPath);
  assertCatalogComplete(catalog);
  return {
    profile: "krr-html84-condition-plan-v1",
    acceptance: false,
    catalogPath: path.resolve(catalogPath),
    catalogSha256: sha256File(path.resolve(catalogPath)),
    conditions: requiredStageScoreTuples(catalog).map((tuple) => {
      const entry = [...catalog.visualAspects, ...catalog.scriptCases].find(
        (candidate) => candidate.id === tuple.caseId,
      );
      if (!entry) throw new Error(`missing catalog case for ${tuple.conditionId}`);
      const fixturePath = path.resolve("tests/fixtures/html", entry.fixture);
      return { ...tuple, fixturePath, sourceSha256: sha256File(fixturePath) };
    }),
  };
}

export function assertCatalogStage90ConditionPlan(
  value: JsonValue,
  catalogPath = "tests/fixtures/html/feature-catalog.json",
): CatalogStage90ConditionPlan {
  const plan = record(value, "catalog stage90 condition plan");
  if (plan.profile !== "krr-html84-condition-plan-v1" || plan.acceptance !== false)
    throw new Error("invalid catalog stage90 condition plan profile");
  const expected = createCatalogStage90ConditionPlan(catalogPath);
  if (
    path.resolve(text(plan.catalogPath, "catalog plan path")) !== expected.catalogPath ||
    plan.catalogSha256 !== expected.catalogSha256
  )
    throw new Error("catalog stage90 plan source differs");
  const supplied = records(plan.conditions, "catalog plan conditions");
  if (supplied.length !== expected.conditions.length)
    throw new Error("catalog stage90 plan condition coverage is incomplete");
  const byId = new Map(
    supplied.map((condition) => [text(condition.conditionId, "plan condition id"), condition]),
  );
  if (byId.size !== supplied.length)
    throw new Error("catalog stage90 plan has duplicate conditions");
  for (const condition of expected.conditions) {
    const actual = byId.get(condition.conditionId);
    if (!actual) throw new Error(`catalog stage90 plan is missing: ${condition.conditionId}`);
    if (
      actual.caseId !== condition.caseId ||
      actual.state !== condition.state ||
      actual.operationKind !== condition.operationKind ||
      actual.targetSelector !== condition.targetSelector ||
      !sameViewport(actual.viewport, condition.viewport) ||
      path.resolve(text(actual.fixturePath, "fixture path")) !== condition.fixturePath ||
      actual.sourceSha256 !== condition.sourceSha256
    )
      throw new Error(`catalog stage90 plan tuple or source differs: ${condition.conditionId}`);
  }
  return expected;
}

export function assertStageScoreSuite(
  value: JsonValue,
  trustedSuitePlan: JsonValue,
  catalogPath = "tests/fixtures/html/feature-catalog.json",
): void {
  const suite = record(value, "stage score suite");
  if (suite.profile !== "stage90-suite" || suite.threshold !== 90 || suite.acceptance !== false)
    throw new Error("invalid stage score suite profile");
  if (path.resolve(text(suite.catalogPath, "catalog path")) !== path.resolve(catalogPath))
    throw new Error("suite catalog path differs");
  if (sha256File(path.resolve(catalogPath)) !== text(suite.catalogSha256, "catalog SHA"))
    throw new Error("suite catalog SHA differs");
  const catalog = loadFeatureCatalog(catalogPath);
  assertCatalogComplete(catalog);
  const conditions = records(suite.conditions, "suite conditions").map(parseCandidateCondition);
  const trustedPlan = record(trustedSuitePlan, "trusted suite plan");
  if (trustedPlan.profile !== "stage90-trusted-plan")
    throw new Error("invalid trusted suite plan profile");
  const trustedConditions = records(trustedPlan.conditions, "trusted plan conditions").map(
    parseTrustedCondition,
  );
  assertConditionInventory(conditions, catalog);
  assertTrustedConditionInventory(trustedConditions, catalog);
  const executionNonces = new Set<string>();
  const comparisonSession = new ImageComparisonSession();
  for (const condition of conditions) {
    const trusted = trustedConditions.find((item) => item.conditionId === condition.conditionId);
    if (!trusted) throw new Error(`missing trusted condition: ${condition.conditionId}`);
    assertCondition(condition, trusted, catalog, executionNonces, comparisonSession);
  }
}

function parseCandidateCondition(value: { [key: string]: JsonValue }): StageScoreSuiteCondition {
  if (value.trusted !== undefined) throw new Error("candidate suite must not include trusted data");
  assertOnlyStrictKeys(
    value,
    [
      "conditionId",
      "caseId",
      "state",
      "viewport",
      "sourceSha256",
      "binarySha256",
      "binaryPath",
      "operationKind",
      "targetSelector",
      "rawKrrReportPath",
      "rawKrrReportSha256",
      "candidate",
    ],
    "candidate condition",
  );
  return {
    ...parseConditionTuple(value),
    rawKrrReportPath: path.resolve(text(value.rawKrrReportPath, "KRR raw report path")),
    rawKrrReportSha256: sha(text(value.rawKrrReportSha256, "KRR raw report SHA")),
    candidate: required(value.candidate, "candidate condition"),
  };
}

function parseTrustedCondition(value: { [key: string]: JsonValue }): TrustedSuiteCondition {
  assertOnlyStrictKeys(
    value,
    [
      "conditionId",
      "caseId",
      "state",
      "viewport",
      "sourceSha256",
      "binarySha256",
      "binaryPath",
      "operationKind",
      "targetSelector",
      "chromeReceiptPath",
      "chromeReceiptSha256",
      "trusted",
    ],
    "trusted condition",
  );
  return {
    ...parseConditionTuple(value),
    chromeReceiptPath: path.resolve(text(value.chromeReceiptPath, "Chrome receipt path")),
    chromeReceiptSha256: sha(text(value.chromeReceiptSha256, "Chrome receipt SHA")),
    trusted: required(value.trusted, "trusted condition"),
  };
}

function parseConditionTuple(value: {
  [key: string]: JsonValue;
}): Omit<StageScoreSuiteCondition, "candidate" | "rawKrrReportPath" | "rawKrrReportSha256"> {
  const viewport = record(value.viewport, "condition viewport");
  return {
    conditionId: text(value.conditionId, "condition id"),
    caseId: text(value.caseId, "case id"),
    state: text(value.state, "condition state"),
    viewport: {
      width: positive(viewport.width, "viewport width"),
      height: positive(viewport.height, "viewport height"),
      scale: positive(viewport.scale, "viewport scale"),
    },
    sourceSha256: sha(text(value.sourceSha256, "condition source SHA")),
    binarySha256: sha(text(value.binarySha256, "condition binary SHA")),
    binaryPath: path.resolve(text(value.binaryPath, "condition binary path")),
    operationKind: text(value.operationKind, "condition operation kind"),
    ...(value.targetSelector === undefined
      ? {}
      : { targetSelector: text(value.targetSelector, "condition target selector") }),
  };
}

function assertCondition(
  condition: StageScoreSuiteCondition,
  trustedCondition: TrustedSuiteCondition,
  catalog: { scriptCases: CatalogCase[]; visualAspects: CatalogCase[] },
  executionNonces: Set<string>,
  comparisonSession: ImageComparisonSession,
): void {
  const entry = [...catalog.visualAspects, ...catalog.scriptCases].find(
    (item) => item.id === condition.caseId,
  );
  if (!entry) throw new Error(`unrecognized suite case: ${condition.caseId}`);
  const expectedOperation = catalogOperationIdentity(condition.caseId, condition.state);
  if (
    condition.operationKind !== expectedOperation.operationKind ||
    condition.targetSelector !== expectedOperation.targetSelector
  )
    throw new Error(`condition operation differs from catalog: ${condition.conditionId}`);
  assertConditionSourceAndBinary(condition, entry);
  assertTrustedConditionIdentity(condition, trustedCondition);
  const trusted = record(trustedCondition.trusted, "trusted condition");
  const candidate = record(condition.candidate, "candidate condition");
  assertStageManifests(condition, trustedCondition, trusted, candidate, comparisonSession);
  const identity: Html84ConditionIdentity = {
    conditionId: condition.conditionId,
    caseId: condition.caseId,
    state: condition.state,
    operationKind: condition.operationKind,
    ...(condition.targetSelector ? { targetSelector: condition.targetSelector } : {}),
    viewport: condition.viewport,
    sourceSha256: condition.sourceSha256,
    binarySha256: condition.binarySha256,
  };
  assertIndependentRawReceipts(
    condition,
    trustedCondition,
    identity,
    executionNonces,
    comparisonSession,
  );
}

function assertConditionSourceAndBinary(
  condition: StageScoreSuiteCondition,
  entry: CatalogCase,
): void {
  const fixture = path.resolve("tests/fixtures/html", entry.fixture);
  if (sha256File(fixture) !== condition.sourceSha256)
    throw new Error(`condition source SHA differs: ${condition.conditionId}`);
  if (!fsFile(condition.binaryPath) || sha256File(condition.binaryPath) !== condition.binarySha256)
    throw new Error(`condition binary differs: ${condition.conditionId}`);
}

function assertTrustedConditionIdentity(
  condition: StageScoreSuiteCondition,
  trustedCondition: TrustedSuiteCondition,
): void {
  if (
    trustedCondition.caseId !== condition.caseId ||
    trustedCondition.state !== condition.state ||
    trustedCondition.viewport.width !== condition.viewport.width ||
    trustedCondition.viewport.height !== condition.viewport.height ||
    trustedCondition.viewport.scale !== condition.viewport.scale ||
    trustedCondition.sourceSha256 !== condition.sourceSha256 ||
    trustedCondition.binarySha256 !== condition.binarySha256
  )
    throw new Error(`trusted plan tuple differs: ${condition.conditionId}`);
  if (
    trustedCondition.operationKind !== condition.operationKind ||
    trustedCondition.targetSelector !== condition.targetSelector
  )
    throw new Error(`trusted operation identity differs: ${condition.conditionId}`);
}

function assertStageManifests(
  condition: StageScoreSuiteCondition,
  trustedCondition: TrustedSuiteCondition,
  trusted: { [key: string]: JsonValue },
  candidate: { [key: string]: JsonValue },
  comparisonSession: ImageComparisonSession,
): void {
  if (
    trusted.sourceSha256 !== condition.sourceSha256 ||
    trusted.binarySha256 !== condition.binarySha256
  )
    throw new Error(`trusted provenance differs: ${condition.conditionId}`);
  if (
    candidate.sourceSha256 !== condition.sourceSha256 ||
    candidate.binarySha256 !== condition.binarySha256
  )
    throw new Error(`candidate provenance differs: ${condition.conditionId}`);
  if (candidate.profile !== "stage90" || trusted.profile !== "stage90")
    throw new Error(`condition is not an individual stage90 manifest: ${condition.conditionId}`);
  assertManifestTuple(trusted, condition, "trusted");
  assertManifestTuple(candidate, condition, "candidate");
  assertStrictStage90Manifest(condition.candidate, trustedCondition.trusted, comparisonSession);
}

function assertIndependentRawReceipts(
  condition: StageScoreSuiteCondition,
  trustedCondition: TrustedSuiteCondition,
  identity: Html84ConditionIdentity,
  executionNonces: Set<string>,
  comparisonSession: ImageComparisonSession,
): void {
  const krr = assertStrictHtml84KrrReceipt(
    condition.rawKrrReportPath,
    condition.rawKrrReportSha256,
    identity,
  );
  if (executionNonces.has(krr.executionNonce))
    throw new Error(`reused KRR execution nonce: ${condition.conditionId}`);
  executionNonces.add(krr.executionNonce);
  const chrome = assertStrictHtml84ChromeReceipt(
    trustedCondition.chromeReceiptPath,
    trustedCondition.chromeReceiptSha256,
    identity,
  );
  if (stableJson(krr.state) !== stableJson(chrome.state))
    throw new Error(`independent raw semantic state differs: ${condition.conditionId}`);
  const candidateManifest = record(condition.candidate, "candidate condition");
  const candidateImages = records(candidateManifest.images, "candidate images");
  if (
    !candidateImages.some((image) => {
      const candidateImage = record(image, "candidate stage image");
      return (
        candidateImage.candidatePath === krr.capturePath &&
        candidateImage.candidateSha256 === krr.captureSha256
      );
    })
  )
    throw new Error(
      `KRR raw capture is not bound to a candidate stage image: ${condition.conditionId}`,
    );
  if (!comparisonSession.compare(krr.capturePath, chrome.capturePath, 90).passed)
    throw new Error(`independent raw image score is below 90: ${condition.conditionId}`);
}

function assertTrustedConditionInventory(
  conditions: TrustedSuiteCondition[],
  catalog: { scriptCases: CatalogCase[]; visualAspects: CatalogCase[] },
): void {
  const ids = new Set<string>();
  const receiptHashes = new Set<string>();
  for (const condition of conditions) {
    if (ids.has(condition.conditionId))
      throw new Error(`duplicate trusted plan condition: ${condition.conditionId}`);
    ids.add(condition.conditionId);
    if (receiptHashes.has(condition.chromeReceiptSha256))
      throw new Error(`reused Chrome raw receipt SHA: ${condition.conditionId}`);
    receiptHashes.add(condition.chromeReceiptSha256);
  }
  const required = requiredTuples(catalog);
  const tuples = new Set(
    conditions.map((condition) => tupleKey(condition.caseId, condition.state, condition.viewport)),
  );
  if (tuples.size !== conditions.length) throw new Error("duplicate trusted plan tuple");
  for (const tuple of required)
    if (!tuples.has(tuple)) throw new Error(`missing trusted plan tuple: ${tuple}`);
}

function assertManifestTuple(
  manifest: { [key: string]: JsonValue },
  condition: StageScoreSuiteCondition,
  name: string,
): void {
  const images = records(manifest.images, `${name} images`);
  if (images.length === 0) throw new Error(`${name} images are empty`);
  const performance = record(manifest.performance, `${name} performance`);
  if (performance.conditionId !== condition.conditionId)
    throw new Error(`${name} performance condition differs: ${condition.conditionId}`);
  for (const image of images) {
    if (
      image.caseId !== condition.caseId ||
      image.state !== condition.state ||
      !sameViewport(image.viewport, condition.viewport)
    )
      throw new Error(`${name} image tuple differs: ${condition.conditionId}`);
  }
}

function assertConditionInventory(
  conditions: StageScoreSuiteCondition[],
  catalog: { scriptCases: CatalogCase[]; visualAspects: CatalogCase[] },
): void {
  const ids = new Set<string>();
  const receiptHashes = new Set<string>();
  for (const condition of conditions) {
    if (ids.has(condition.conditionId))
      throw new Error(`duplicate suite condition: ${condition.conditionId}`);
    ids.add(condition.conditionId);
  }
  const required = requiredTuples(catalog);
  const tuples = new Set<string>();
  for (const condition of conditions) {
    const tuple = tupleKey(condition.caseId, condition.state, condition.viewport);
    if (!required.has(tuple)) throw new Error(`unrecognized suite tuple: ${tuple}`);
    if (tuples.has(tuple)) throw new Error(`duplicate suite tuple: ${tuple}`);
    assertExactConditionId(condition.conditionId, tuple, "suite");
    tuples.add(tuple);
  }
  for (const tuple of required)
    if (!tuples.has(tuple)) throw new Error(`missing suite tuple: ${tuple}`);
  for (const condition of conditions) {
    if (receiptHashes.has(condition.rawKrrReportSha256))
      throw new Error(`reused KRR raw receipt SHA: ${condition.conditionId}`);
    receiptHashes.add(condition.rawKrrReportSha256);
  }
}

function requiredTuples(catalog: {
  scriptCases: CatalogCase[];
  visualAspects: CatalogCase[];
}): Set<string> {
  const result = new Set<string>();
  const viewports = [
    { width: 1280, height: 900, scale: 1 },
    { width: 960, height: 720, scale: 1 },
    { width: 620, height: 900, scale: 1 },
  ];
  for (const entry of [...catalog.visualAspects, ...catalog.scriptCases]) {
    const states =
      entry.mode === "script" ? [entry.expectedStateId ?? ""] : (entry.boundaryStates ?? []);
    for (const state of states)
      for (const viewport of viewports) result.add(tupleKey(entry.id, state, viewport));
  }
  return result;
}

/** Returns the catalog-authoritative condition inventory used by validation and capture. */
export function requiredStageScoreTuples(
  catalog: { scriptCases: CatalogCase[]; visualAspects: CatalogCase[] } = loadFeatureCatalog(),
): Array<{
  conditionId: string;
  caseId: string;
  state: string;
  operationKind: string;
  targetSelector?: string;
  viewport: StageViewport;
}> {
  return [...requiredTuples(catalog)]
    .map((tuple) => {
      const match = /^(.*)\/([^/]+)\/(\d+)x(\d+)@(\d+(?:\.\d+)?)$/.exec(tuple);
      if (!match) throw new Error(`invalid required stage tuple: ${tuple}`);
      const [, caseId, state, width, height, scale] = match;
      if (!caseId || !state || !width || !height || !scale)
        throw new Error(`invalid required stage tuple: ${tuple}`);
      return {
        conditionId: tuple,
        caseId,
        state,
        ...catalogOperationIdentity(caseId, state),
        viewport: { width: Number(width), height: Number(height), scale: Number(scale) },
      };
    })
    .sort((left, right) => left.conditionId.localeCompare(right.conditionId));
}

function catalogOperationIdentity(
  caseId: string,
  state: string,
): { operationKind: string; targetSelector?: string } {
  const scriptKinds: Record<string, string> = {
    "dynamic-dom": "script-load-observe",
    events: "script-real-events",
    async: "script-await-timer-microtask",
    observers: "script-await-mutation-delivery",
    resources: "script-await-resource-font",
  };
  const scriptKind = scriptKinds[caseId];
  if (scriptKind) return { operationKind: scriptKind };
  const targets: Record<string, string> = {
    "typography/ja-en-mixed": ".mixed",
    "typography/fractional-metrics": ".mixed",
    "typography/nowrap": ".nowrap",
    "typography/long-line": "#long-line",
    "layout/narrow-viewport": ".flex",
    "layout/multi-column": ".grid",
    "layout/min-content": "#min-content",
    "table/long-status": ".badge",
    "table/collapsed-border": "table",
    "table/rich-cell": "tbody",
    "navigation/top": "#toc-a",
    "navigation/middle": "#toc-b",
    "navigation/bottom": "#toc-c",
    "navigation/all-toc-links": ".sidebar .toc-item[href^='#']",
    "input-state/empty": "#name",
    "input-state/filled": "#name",
    "input-state/selected": "#tab-advanced",
    "input-state/expanded": "#accordion",
    "input-state/disabled": "#disabled",
    "input-state/focus-visible": "#name",
    "decoration/stacked": ".alert",
    "decoration/clipped": ".overlay",
    "decoration/translucent": ".overlay",
  };
  const targetSelector = targets[`${caseId}/${state}`];
  if (!targetSelector) throw new Error(`missing finite condition target: ${caseId}/${state}`);
  const inputKind: Record<string, string> = {
    empty: "input-state-empty",
    filled: "input-state-filled-save",
    selected: "input-state-selected",
    expanded: "input-state-expanded",
    disabled: "input-state-disabled-observe",
    "focus-visible": "input-state-focus-visible-tab",
  };
  if (caseId === "input-state") {
    const operationKind = inputKind[state];
    if (!operationKind) throw new Error(`missing input operation: ${state}`);
    return { operationKind, targetSelector };
  }
  if (caseId === "navigation")
    return {
      operationKind:
        state === "all-toc-links" ? "navigation-all-pointer-clicks" : "navigation-pointer-click",
      targetSelector,
    };
  return { operationKind: "load-and-observe-target", targetSelector };
}

function assertExactConditionId(actual: string, expected: string, source: string): void {
  if (actual !== expected)
    throw new Error(`${source} condition id differs from exact tuple: ${actual}`);
}

function tupleKey(caseId: string, state: string, viewport: StageViewport): string {
  return `${caseId}/${state}/${viewport.width}x${viewport.height}@${viewport.scale}`;
}

function sameViewport(value: JsonValue | undefined, expected: StageViewport): boolean {
  const viewport = record(value, "image viewport");
  return (
    viewport.width === expected.width &&
    viewport.height === expected.height &&
    viewport.scale === expected.scale
  );
}

function fsFile(value: string): boolean {
  return path.isAbsolute(value) && requireFile(value);
}

function requireFile(value: string): boolean {
  try {
    return fs.statSync(value).isFile() && !fs.lstatSync(value).isSymbolicLink();
  } catch {
    return false;
  }
}

function record(value: JsonValue | undefined, name: string): { [key: string]: JsonValue } {
  if (typeof value !== "object" || value === null || Array.isArray(value))
    throw new Error(`invalid ${name}`);
  return value;
}

function records(value: JsonValue | undefined, name: string): Array<{ [key: string]: JsonValue }> {
  if (!Array.isArray(value) || value.some((item) => !isRecord(item)))
    throw new Error(`invalid ${name}`);
  return value as Array<{ [key: string]: JsonValue }>;
}

function isRecord(value: JsonValue): value is { [key: string]: JsonValue } {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function required(value: JsonValue | undefined, name: string): JsonValue {
  if (value === undefined) throw new Error(`missing ${name}`);
  return value;
}

function text(value: JsonValue | undefined, name: string): string {
  if (typeof value !== "string" || value.length === 0) throw new Error(`invalid ${name}`);
  return value;
}

function sha(value: string): string {
  if (!/^[0-9a-f]{64}$/.test(value)) throw new Error("invalid SHA-256");
  return value;
}

function positive(value: JsonValue | undefined, name: string): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value <= 0)
    throw new Error(`invalid ${name}`);
  return value;
}
