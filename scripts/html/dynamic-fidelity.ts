import { randomUUID } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { chromium, type Page } from "playwright";
import { validateFontProvenance } from "./coretext-font-provenance";
import { applyEventsInteraction } from "./dynamic-events";
import { assertExpectedStateMatches, type JsonValue, specForDynamicCase } from "./dynamic-state";
import {
  type CatalogCase,
  type FeatureCaseResult,
  loadFeatureCatalog,
  readExpectedState,
} from "./feature-catalog";
import { type FidelityPhase, fidelityThresholdForPhase } from "./fidelity-phase";
import {
  assertExpectedInputSha,
  compareImages,
  type ScoreReport,
  sha256File,
} from "./reference-capture";

export type DynamicViewport = { width: number; height: number };

export const DYNAMIC_VIEWPORTS = [
  { width: 1280, height: 900 },
  { width: 960, height: 720 },
  { width: 620, height: 900 },
] as const;

export async function captureDynamicStatePair(
  fixture: string,
  captureChrome: () => Promise<JsonValue>,
  captureKrr: (sourceSha256: string) => JsonValue,
): Promise<{ sourceSha256: string; chromeState: JsonValue; krrState: JsonValue }> {
  const sourceSha256 = sha256File(fixture);
  const chromeState = await captureChrome();
  assertExpectedInputSha(sourceSha256, sha256File(fixture));
  const krrState = captureKrr(sourceSha256);
  assertExpectedInputSha(sourceSha256, sha256File(fixture));
  return { sourceSha256, chromeState, krrState };
}

export function captureKrrState(
  binary: string,
  fixture: string,
  expression: string,
  output: string,
  caseId = "dynamic-dom",
  viewport: DynamicViewport = DYNAMIC_VIEWPORTS[0],
  expectedHtmlSha256?: string,
): JsonValue {
  const sourceSha256 = sha256File(fixture);
  assertExpectedInputSha(expectedHtmlSha256, sourceSha256);
  const binarySha256 = sha256File(binary);
  const nonce = randomUUID();
  const result = Bun.spawnSync(
    [binary, "writes_fixture_feature_state_interactive", "--nocapture"],
    {
      env: {
        ...process.env,
        KRR_HTML_FEATURE_INPUT: fixture,
        KRR_HTML_FEATURE_EXPRESSION: expression,
        KRR_HTML_FEATURE_NONCE: nonce,
        KRR_HTML_FEATURE_CASE: caseId,
        KRR_HTML_FEATURE_BINARY_SHA256: binarySha256,
        KRR_HTML_FEATURE_WIDTH: String(viewport.width),
        KRR_HTML_FEATURE_HEIGHT: String(viewport.height),
        KRR_HTML_FEATURE_OUTPUT: output,
      },
    },
  );
  if (result.exitCode !== 0)
    throw new Error(
      new TextDecoder().decode(result.stdout) + new TextDecoder().decode(result.stderr),
    );
  if (sha256File(binary) !== binarySha256) throw new Error("Dynamic probe binary changed");
  assertExpectedInputSha(sourceSha256, sha256File(fixture));
  if (!new TextDecoder().decode(result.stdout).includes("test result: ok. 1 passed"))
    throw new Error("Dynamic diagnostic did not execute exactly one test");
  return readKrrDynamicState(output, nonce, sourceSha256, binarySha256);
}

export function readKrrDynamicState(
  output: string,
  nonce: string,
  sourceSha256: string,
  binarySha256: string,
): JsonValue {
  const report: JsonValue = JSON.parse(fs.readFileSync(output, "utf8"));
  if (typeof report !== "object" || report === null) throw new Error("Invalid dynamic report");
  const value = report as Record<string, JsonValue>;
  if (value.executionNonce !== nonce || value.htmlSha256 !== sourceSha256)
    throw new Error("Stale dynamic report");
  if (value.state === undefined) throw new Error("Missing dynamic state");
  if (typeof value.capturePath !== "string" || !fs.existsSync(value.capturePath))
    throw new Error("Missing post-operation dynamic capture");
  if (value.binarySha256 !== binarySha256) throw new Error("Dynamic binary provenance mismatch");
  return value.state;
}

interface KrrDynamicCaptureIdentity {
  schema: "krr-html84-raw-condition-v1";
  conditionId: string;
  caseId: string;
  state: string;
  operationKind: string;
  executionNonce: string;
  htmlSha256: string;
  binarySha256: string;
  viewport: DynamicViewport & { scale: number };
}

export type KrrDynamicCapture = KrrDynamicCaptureIdentity &
  (
    | {
        status: "observed";
        capturePath: string;
        captureSha256: string;
        stateValue: JsonValue;
        operationEvidence: Record<string, JsonValue>;
        frame: { generation: number; contentHeight: number; scrollY: number };
        targetVisible: true;
        resourceProvenance: { inventoryComplete: true; requests: JsonValue[] };
        fontProvenance: JsonValue[];
      }
    | {
        status: "unknown";
        unknownReason: string;
        stateValue?: JsonValue;
        capturePath?: string;
        captureSha256?: string;
        operationEvidence: Record<string, JsonValue>;
        observedOperationEvidence?: Record<string, JsonValue>;
        frame?: { generation: number; contentHeight?: number; scrollY?: number };
        geometry?: JsonValue[];
        targetVisible?: boolean;
        resourceProvenance?: JsonValue;
        fontProvenance?: JsonValue[];
      }
  );

/** Reads the finite catalog wire format without weakening the legacy state-only reader. */
export function readKrrDynamicCapture(
  output: string,
  expected: {
    conditionId: string;
    caseId: string;
    state: string;
    operationKind: string;
    nonce: string;
    sourceSha256: string;
    binarySha256: string;
    viewport: DynamicViewport & { scale: number };
    targetSelector?: string;
  },
): KrrDynamicCapture {
  const parsed: JsonValue = JSON.parse(fs.readFileSync(output, "utf8"));
  if (!isJsonRecord(parsed)) throw new Error("Invalid catalog capture report");
  const report = parsed;
  assertOnlyKeys(
    report,
    [
      "schema",
      "status",
      "conditionId",
      "unknownReason",
      "observedOperationEvidence",
      "caseId",
      "boundaryState",
      "operationKind",
      "executionNonce",
      "htmlSha256",
      "binarySha256",
      "capturePath",
      "captureSha256",
      "viewport",
      "state",
      "geometry",
      "operationEvidence",
      "frame",
      "targetVisible",
      "resourceProvenance",
      "fontProvenance",
    ],
    "catalog capture report",
  );
  if (
    report.schema !== "krr-html84-raw-condition-v1" ||
    report.executionNonce !== expected.nonce ||
    report.conditionId !== expected.conditionId ||
    report.caseId !== expected.caseId ||
    report.boundaryState !== expected.state ||
    report.operationKind !== expected.operationKind ||
    report.htmlSha256 !== expected.sourceSha256 ||
    report.binarySha256 !== expected.binarySha256
  ) {
    throw new Error(`Catalog capture identity mismatch: ${expected.conditionId}`);
  }
  if (!sameViewportRecord(report.viewport, expected.viewport))
    throw new Error(`Catalog capture viewport mismatch: ${expected.conditionId}`);
  if (report.status === "unknown") return readUnknownCatalogCapture(report, expected);
  if (report.status !== "observed") throw new Error("Catalog capture status is not recognized");
  if (report.targetVisible !== true)
    throw new Error(`Catalog target is not observed visible: ${expected.conditionId}`);
  const evidence = requireJsonRecord(report.operationEvidence, "operation evidence");
  assertOperationEvidence(
    expected.operationKind,
    evidence,
    expected.targetSelector,
    expected.sourceSha256,
    expectedObserverLogForOperation(expected),
    report.state,
    expectedEventStateForOperation(expected),
    expectedResourceStateForOperation(expected),
    expected.sourceSha256,
  );
  const frame = requireJsonRecord(report.frame, "frame observation");
  if (
    !positiveInteger(frame.generation) ||
    !positiveNumber(frame.contentHeight) ||
    !nonNegativeNumber(frame.scrollY)
  )
    throw new Error(`Invalid actual frame metadata: ${expected.conditionId}`);
  const capturePath = requireText(report.capturePath, "capture path");
  const resolvedCapture = path.resolve(capturePath);
  if (!fs.existsSync(resolvedCapture) || fs.lstatSync(resolvedCapture).isSymbolicLink())
    throw new Error(`Missing or indirect catalog capture: ${expected.conditionId}`);
  const captureSha256 = requireSha(report.captureSha256, "capture SHA");
  if (sha256File(resolvedCapture) !== captureSha256)
    throw new Error(`Catalog capture PNG digest mismatch: ${expected.conditionId}`);
  const resourceProvenance = requireResourceProvenance(report.resourceProvenance);
  const fontProvenance = requireFontProvenance(report.fontProvenance);
  if (report.state === undefined) throw new Error("Missing catalog observed state");
  if (
    !Array.isArray(report.geometry) ||
    report.geometry.length === 0 ||
    !report.geometry.every(isActualGeometry)
  )
    throw new Error(`Catalog target geometry is missing: ${expected.conditionId}`);
  return {
    schema: "krr-html84-raw-condition-v1",
    status: "observed",
    conditionId: expected.conditionId,
    caseId: expected.caseId,
    state: expected.state,
    operationKind: expected.operationKind,
    executionNonce: expected.nonce,
    htmlSha256: expected.sourceSha256,
    binarySha256: expected.binarySha256,
    capturePath: resolvedCapture,
    captureSha256,
    viewport: { ...expected.viewport },
    stateValue: report.state,
    operationEvidence: evidence,
    frame: {
      generation: frame.generation,
      contentHeight: frame.contentHeight,
      scrollY: frame.scrollY,
    },
    targetVisible: true,
    resourceProvenance: resourceProvenance as { inventoryComplete: true; requests: JsonValue[] },
    fontProvenance,
  };
}

function readUnknownCatalogCapture(
  report: Record<string, JsonValue>,
  expected: {
    conditionId: string;
    caseId: string;
    state: string;
    operationKind: string;
    nonce: string;
    sourceSha256: string;
    binarySha256: string;
    viewport: DynamicViewport & { scale: number };
    targetSelector?: string;
  },
): KrrDynamicCapture {
  const reason = requireText(report.unknownReason, "unknown reason");
  const evidence = requireUnknownOperationEvidence(report, expected, reason);
  const measurements = readUnknownMeasurements(report, expected);
  const capture = readOptionalCapture(report, expected.conditionId);
  const stateValue = report.state;
  return {
    schema: "krr-html84-raw-condition-v1",
    status: "unknown",
    conditionId: expected.conditionId,
    caseId: expected.caseId,
    state: expected.state,
    operationKind: expected.operationKind,
    executionNonce: expected.nonce,
    htmlSha256: expected.sourceSha256,
    binarySha256: expected.binarySha256,
    viewport: { ...expected.viewport },
    operationEvidence: evidence,
    ...measurements,
    unknownReason: reason,
    ...(stateValue === undefined ? {} : { stateValue }),
    ...(capture ?? {}),
  };
}

function requireUnknownOperationEvidence(
  report: Record<string, JsonValue>,
  expected: {
    caseId: string;
    state: string;
    operationKind: string;
    sourceSha256: string;
    conditionId: string;
  },
  reason: string,
): Record<string, JsonValue> {
  const evidence = requireJsonRecord(report.operationEvidence, "unknown operation evidence");
  assertOnlyKeys(evidence, ["schema", "kind", "status", "reason"], "unknown operation evidence");
  if (
    evidence.schema !== "krr-html84-operation-evidence-v1" ||
    evidence.kind !== expected.operationKind ||
    evidence.status !== "unknown" ||
    evidence.reason !== reason
  )
    throw new Error(`Unknown operation contract mismatch: ${expected.conditionId}`);
  const observedValue = report.observedOperationEvidence;
  if (observedValue !== undefined) {
    const observed = requireJsonRecord(observedValue, "observed operation evidence");
    assertOperationEvidence(
      expected.operationKind,
      observed,
      undefined,
      expected.sourceSha256,
      expectedObserverLogForOperation(expected),
      report.state,
      expectedEventStateForOperation(expected),
      expectedResourceStateForOperation(expected),
      expected.sourceSha256,
    );
  }
  return evidence;
}

type UnknownMeasurements = {
  frame?: { generation: number; contentHeight?: number; scrollY?: number };
  geometry?: JsonValue[];
  targetVisible?: boolean;
  resourceProvenance?: { inventoryComplete: boolean; requests: JsonValue[] };
  fontProvenance?: JsonValue[];
  observedOperationEvidence?: Record<string, JsonValue>;
};

function readUnknownMeasurements(
  report: Record<string, JsonValue>,
  expected: {
    caseId: string;
    state: string;
    operationKind: string;
    sourceSha256: string;
    targetSelector?: string;
  },
): UnknownMeasurements {
  if (report.targetVisible !== undefined && typeof report.targetVisible !== "boolean")
    throw new Error("Unknown report target visibility is invalid");
  const observedOperationEvidence = readUnknownObservedOperationEvidence(report, expected);
  return {
    ...(report.frame === undefined ? {} : { frame: requireUnknownFrame(report.frame) }),
    ...(report.geometry === undefined ? {} : { geometry: requireUnknownGeometry(report.geometry) }),
    ...(report.targetVisible === undefined
      ? {}
      : { targetVisible: report.targetVisible as boolean }),
    ...(report.resourceProvenance === undefined
      ? {}
      : { resourceProvenance: requireResourceProvenance(report.resourceProvenance, true) }),
    ...(report.fontProvenance === undefined
      ? {}
      : { fontProvenance: requireFontProvenance(report.fontProvenance, true) }),
    ...(observedOperationEvidence ? { observedOperationEvidence } : {}),
  };
}

function readUnknownObservedOperationEvidence(
  report: Record<string, JsonValue>,
  expected: {
    caseId: string;
    state: string;
    operationKind: string;
    sourceSha256: string;
    targetSelector?: string;
  },
): Record<string, JsonValue> | undefined {
  if (report.observedOperationEvidence === undefined) return undefined;
  const evidence = requireJsonRecord(
    report.observedOperationEvidence,
    "observed operation evidence",
  );
  assertOperationEvidence(
    expected.operationKind,
    evidence,
    expected.targetSelector,
    expected.sourceSha256,
    expectedObserverLogForOperation(expected),
    report.state,
    expectedEventStateForOperation(expected),
    expectedResourceStateForOperation(expected),
    expected.sourceSha256,
  );
  return evidence;
}

function readOptionalCapture(
  report: Record<string, JsonValue>,
  conditionId: string,
): { capturePath?: string; captureSha256?: string } {
  const hasCapturePath = typeof report.capturePath === "string";
  const hasCaptureSha = typeof report.captureSha256 === "string";
  if (hasCapturePath !== hasCaptureSha)
    throw new Error("Unknown report capture identity is partial");
  if (!hasCapturePath || !hasCaptureSha) return {};
  return verifyCaptureArtifact(
    report.capturePath as string,
    report.captureSha256 as string,
    conditionId,
  );
}

function requireUnknownFrame(value: JsonValue): {
  generation: number;
  contentHeight?: number;
  scrollY?: number;
} {
  const frame = requireJsonRecord(value, "unknown frame observation");
  assertOnlyKeys(frame, ["generation", "contentHeight", "scrollY"], "unknown frame");
  if (!positiveInteger(frame.generation)) throw new Error("Unknown frame generation is invalid");
  if (frame.contentHeight !== undefined && !positiveNumber(frame.contentHeight))
    throw new Error("Unknown frame content height is invalid");
  if (frame.scrollY !== undefined && !nonNegativeNumber(frame.scrollY))
    throw new Error("Unknown frame scroll is invalid");
  return {
    generation: frame.generation,
    ...(typeof frame.contentHeight === "number" ? { contentHeight: frame.contentHeight } : {}),
    ...(typeof frame.scrollY === "number" ? { scrollY: frame.scrollY } : {}),
  };
}

function requireUnknownGeometry(value: JsonValue): JsonValue[] {
  if (!Array.isArray(value) || !value.every(isActualGeometry))
    throw new Error("Unknown report geometry is invalid");
  return value;
}

function verifyCaptureArtifact(
  capturePath: string,
  captureSha: string,
  conditionId: string,
): { capturePath: string; captureSha256: string } {
  const resolvedCapture = path.resolve(capturePath);
  if (!fs.existsSync(resolvedCapture) || fs.lstatSync(resolvedCapture).isSymbolicLink())
    throw new Error(`Missing or indirect catalog capture: ${conditionId}`);
  const captureSha256 = requireSha(captureSha, "capture SHA");
  if (sha256File(resolvedCapture) !== captureSha256)
    throw new Error(`Catalog capture PNG digest mismatch: ${conditionId}`);
  return { capturePath: resolvedCapture, captureSha256 };
}

export function captureKrrCatalogCondition(
  binary: string,
  fixture: string,
  expression: string,
  output: string,
  request: {
    conditionId: string;
    caseId: string;
    state: string;
    operationKind: string;
    targetSelector?: string;
    viewport: DynamicViewport & { scale: number };
  },
): KrrDynamicCapture {
  const sourceSha256 = sha256File(fixture);
  const binarySha256 = sha256File(binary);
  const nonce = randomUUID();
  const operation = {
    schema: "krr-html84-operation-v1",
    conditionId: request.conditionId,
    caseId: request.caseId,
    boundaryState: request.state,
    operationKind: request.operationKind,
    viewport: request.viewport,
    ...(request.targetSelector ? { targetSelector: request.targetSelector } : {}),
  };
  const result = Bun.spawnSync(
    [binary, "writes_fixture_feature_state_interactive", "--nocapture"],
    {
      env: {
        ...process.env,
        KRR_HTML_FEATURE_INPUT: fixture,
        KRR_HTML_FEATURE_EXPRESSION: expression,
        KRR_HTML_FEATURE_NONCE: nonce,
        KRR_HTML_FEATURE_CASE: request.caseId,
        KRR_HTML_FEATURE_BINARY_SHA256: binarySha256,
        KRR_HTML_FEATURE_WIDTH: String(request.viewport.width),
        KRR_HTML_FEATURE_HEIGHT: String(request.viewport.height),
        KRR_HTML_FEATURE_OUTPUT: output,
        KRR_HTML_FEATURE_OPERATION_JSON: JSON.stringify(operation),
      },
    },
  );
  if (result.exitCode !== 0)
    throw new Error(
      new TextDecoder().decode(result.stdout) + new TextDecoder().decode(result.stderr),
    );
  if (sha256File(binary) !== binarySha256) throw new Error("Catalog probe binary changed");
  if (sha256File(fixture) !== sourceSha256) throw new Error("Catalog fixture changed");
  if (!new TextDecoder().decode(result.stdout).includes("test result: ok. 1 passed"))
    throw new Error("Catalog diagnostic did not execute exactly one test");
  return readKrrDynamicCapture(output, {
    ...request,
    nonce,
    sourceSha256,
    binarySha256,
  });
}

function assertOperationEvidence(
  operationKind: string,
  evidence: Record<string, JsonValue>,
  targetSelector?: string,
  sourceSha256?: string,
  expectedObserverLog?: JsonValue,
  reportState?: JsonValue,
  expectedEventState?: JsonValue,
  expectedResourceState?: JsonValue,
  expectedOperationSha256?: string,
): void {
  if (
    evidence.schema !== "krr-html84-operation-evidence-v1" ||
    evidence.kind !== operationKind ||
    evidence.sourceSha256Before !== sourceSha256 ||
    evidence.sourceSha256After !== sourceSha256
  )
    throw new Error(`Operation evidence does not prove ${operationKind}`);
  const target = requireJsonRecord(evidence.target, "operation target");
  if (
    target.visible !== true ||
    !positiveInteger(target.nodeId) ||
    (targetSelector !== undefined && target.selector !== targetSelector)
  )
    throw new Error(`Operation target differs from catalog: ${targetSelector}`);
  const preFrame = requireJsonRecord(evidence.preFrame, "pre-operation frame");
  const postFrame = requireJsonRecord(evidence.postFrame, "post-operation frame");
  if (
    !positiveInteger(preFrame.generation) ||
    !positiveInteger(postFrame.generation) ||
    !nonNegativeNumber(preFrame.scrollY) ||
    !nonNegativeNumber(postFrame.scrollY)
  )
    throw new Error("Operation frame trace is incomplete");
  const beforeAfter = requireJsonRecord(evidence.state, "operation state transition");
  if (beforeAfter.before === undefined || beforeAfter.after === undefined)
    throw new Error("Operation state transition is missing actual before/after state");
  assertOperationSpecificEvidence(
    operationKind,
    evidence,
    beforeAfter,
    expectedObserverLog,
    reportState,
    expectedEventState,
    expectedResourceState,
    expectedOperationSha256,
  );
}

function assertOperationSpecificEvidence(
  operationKind: string,
  evidence: Record<string, JsonValue>,
  state: Record<string, JsonValue>,
  expectedObserverLog?: JsonValue,
  reportState?: JsonValue,
  expectedEventState?: JsonValue,
  expectedResourceState?: JsonValue,
  sourceSha256?: string,
): void {
  switch (operationKind) {
    case "load-and-observe-target":
    case "script-load-observe":
      if (evidence.delivery !== "session-load")
        throw new Error("Load operation did not prove a fresh session load");
      return;
    case "input-state-empty":
    case "input-state-disabled-observe":
      if (requireJsonArray(evidence.steps, "operation steps").length !== 0)
        throw new Error("Observation-only input state dispatched an action");
      return;
    case "input-state-focus-visible-tab":
      if (
        !requireJsonArray(evidence.steps, "operation steps").some(
          (step) =>
            isJsonRecord(step) &&
            step.kind === "key" &&
            step.key === "Tab" &&
            positiveInteger(step.targetNodeId),
        )
      )
        throw new Error("Tab operation trace is missing the actual key dispatch");
      return;
    case "input-state-filled-save":
    case "input-state-selected":
    case "input-state-expanded":
      assertPointerTrace(requireJsonArray(evidence.steps, "operation steps"));
      if (
        stableJson(requireJsonValue(state.before, "state before")) ===
        stableJson(requireJsonValue(state.after, "state after"))
      )
        throw new Error("Input operation did not prove an actual state transition");
      return;
    case "navigation-pointer-click":
      assertPointerTrace(requireJsonArray(evidence.steps, "operation steps"));
      if (
        typeof evidence.hashBefore !== "string" ||
        typeof evidence.hashAfter !== "string" ||
        evidence.hashAfter === evidence.hashBefore ||
        evidence.destination !== evidence.hashAfter
      )
        throw new Error("Navigation operation did not prove the requested hash transition");
      return;
    case "navigation-all-pointer-clicks":
      assertAllNavigationCaptures(
        evidence,
        requireJsonArray(evidence.steps, "operation steps"),
        sourceSha256,
      );
      return;
    case "script-real-events":
      assertScriptEventTrace(evidence, state, reportState, expectedEventState);
      return;
    case "script-await-timer-microtask":
      assertAsyncTimerEvidence(evidence, state, reportState, sourceSha256);
      return;
    case "script-await-mutation-delivery":
      assertMutationDeliveryEvidence(evidence, state, expectedObserverLog, reportState);
      return;
    case "script-await-resource-font":
      assertSettledOperationEvidence(
        evidence,
        state,
        expectedResourceState,
        reportState,
        "session-load-settled-resource-state",
        "Resource/font",
      );
      return;
    default:
      throw new Error(`Unsupported finite operation: ${operationKind}`);
  }
}

function readExpectedObserverLog(expected: {
  caseId: string;
  state: string;
  sourceSha256: string;
}): JsonValue {
  if (expected.caseId !== "observers")
    throw new Error("Mutation delivery is only defined for the observers catalog case");
  const entry = loadFeatureCatalog().scriptCases.find((candidate) => candidate.id === "observers");
  if (
    entry === undefined ||
    entry.fixture !== "aspects/script-observers.html" ||
    entry.expectedStateId !== expected.state
  )
    throw new Error("Mutation delivery does not match the observers fixture contract");
  const fixture = path.resolve("tests/fixtures/html", entry.fixture);
  if (sha256File(fixture) !== expected.sourceSha256)
    throw new Error("Observers fixture changed before its expected callback log was read");
  return readExpectedState(fixture, entry.expectedStateId);
}

function expectedObserverLogForOperation(expected: {
  caseId: string;
  state: string;
  operationKind: string;
  sourceSha256: string;
}): JsonValue | undefined {
  return expected.operationKind === "script-await-mutation-delivery"
    ? readExpectedObserverLog(expected)
    : undefined;
}

function readExpectedEventState(expected: {
  caseId: string;
  state: string;
  sourceSha256: string;
}): JsonValue {
  if (expected.caseId !== "events")
    throw new Error("Real event delivery is only defined for the events catalog case");
  const entry = loadFeatureCatalog().scriptCases.find((candidate) => candidate.id === "events");
  if (
    entry === undefined ||
    entry.fixture !== "aspects/script-events.html" ||
    entry.expectedStateId !== expected.state
  )
    throw new Error("Real event trace does not match the events fixture contract");
  const fixture = path.resolve("tests/fixtures/html", entry.fixture);
  if (sha256File(fixture) !== expected.sourceSha256)
    throw new Error("Events fixture changed before its expected state was read");
  return readExpectedState(fixture, entry.expectedStateId);
}

function readExpectedAsyncState(sourceSha256: string | undefined): JsonValue {
  const entry = loadFeatureCatalog().scriptCases.find((candidate) => candidate.id === "async");
  if (
    entry === undefined ||
    entry.fixture !== "aspects/script-async.html" ||
    entry.expectedStateId !== "expected-state"
  )
    throw new Error("Timer settlement does not match the async fixture contract");
  const fixture = path.resolve("tests/fixtures/html", entry.fixture);
  if (sourceSha256 === undefined || sha256File(fixture) !== sourceSha256)
    throw new Error("Async fixture changed before its expected state was read");
  return readExpectedState(fixture, entry.expectedStateId);
}

function expectedEventStateForOperation(expected: {
  caseId: string;
  state: string;
  operationKind: string;
  sourceSha256: string;
}): JsonValue | undefined {
  return expected.operationKind === "script-real-events"
    ? readExpectedEventState(expected)
    : undefined;
}

function readExpectedResourceState(expected: {
  caseId: string;
  state: string;
  sourceSha256: string;
}): JsonValue {
  if (expected.caseId !== "resources")
    throw new Error("Resource settlement is only defined for the resources catalog case");
  const entry = loadFeatureCatalog().scriptCases.find((candidate) => candidate.id === "resources");
  if (
    entry === undefined ||
    entry.fixture !== "aspects/script-resources.html" ||
    entry.expectedStateId !== expected.state
  )
    throw new Error("Resource settlement does not match the resources fixture contract");
  const fixture = path.resolve("tests/fixtures/html", entry.fixture);
  if (sha256File(fixture) !== expected.sourceSha256)
    throw new Error("Resources fixture changed before its expected settled state was read");
  return readExpectedState(fixture, entry.expectedStateId);
}

function expectedResourceStateForOperation(expected: {
  caseId: string;
  state: string;
  operationKind: string;
  sourceSha256: string;
}): JsonValue | undefined {
  return expected.operationKind === "script-await-resource-font"
    ? readExpectedResourceState(expected)
    : undefined;
}

function assertMutationDeliveryEvidence(
  evidence: Record<string, JsonValue>,
  state: Record<string, JsonValue>,
  expectedObserverLog: JsonValue | undefined,
  reportState: JsonValue | undefined,
): void {
  if (evidence.delivery === "mutation-observer") {
    assertDeliveredTransition(evidence, state, "mutation-observer");
    return;
  }
  assertSettledOperationEvidence(
    evidence,
    state,
    expectedObserverLog,
    reportState,
    "session-load-settled-observer-state",
    "Mutation",
  );
}

function assertSettledOperationEvidence(
  evidence: Record<string, JsonValue>,
  state: Record<string, JsonValue>,
  expectedState: JsonValue | undefined,
  reportState: JsonValue | undefined,
  stepKind: string,
  operationName: string,
): void {
  if (evidence.delivery !== "session-load-settled")
    throw new Error(`${operationName} delivery lacks an observed fresh-load settlement`);
  if (expectedState === undefined || reportState === undefined)
    throw new Error(`${operationName} delivery is missing its fixture and report state bindings`);
  assertOnlyKeys(
    evidence,
    [
      "schema",
      "kind",
      "status",
      "sourceSha256Before",
      "sourceSha256After",
      "preFrame",
      "postFrame",
      "target",
      "state",
      "steps",
      "delivery",
      "status",
    ],
    "settled operation evidence",
  );
  if (evidence.status !== "observed")
    throw new Error(`${operationName} settled operation evidence is not observed`);
  assertOnlyKeys(state, ["before", "after"], "settled operation state");
  const steps = requireJsonArray(evidence.steps, "settled operation steps");
  if (steps.length !== 1) throw new Error("Fresh-load settled evidence needs one producer step");
  const step = requireJsonRecord(steps[0], "settled operation step");
  assertOnlyKeys(step, ["kind", "stateBefore", "stateAfter"], "settled producer step");
  if (step.kind !== stepKind)
    throw new Error(`${operationName} evidence has the wrong fresh-load producer step`);
  const expected = stableJson(expectedState);
  for (const [name, value] of [
    ["operation state before", state.before],
    ["operation state after", state.after],
    ["producer state before", step.stateBefore],
    ["producer state after", step.stateAfter],
    ["raw report state", reportState],
  ] as const) {
    if (value === undefined || stableJson(value) !== expected)
      throw new Error(`Settled ${operationName.toLowerCase()} ${name} differs from fixture state`);
  }
}

function assertAsyncTimerEvidence(
  evidence: Record<string, JsonValue>,
  state: Record<string, JsonValue>,
  reportState: JsonValue | undefined,
  sourceSha256: string | undefined,
): void {
  if (evidence.delivery === "timer-and-microtask") {
    assertDeliveredTransition(evidence, state, "timer-and-microtask");
    return;
  }
  if (evidence.delivery !== "session-load-settled") {
    throw new Error("Timer operation lacks an observed delivery or startup settlement");
  }
  assertOnlyKeys(
    evidence,
    [
      "schema",
      "kind",
      "status",
      "sourceSha256Before",
      "sourceSha256After",
      "preFrame",
      "postFrame",
      "target",
      "state",
      "steps",
      "delivery",
    ],
    "async settled operation evidence",
  );
  if (evidence.status !== "observed")
    throw new Error("Async startup settlement evidence is not observed");
  assertOnlyKeys(state, ["before", "after"], "async settled operation state");
  const steps = requireJsonArray(evidence.steps, "async settled operation steps");
  if (steps.length !== 1) throw new Error("Async startup settlement needs one host advance step");
  const step = requireJsonRecord(steps[0], "async host advance step");
  assertOnlyKeys(
    step,
    [
      "kind",
      "elapsedMs",
      "timerRan",
      "microtaskCheckpoint",
      "stateBefore",
      "stateAfter",
      "fixtureStateVerified",
    ],
    "async host advance step",
  );
  if (
    step.kind !== "host-time-advance" ||
    step.elapsedMs !== 0 ||
    step.timerRan !== false ||
    step.microtaskCheckpoint !== "runtime-advance-timers" ||
    step.fixtureStateVerified !== true
  )
    throw new Error("Async startup settlement lacks the exact verified host advance");
  const expected = stableJson(readExpectedAsyncState(sourceSha256));
  for (const [name, value] of [
    ["operation state before", state.before],
    ["operation state after", state.after],
    ["host state before", step.stateBefore],
    ["host state after", step.stateAfter],
    ["report state", reportState],
  ] as const) {
    if (value === undefined || stableJson(value) !== expected)
      throw new Error(`Async startup settlement ${name} differs from fixture state`);
  }
}

function assertAllNavigationCaptures(
  evidence: Record<string, JsonValue>,
  operationSteps: JsonValue[],
  sourceSha256?: string,
): void {
  assertOnlyKeys(
    evidence,
    [
      "schema",
      "kind",
      "status",
      "sourceSha256Before",
      "sourceSha256After",
      "preFrame",
      "postFrame",
      "target",
      "state",
      "steps",
      "delivery",
      "links",
    ],
    "all-TOC operation evidence",
  );
  if (evidence.status !== "observed")
    throw new Error("All-TOC operation evidence must be observed");
  const destinations = ["#a", "#b", "#c"];
  const links = requireJsonArray(evidence.links, "navigation link trace");
  if (sourceSha256 === undefined || links.length !== destinations.length)
    throw new Error("All-TOC operation is missing ordered per-link capture evidence");
  const captures = new Set<string>();
  const targetNodeIds = new Set<number>();
  let previousHash = "";
  const allPointerSteps: JsonValue[] = [];
  for (const [index, value] of links.entries()) {
    const checked = assertNavigationLink(value, destinations[index], previousHash, sourceSha256);
    if (targetNodeIds.has(checked.targetNodeId))
      throw new Error("TOC link traces reused an actual target node");
    targetNodeIds.add(checked.targetNodeId);
    allPointerSteps.push(...checked.pointerTrace);
    if (captures.has(checked.capturePath))
      throw new Error("TOC link capture paths must be distinct physical files");
    captures.add(checked.capturePath);
    previousHash = checked.destination;
  }
  assertDetailedPointerTrace(operationSteps);
  if (stableJson(allPointerSteps) !== stableJson(operationSteps))
    throw new Error("All-TOC operation steps differ from per-link pointer traces");
}

function assertNavigationLink(
  value: JsonValue,
  destination: string | undefined,
  previousHash: string,
  sourceSha256: string | undefined,
): { destination: string; targetNodeId: number; pointerTrace: JsonValue[]; capturePath: string } {
  const link = requireJsonRecord(value, "navigation link evidence");
  assertOnlyKeys(
    link,
    [
      "destination",
      "hash",
      "scrollY",
      "capturePath",
      "captureSha256",
      "targetVisible",
      "svgPath",
      "svgSha256",
      "hashBefore",
      "hashAfter",
      "frame",
      "pointerTrace",
      "sourceSha256",
    ],
    "navigation link evidence",
  );
  if (
    destination === undefined ||
    sourceSha256 === undefined ||
    link.destination !== destination ||
    link.hash !== destination ||
    link.hashBefore !== previousHash ||
    link.hashAfter !== destination ||
    link.sourceSha256 !== sourceSha256 ||
    link.targetVisible !== true ||
    typeof link.scrollY !== "number" ||
    !nonNegativeNumber(link.scrollY)
  )
    throw new Error("All-TOC target, source, or hash sequence differs from the catalog");
  const frame = requireJsonRecord(link.frame, "navigation destination frame");
  assertOnlyKeys(frame, ["generation", "contentHeight", "scrollY"], "navigation frame");
  if (
    !positiveInteger(frame.generation) ||
    !positiveNumber(frame.contentHeight) ||
    !nonNegativeNumber(frame.scrollY) ||
    frame.scrollY !== link.scrollY
  )
    throw new Error("All-TOC destination frame is incomplete or inconsistent");
  const pointerTrace = requireJsonArray(link.pointerTrace, "navigation link pointer trace");
  if (pointerTrace.length !== 3) throw new Error("Each TOC link needs one actual pointer click");
  assertDetailedPointerTrace(pointerTrace);
  const finalPointer = requireJsonRecord(pointerTrace[2], "final navigation pointer step");
  const finalPointerFrame = requireJsonRecord(
    finalPointer.postFrame,
    "final navigation pointer frame",
  );
  if (
    finalPointerFrame.generation !== frame.generation ||
    finalPointerFrame.scrollY !== frame.scrollY
  )
    throw new Error("TOC screenshot frame differs from the actual final pointer frame");
  const pointerTarget = requireJsonRecord(pointerTrace[0], "navigation pointer target");
  const targetNodeId = positiveInteger(pointerTarget.targetNodeId)
    ? pointerTarget.targetNodeId
    : undefined;
  if (targetNodeId === undefined) throw new Error("TOC pointer target node is invalid");
  const capturePathValue = requireText(link.capturePath, "TOC capture path");
  if (!path.isAbsolute(capturePathValue)) throw new Error("TOC link capture path must be absolute");
  const capturePath = path.resolve(capturePathValue);
  if (
    !fs.existsSync(capturePath) ||
    fs.lstatSync(capturePath).isSymbolicLink() ||
    !fs.lstatSync(capturePath).isFile()
  )
    throw new Error("TOC link capture path is not a physical file");
  const captureSha256 = requireSha(link.captureSha256, "TOC capture SHA");
  if (sha256File(capturePath) !== captureSha256)
    throw new Error("TOC link capture file digest mismatch");
  return { destination, targetNodeId, pointerTrace, capturePath };
}

function assertScriptEventTrace(
  evidence: Record<string, JsonValue>,
  operationState: Record<string, JsonValue>,
  reportState: JsonValue | undefined,
  expectedState: JsonValue | undefined,
): void {
  if (evidence.delivery !== "session-load")
    throw new Error("Script event operation does not prove a fresh session load");
  assertOnlyKeys(
    evidence,
    [
      "schema",
      "kind",
      "status",
      "sourceSha256Before",
      "sourceSha256After",
      "preFrame",
      "postFrame",
      "target",
      "state",
      "steps",
      "delivery",
      "events",
    ],
    "script event operation evidence",
  );
  if (evidence.status !== "observed")
    throw new Error("Script event operation evidence must be observed");
  const events = requireJsonArray(evidence.events, "script event trace");
  const steps = requireJsonArray(evidence.steps, "script event operation steps");
  if (events.length !== 6 || steps.length !== 9)
    throw new Error("Script event trace must contain six events and nine actual input steps");
  const focus = requireJsonRecord(events[0], "focus event");
  const input = requireJsonRecord(events[1], "input event");
  const keydown = requireJsonRecord(events[2], "keydown event");
  const keyup = requireJsonRecord(events[3], "keyup event");
  const click = requireJsonRecord(events[4], "click event");
  const submit = requireJsonRecord(events[5], "submit event");
  const expectedTypes = ["focus", "input", "keydown", "keyup", "click", "submit"];
  const orderedEvents = [focus, input, keydown, keyup, click, submit];
  if (
    orderedEvents.some(
      (event, index) => event.type !== expectedTypes[index] || !positiveInteger(event.targetNodeId),
    )
  )
    throw new Error("Script events are missing, reordered, or lack actual target ids");
  const fieldMove = requireJsonRecord(steps[0], "first focus pointer step");
  const textStep = steps[3];
  const keyDownStep = steps[4];
  const keyUpStep = steps[5];
  const submitMove = requireJsonRecord(steps[6], "first submit pointer step");
  const focusPointer = steps.slice(0, 3);
  const submitPointer = steps.slice(6, 9);
  assertDetailedPointerTrace(focusPointer);
  assertDetailedPointerTrace(submitPointer);
  const fieldTarget = requireJsonRecord(fieldMove, "focus pointer target").targetNodeId;
  const submitTarget = requireJsonRecord(submitMove, "submit pointer target").targetNodeId;
  if (
    focus.targetNodeId !== fieldTarget ||
    input.targetNodeId !== fieldTarget ||
    keydown.targetNodeId !== fieldTarget ||
    keyup.targetNodeId !== fieldTarget ||
    click.targetNodeId !== submitTarget ||
    submit.targetNodeId === submitTarget
  )
    throw new Error("Script event targets differ from actual input and submit dispatches");
  assertOnlyKeys(
    focus,
    ["type", "targetNodeId", "focused", "stateBefore", "stateAfter"],
    "focus event",
  );
  assertOnlyKeys(input, ["type", "targetNodeId", "stateBefore", "stateAfter"], "input event");
  assertOnlyKeys(
    keydown,
    ["type", "targetNodeId", "key", "stateBefore", "stateAfter"],
    "keydown event",
  );
  assertOnlyKeys(
    keyup,
    ["type", "targetNodeId", "key", "stateBefore", "stateAfter"],
    "keyup event",
  );
  assertOnlyKeys(click, ["type", "targetNodeId", "stateBefore", "stateAfter"], "click event");
  assertOnlyKeys(submit, ["type", "targetNodeId", "stateBefore", "stateAfter"], "submit event");
  if (focus.focused !== true || keydown.key !== "Shift" || keyup.key !== "Shift")
    throw new Error("Script focus/key event evidence differs from the dispatched actions");
  const text = requireJsonRecord(textStep, "text dispatch step");
  const down = requireJsonRecord(keyDownStep, "key-down dispatch step");
  const up = requireJsonRecord(keyUpStep, "key-up dispatch step");
  assertOnlyKeys(
    text,
    ["kind", "targetNodeId", "text", "stateBefore", "stateAfter"],
    "text dispatch",
  );
  assertOnlyKeys(
    down,
    ["kind", "targetNodeId", "key", "stateBefore", "stateAfter"],
    "key-down dispatch",
  );
  assertOnlyKeys(
    up,
    ["kind", "targetNodeId", "key", "stateBefore", "stateAfter"],
    "key-up dispatch",
  );
  if (
    text.kind !== "text" ||
    text.targetNodeId !== fieldTarget ||
    text.text !== "入力" ||
    down.kind !== "key_down" ||
    down.targetNodeId !== fieldTarget ||
    down.key !== "Shift" ||
    up.kind !== "key_up" ||
    up.targetNodeId !== fieldTarget ||
    up.key !== "Shift"
  )
    throw new Error("Script operation steps do not match the real text and key dispatches");
  const statePairs: Array<[JsonValue | undefined, JsonValue | undefined]> = [
    [focus.stateAfter, input.stateBefore],
    [input.stateBefore, text.stateBefore],
    [input.stateAfter, text.stateAfter],
    [input.stateAfter, keydown.stateBefore],
    [keydown.stateBefore, down.stateBefore],
    [keydown.stateAfter, down.stateAfter],
    [keydown.stateAfter, keyup.stateBefore],
    [keyup.stateBefore, up.stateBefore],
    [keyup.stateAfter, up.stateAfter],
    [keyup.stateAfter, click.stateBefore],
    [click.stateBefore, submit.stateBefore],
    [click.stateAfter, submit.stateAfter],
    [operationState.before, focus.stateBefore],
    [operationState.after, reportState],
    [reportState, expectedState],
    [submit.stateAfter, expectedState],
  ];
  if (
    expectedState === undefined ||
    reportState === undefined ||
    statePairs.some(
      ([left, right]) =>
        left === undefined || right === undefined || stableJson(left) !== stableJson(right),
    )
  )
    throw new Error("Script event states are not consistently bound to actions and fixture output");
}

function assertDetailedPointerTrace(steps: JsonValue[]): void {
  assertPointerTrace(steps);
  for (const value of steps) {
    const step = requireJsonRecord(value, "pointer dispatch step");
    assertOnlyKeys(
      step,
      ["kind", "targetNodeId", "x", "y", "preFrame", "postFrame"],
      "pointer dispatch step",
    );
    for (const frameName of ["preFrame", "postFrame"]) {
      const frame = requireJsonRecord(step[frameName], `${frameName} pointer frame`);
      assertOnlyKeys(
        frame,
        ["generation", "scrollY", "contentHeight"],
        `${frameName} pointer frame`,
      );
      if (
        !positiveInteger(frame.generation) ||
        !nonNegativeNumber(frame.scrollY) ||
        (frame.contentHeight !== undefined && !nonNegativeNumber(frame.contentHeight))
      )
        throw new Error("Pointer dispatch frame lacks actual generation or scroll");
    }
  }
}

function assertDeliveredTransition(
  evidence: Record<string, JsonValue>,
  state: Record<string, JsonValue>,
  delivery: string,
): void {
  if (
    evidence.delivery !== delivery ||
    stableJson(requireJsonValue(state.before, "state before")) ===
      stableJson(requireJsonValue(state.after, "state after"))
  )
    throw new Error(`${delivery} operation did not prove a state transition`);
}

function isJsonRecord(value: JsonValue): value is Record<string, JsonValue> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function requireJsonRecord(value: JsonValue | undefined, name: string): Record<string, JsonValue> {
  if (value === undefined || !isJsonRecord(value)) throw new Error(`Invalid ${name}`);
  return value;
}

function requireJsonValue(value: JsonValue | undefined, name: string): JsonValue {
  if (value === undefined) throw new Error(`Missing ${name}`);
  return value;
}

function requireJsonArray(value: JsonValue | undefined, name: string): JsonValue[] {
  if (!Array.isArray(value) || value.some((entry) => !isJsonRecord(entry)))
    throw new Error(`Unknown or invalid ${name}`);
  return value;
}

function requireResourceProvenance(
  value: JsonValue | undefined,
  allowIncomplete = false,
): { inventoryComplete: boolean; requests: JsonValue[] } {
  const inventory = requireJsonRecord(value, "resource provenance");
  if (
    typeof inventory.inventoryComplete !== "boolean" ||
    (!allowIncomplete && inventory.inventoryComplete !== true) ||
    !Array.isArray(inventory.requests)
  )
    throw new Error("Resource inventory is incomplete or unknown");
  const entries = inventory.requests;
  if (
    !entries.every(
      (entry) =>
        isJsonRecord(entry) &&
        typeof entry.url === "string" &&
        entry.url.length > 0 &&
        typeof entry.bodyPath === "string" &&
        path.isAbsolute(entry.bodyPath) &&
        fs.existsSync(entry.bodyPath) &&
        typeof entry.bytesSha256 === "string" &&
        /^[a-f0-9]{64}$/.test(entry.bytesSha256) &&
        entry.loaded === true &&
        sha256File(entry.bodyPath) === entry.bytesSha256,
    )
  )
    throw new Error("Resource identity or fetch outcome is unknown");
  return { inventoryComplete: inventory.inventoryComplete, requests: entries };
}

function requireFontProvenance(value: JsonValue | undefined, allowEmpty = false): JsonValue[] {
  return validateFontProvenance(value, allowEmpty) as unknown as JsonValue[];
}

function isActualGeometry(value: JsonValue): boolean {
  return (
    isJsonRecord(value) &&
    positiveInteger(value.nodeId) &&
    [value.x, value.y, value.width, value.height].every(
      (part) => typeof part === "number" && Number.isFinite(part),
    ) &&
    positiveNumber(value.width) &&
    positiveNumber(value.height)
  );
}

function assertPointerTrace(steps: JsonValue[]): void {
  const dispatches = steps.filter(
    (step) => isJsonRecord(step) && ["move", "down", "up"].includes(String(step.kind)),
  );
  if (
    dispatches.length < 3 ||
    dispatches.length % 3 !== 0 ||
    dispatches.some(
      (step) =>
        !isJsonRecord(step) ||
        !positiveInteger(step.targetNodeId) ||
        typeof step.x !== "number" ||
        !Number.isFinite(step.x) ||
        typeof step.y !== "number" ||
        !Number.isFinite(step.y),
    )
  )
    throw new Error("Pointer dispatch trace is incomplete");
  for (let index = 0; index < dispatches.length; index += 3) {
    const group = dispatches.slice(index, index + 3);
    if (!group.every(isJsonRecord)) throw new Error("Pointer dispatch group is invalid");
    const [move, down, up] = group;
    if (
      move?.kind !== "move" ||
      down?.kind !== "down" ||
      up?.kind !== "up" ||
      move.targetNodeId !== down.targetNodeId ||
      move.targetNodeId !== up.targetNodeId ||
      move.x !== down.x ||
      move.x !== up.x ||
      move.y !== down.y ||
      move.y !== up.y
    )
      throw new Error("Pointer dispatch trace target/order/coordinates differ");
  }
}

export function stableJson(value: JsonValue): string {
  if (Array.isArray(value)) return `[${value.map(stableJson).join(",")}]`;
  if (typeof value === "object" && value !== null)
    return `{${Object.entries(value)
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([key, item]) => `${JSON.stringify(key)}:${stableJson(item)}`)
      .join(",")}}`;
  return JSON.stringify(value) ?? "null";
}

function assertOnlyKeys(record: Record<string, JsonValue>, allowed: string[], name: string): void {
  const extra = Object.keys(record).filter((key) => !allowed.includes(key));
  if (extra.length > 0) throw new Error(`Unknown ${name} fields: ${extra.join(", ")}`);
}

function sameViewportRecord(
  value: JsonValue | undefined,
  expected: DynamicViewport & { scale: number },
): boolean {
  return (
    value !== undefined &&
    isJsonRecord(value) &&
    value.width === expected.width &&
    value.height === expected.height &&
    value.scale === expected.scale
  );
}

function requireText(value: JsonValue | undefined, name: string): string {
  if (typeof value !== "string" || value.length === 0) throw new Error(`Invalid ${name}`);
  return value;
}

function requireSha(value: JsonValue | undefined, name: string): string {
  const sha = requireText(value, name);
  if (!/^[a-f0-9]{64}$/.test(sha)) throw new Error(`Invalid ${name}`);
  return sha;
}

function positiveInteger(value: JsonValue | undefined): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value > 0;
}

function positiveNumber(value: JsonValue | undefined): value is number {
  return typeof value === "number" && Number.isFinite(value) && value > 0;
}

function nonNegativeNumber(value: JsonValue | undefined): value is number {
  return typeof value === "number" && Number.isFinite(value) && value >= 0;
}

async function captureChromeState(
  page: Page,
  entry: CatalogCase,
  output: string,
  viewport: DynamicViewport,
): Promise<JsonValue> {
  const fixture = path.resolve("tests/fixtures/html", entry.fixture);
  const spec = specForDynamicCase(entry.id);
  await page.setViewportSize(viewport);
  await page.goto(`file://${fixture}`, { waitUntil: "networkidle" });
  if (entry.id === "events") {
    await applyEventsInteraction(page);
  }
  await waitForDynamicState(page, entry.id);
  const state = (await page.evaluate((expression) => {
    const script = document.createElement("script");
    script.textContent = `globalThis.__krrCatalogState = (${expression});`;
    document.head.append(script);
    const value =
      (globalThis as typeof globalThis & { __krrCatalogState?: JsonValue }).__krrCatalogState ??
      null;
    delete (globalThis as typeof globalThis & { __krrCatalogState?: JsonValue }).__krrCatalogState;
    script.remove();
    return value;
  }, spec.expression)) as JsonValue;
  const expected = readExpectedState(fixture, entry.expectedStateId);
  assertExpectedStateMatches(expected, state);
  await page.screenshot({ path: output, fullPage: false });
  return state;
}

async function waitForDynamicState(page: Page, caseId: string): Promise<void> {
  await page.waitForFunction(() => document.readyState === "complete");
  const selectors: Record<string, string> = {
    async: "#state",
    observers: "#state",
    resources: "#state",
  };
  const selector = selectors[caseId];
  if (!selector) return;
  await page.waitForFunction((target) => {
    const text = document.querySelector(target)?.textContent ?? "";
    return target === "#state" && text !== "" && text !== "initial" && text !== "waiting";
  }, selector);
}

export function runDynamicStates(
  binary: string,
  outputDirectory: string,
): Array<{ caseId: string; passed: boolean; error?: string }> {
  fs.mkdirSync(outputDirectory, { recursive: true });
  const results = loadFeatureCatalog().scriptCases.map((entry) => {
    const fixture = path.resolve("tests/fixtures/html", entry.fixture);
    const spec = specForDynamicCase(entry.id);
    try {
      const actual = captureKrrState(
        binary,
        fixture,
        spec.expression,
        path.join(outputDirectory, `${entry.id}.json`),
        entry.id,
      );
      assertExpectedStateMatches(readExpectedState(fixture, entry.expectedStateId), actual);
      return { caseId: entry.id, passed: true };
    } catch (error) {
      return { caseId: entry.id, passed: false, error: String(error) };
    }
  });
  fs.writeFileSync(
    path.join(outputDirectory, "results.json"),
    `${JSON.stringify(results, null, 2)}\n`,
  );
  return results;
}

export interface DynamicFidelityResult {
  caseId: string;
  sourceSha256: string;
  viewport: { width: number; height: number };
  score: ScoreReport;
  passed: boolean;
  chromePath: string;
  krrPath: string;
}

export async function runDynamicFidelity(
  binary: string,
  outputDirectory: string,
  minScore = 99,
  options: { phase: "interim" } | undefined = undefined,
): Promise<DynamicFidelityResult[]> {
  const phase: FidelityPhase = options?.phase ?? "final";
  const threshold = fidelityThresholdForPhase(phase, minScore);
  const binarySha256 = sha256File(binary);
  fs.mkdirSync(outputDirectory, { recursive: true });
  const browser = await chromium.launch({ channel: "chrome", headless: true });
  const page = await browser.newPage();
  const results: DynamicFidelityResult[] = [];
  try {
    for (const entry of loadFeatureCatalog().scriptCases) {
      for (const viewport of DYNAMIC_VIEWPORTS) {
        const label = `${entry.id}-${viewport.width}x${viewport.height}`;
        const fixture = path.resolve("tests/fixtures/html", entry.fixture);
        const chromePath = path.join(outputDirectory, `${label}-chrome.png`);
        const krrJson = path.join(outputDirectory, `${label}-krr.json`);
        const krrPath = krrJson.replace(/\.json$/, ".png");
        const { sourceSha256, chromeState, krrState } = await captureDynamicStatePair(
          fixture,
          () => captureChromeState(page, entry, chromePath, viewport),
          (expectedSha) =>
            captureKrrState(
              binary,
              fixture,
              specForDynamicCase(entry.id).expression,
              krrJson,
              entry.id,
              viewport,
              expectedSha,
            ),
        );
        assertExpectedStateMatches(readExpectedState(fixture, entry.expectedStateId), chromeState);
        assertExpectedStateMatches(chromeState, krrState);
        const score = compareImages(chromePath, krrPath, threshold);
        results.push({
          caseId: entry.id,
          sourceSha256,
          viewport,
          score,
          passed: score.passed,
          chromePath,
          krrPath,
        });
      }
    }
  } finally {
    await page.close();
    await browser.close();
  }
  if (sha256File(binary) !== binarySha256)
    throw new Error("Dynamic probe binary changed during fidelity capture");
  const fidelityResultPath = path.join(
    outputDirectory,
    phase === "interim" ? "interim-fidelity-results.json" : "fidelity-results.json",
  );
  const fidelityResult = phase === "interim" ? { phase, threshold, results } : results;
  fs.writeFileSync(fidelityResultPath, `${JSON.stringify(fidelityResult, null, 2)}\n`);
  const scriptCases = loadFeatureCatalog().scriptCases;
  const scriptFeatureResults: FeatureCaseResult[] = scriptCases.map((entry) => {
    const caseResults = results.filter((result) => result.caseId === entry.id);
    const passed =
      caseResults.length === DYNAMIC_VIEWPORTS.length &&
      caseResults.every((result) => result.passed);
    return {
      caseId: entry.id,
      status: passed ? "passed" : "failed",
      features: Object.fromEntries(
        entry.features.map((feature) => [feature, passed ? "passed" : "failed"]),
      ),
    };
  });
  const scriptFeatureResultPath = path.join(
    outputDirectory,
    phase === "interim" ? "interim-script-feature-results.json" : "script-feature-results.json",
  );
  const scriptFeatureResult =
    phase === "interim"
      ? { phase, threshold, results: scriptFeatureResults }
      : scriptFeatureResults;
  fs.writeFileSync(scriptFeatureResultPath, `${JSON.stringify(scriptFeatureResult, null, 2)}\n`);
  if (
    results.length !== scriptCases.length * DYNAMIC_VIEWPORTS.length ||
    results.some((result) => !result.passed)
  ) {
    throw new Error("Dynamic fidelity score gate failed");
  }
  return results;
}
