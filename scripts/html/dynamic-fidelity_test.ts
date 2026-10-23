import { describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { EventEmitter } from "node:events";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { assertCoreTextPostScriptOrder, validateFontProvenance } from "./coretext-font-provenance";

const CORETEXT_TEST_AXIS_ID = 2_003_265_652;
const CORETEXT_TEST_AXIS_MINIMUM = 100;
const CORETEXT_TEST_AXIS_MAXIMUM = 900;
const CORETEXT_TEST_AXIS_DEFAULT = 400;
const CORETEXT_TEST_AXIS_OUT_OF_RANGE = 901;
const CORETEXT_TEST_U32_OVERFLOW = 4_294_967_296;

import type { Page, Request, Response } from "playwright";
import { buildJoinedStaticFontTrace } from "./chromium-font-test-support";
import {
  captureDynamicStatePair,
  captureKrrState,
  readKrrDynamicCapture,
  readKrrDynamicState,
  runDynamicFidelity,
} from "./dynamic-fidelity";
import type { JsonValue } from "./dynamic-state";
import {
  assertFeatureResultReportsComplete,
  type FeatureCaseResult,
  loadFeatureCatalog,
  readExpectedState,
} from "./feature-catalog";
import { fidelityAcceptancePlan, runFidelityAcceptance } from "./fidelity-acceptance-cli";
import { sha256File } from "./reference-capture";
import { createCatalogStage90ConditionPlan } from "./stage-score-suite";
import {
  ChromeResourceBodyCollector,
  type ChromeResourceProvenance,
  filterObserverReusableConditions,
} from "./visual-state-fidelity";

const SOURCE_COMMENT = "\n<!-- html-pair-input-review-marker -->\n";

function runCatalogPlanCli(
  output?: string,
  extraArgument?: string,
): ReturnType<typeof Bun.spawnSync> {
  if (output === undefined)
    return Bun.spawnSync([
      "bun",
      "scripts/html/fidelity-acceptance-cli.ts",
      "catalog-stage90-plan",
    ]);
  if (extraArgument !== undefined)
    return Bun.spawnSync([
      "bun",
      "scripts/html/fidelity-acceptance-cli.ts",
      "catalog-stage90-plan",
      output,
      extraArgument,
    ]);
  return Bun.spawnSync([
    "bun",
    "scripts/html/fidelity-acceptance-cli.ts",
    "catalog-stage90-plan",
    output,
  ]);
}

function expectCatalogPlanArgumentsRejected(output: string): void {
  expect(runCatalogPlanCli().exitCode).not.toBe(0);
  expect(runCatalogPlanCli("").exitCode).not.toBe(0);
  expect(runCatalogPlanCli(output, "unexpected").exitCode).not.toBe(0);
}

function expectPhysicalCatalogPlan(output: string): void {
  const plan = JSON.parse(fs.readFileSync(output, "utf8")) as {
    acceptance?: unknown;
    conditions?: unknown;
  };
  expect(plan.acceptance).toBe(false);
  if (!Array.isArray(plan.conditions)) throw new Error("CLI plan lacks conditions");
  const conditions = plan.conditions as {
    conditionId: string;
    fixturePath: string;
    sourceSha256: string;
  }[];
  expect(conditions).toHaveLength(84);
  expect(new Set(conditions.map(({ conditionId }) => conditionId)).size).toBe(84);
  for (const condition of conditions)
    expect(sha256File(condition.fixturePath)).toBe(condition.sourceSha256);
}

function expectCatalogPlanCollisionPreservesBytes(output: string): void {
  fs.writeFileSync(output, "preserve existing output");
  expect(runCatalogPlanCli(output).exitCode).not.toBe(0);
  expect(fs.readFileSync(output, "utf8")).toBe("preserve existing output");
}

async function withDynamicFixture(
  action: (fixture: string, directory: string) => void | Promise<void>,
): Promise<void> {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "dynamic-paired-input-"));
  const fixture = path.join(directory, "fixture.html");
  fs.copyFileSync("tests/fixtures/html/aspects/script-dynamic-dom.html", fixture);
  try {
    await action(fixture, directory);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
}

function emitChromeResource(
  page: EventEmitter,
  input: {
    url: string;
    status?: number;
    body?: Buffer;
    bodyFailure?: boolean;
    failed?: boolean;
    pending?: boolean;
  },
): void {
  const request = { url: () => input.url, isNavigationRequest: () => false } as Request;
  page.emit("request", request);
  if (input.failed) {
    page.emit("requestfailed", request);
    return;
  }
  if (input.pending) return;
  const response = {
    request: () => request,
    status: () => input.status ?? 200,
    body: async () => {
      if (input.bodyFailure) throw new Error("body unavailable");
      return input.body ?? Buffer.alloc(0);
    },
  } as Response;
  page.emit("response", response);
  page.emit("requestfinished", request);
}

function unknownCaptureWithObservedEvidence(input: {
  report: Record<string, JsonValue>;
  operationEvidence: Record<string, JsonValue>;
  operationKind: string;
  unknownReason: string;
}): Record<string, JsonValue> {
  return {
    ...JSON.parse(JSON.stringify(input.report)),
    status: "unknown",
    unknownReason: input.unknownReason,
    operationEvidence: {
      schema: "krr-html84-operation-evidence-v1",
      kind: input.operationKind,
      status: "unknown",
      reason: input.unknownReason,
    },
    observedOperationEvidence: input.operationEvidence,
  };
}

function invalidOperationStatusEvidence(
  evidence: Record<string, JsonValue>,
): Record<string, JsonValue>[] {
  const withoutStatus = Object.fromEntries(
    Object.entries(evidence).filter(([key]) => key !== "status"),
  ) as Record<string, JsonValue>;
  return [{ ...evidence, status: "unknown" }, withoutStatus, { ...evidence, status: true }];
}

function expectUnknownCapturePreservesObservedEvidence(input: {
  reportPath: string;
  report: Record<string, JsonValue>;
  expected: Parameters<typeof readKrrDynamicCapture>[1];
  operationEvidence: Record<string, JsonValue>;
  operationKind: string;
  unknownReason: string;
}): Record<string, JsonValue> {
  const report = unknownCaptureWithObservedEvidence(input);
  fs.writeFileSync(input.reportPath, JSON.stringify(report));
  const capture = readKrrDynamicCapture(input.reportPath, input.expected);
  expect(capture.status).toBe("unknown");
  if (capture.status === "unknown")
    expect(capture.observedOperationEvidence).toEqual(input.operationEvidence);
  return report;
}

function strictCatalogCaptureReport(input: {
  directory: string;
  fixture: string;
  caseId: string;
  boundaryState: string;
  operationKind: string;
  targetSelector: string;
  state: JsonValue;
  operationEvidence: Record<string, JsonValue>;
}): { report: Record<string, JsonValue>; expected: Parameters<typeof readKrrDynamicCapture>[1] } {
  const sourceSha256 = sha256File(input.fixture);
  const capturePath = path.join(input.directory, "main-capture.png");
  const fontPath = path.join(input.directory, "fixture-font.bin");
  fs.writeFileSync(capturePath, "independent unit capture");
  fs.writeFileSync(fontPath, "actual unit font bytes");
  const report: Record<string, JsonValue> = {
    schema: "krr-html84-raw-condition-v1",
    status: "observed",
    conditionId: `${input.caseId}/${input.boundaryState}/1280x900@1`,
    caseId: input.caseId,
    boundaryState: input.boundaryState,
    operationKind: input.operationKind,
    executionNonce: "strict-operation-nonce",
    htmlSha256: sourceSha256,
    binarySha256: "b".repeat(64),
    capturePath,
    captureSha256: sha256File(capturePath),
    viewport: { width: 1280, height: 900, scale: 1 },
    state: input.state,
    geometry: [{ nodeId: 1, x: 1, y: 1, width: 10, height: 10 }],
    operationEvidence: input.operationEvidence,
    frame: { generation: 1, contentHeight: 900, scrollY: 0 },
    targetVisible: true,
    resourceProvenance: {
      inventoryComplete: true,
      requests: [
        {
          url: `file://${input.fixture}`,
          bodyPath: input.fixture,
          bytesSha256: sourceSha256,
          loaded: true,
        },
      ],
    },
    fontProvenance: [
      {
        family: "sans",
        scope: "capture",
        source: "system",
        filePath: fontPath,
        fileSha256: sha256File(fontPath),
        faceIndex: 0,
        selected: true,
        fileBytesVerified: true,
      },
    ],
  };
  return {
    report,
    expected: {
      conditionId: String(report.conditionId),
      caseId: input.caseId,
      state: input.boundaryState,
      operationKind: input.operationKind,
      nonce: String(report.executionNonce),
      sourceSha256,
      binarySha256: String(report.binarySha256),
      viewport: { width: 1280, height: 900, scale: 1 },
      targetSelector: input.targetSelector,
    },
  };
}

function scriptRealEventsEvidence(
  fixture: string,
  expectedState: JsonValue,
  includeContentHeight: boolean,
): { initial: JsonValue; shifted: JsonValue; evidence: Record<string, JsonValue> } {
  const initial = { result: "", key: "" };
  const shifted = { result: "", key: "Shift" };
  const pointerTrace = (targetNodeId: number, base: number): JsonValue[] =>
    ["move", "down", "up"].map((kind, index) => {
      const frame = (generation: number): Record<string, JsonValue> => ({
        generation,
        scrollY: 0,
        ...(includeContentHeight ? { contentHeight: 900 } : {}),
      });
      return {
        kind,
        targetNodeId,
        x: base,
        y: base,
        preFrame: frame(1 + index),
        postFrame: frame(2 + index),
      };
    });
  const evidence: Record<string, JsonValue> = {
    schema: "krr-html84-operation-evidence-v1",
    kind: "script-real-events",
    status: "observed",
    sourceSha256Before: sha256File(fixture),
    sourceSha256After: sha256File(fixture),
    preFrame: { generation: 1, scrollY: 0 },
    postFrame: { generation: 8, scrollY: 0 },
    target: { selector: "#field", nodeId: 2, visible: true },
    state: { before: initial, after: expectedState },
    steps: [
      ...pointerTrace(2, 20),
      { kind: "text", targetNodeId: 2, text: "入力", stateBefore: initial, stateAfter: initial },
      {
        kind: "key_down",
        targetNodeId: 2,
        key: "Shift",
        stateBefore: initial,
        stateAfter: shifted,
      },
      {
        kind: "key_up",
        targetNodeId: 2,
        key: "Shift",
        stateBefore: shifted,
        stateAfter: shifted,
      },
      ...pointerTrace(4, 40),
    ],
    delivery: "session-load",
    events: [
      {
        type: "focus",
        targetNodeId: 2,
        focused: true,
        stateBefore: initial,
        stateAfter: initial,
      },
      { type: "input", targetNodeId: 2, stateBefore: initial, stateAfter: initial },
      {
        type: "keydown",
        targetNodeId: 2,
        key: "Shift",
        stateBefore: initial,
        stateAfter: shifted,
      },
      { type: "keyup", targetNodeId: 2, key: "Shift", stateBefore: shifted, stateAfter: shifted },
      { type: "click", targetNodeId: 4, stateBefore: shifted, stateAfter: expectedState },
      { type: "submit", targetNodeId: 3, stateBefore: shifted, stateAfter: expectedState },
    ],
  };
  return { initial, shifted, evidence };
}

describe("dynamic capture input binding", () => {
  test("accepts exact settled observer state without inventing a load transition", async () => {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), "dynamic-observer-settled-"));
    const fixture = path.resolve("tests/fixtures/html/aspects/script-observers.html");
    const sourceSha256 = sha256File(fixture);
    const capturePath = path.join(directory, "capture.png");
    const reportPath = path.join(directory, "catalog-report.json");
    const fontPath = path.join(directory, "fixture-font.bin");
    const expectedLog = readExpectedState(fixture);
    fs.writeFileSync(capturePath, "independent observer capture");
    fs.writeFileSync(fontPath, "actual observer font bytes");
    const operationEvidence = {
      schema: "krr-html84-operation-evidence-v1",
      kind: "script-await-mutation-delivery",
      sourceSha256Before: sourceSha256,
      sourceSha256After: sourceSha256,
      preFrame: { generation: 1, contentHeight: 900, scrollY: 0 },
      postFrame: { generation: 1, contentHeight: 900, scrollY: 0 },
      target: { selector: "#state", nodeId: 2, visible: true },
      state: { before: expectedLog, after: expectedLog },
      steps: [
        {
          kind: "session-load-settled-observer-state",
          stateBefore: expectedLog,
          stateAfter: expectedLog,
        },
      ],
      delivery: "session-load-settled",
      status: "observed",
    };
    const report = {
      schema: "krr-html84-raw-condition-v1",
      status: "observed",
      conditionId: "observers/expected-state/1280x900@1",
      caseId: "observers",
      boundaryState: "expected-state",
      operationKind: "script-await-mutation-delivery",
      executionNonce: "observer-settled-nonce",
      htmlSha256: sourceSha256,
      binarySha256: "b".repeat(64),
      capturePath,
      captureSha256: sha256File(capturePath),
      viewport: { width: 1280, height: 900, scale: 1 },
      state: expectedLog,
      geometry: [{ nodeId: 2, x: 1, y: 1, width: 10, height: 10 }],
      operationEvidence,
      frame: { generation: 1, contentHeight: 900, scrollY: 0 },
      targetVisible: true,
      resourceProvenance: {
        inventoryComplete: true,
        requests: [
          {
            url: `file://${fixture}`,
            bodyPath: fixture,
            bytesSha256: sourceSha256,
            loaded: true,
          },
        ],
      },
      fontProvenance: [
        {
          family: "sans",
          scope: "capture",
          source: "system",
          filePath: fontPath,
          fileSha256: sha256File(fontPath),
          faceIndex: 0,
          selected: true,
          fileBytesVerified: true,
        },
      ],
    };
    const expected = {
      conditionId: report.conditionId,
      caseId: report.caseId,
      state: report.boundaryState,
      operationKind: report.operationKind,
      nonce: report.executionNonce,
      sourceSha256,
      binarySha256: report.binarySha256,
      viewport: report.viewport,
      targetSelector: "#state",
    };
    const writeAndRead = (value: typeof report): void => {
      fs.writeFileSync(reportPath, JSON.stringify(value));
      expect(readKrrDynamicCapture(reportPath, expected).status).toBe("observed");
    };
    try {
      expect(operationEvidence.state.before).toEqual(operationEvidence.state.after);
      writeAndRead(report);

      const transitionEvidence = {
        ...operationEvidence,
        state: { before: [], after: expectedLog },
        steps: [],
        delivery: "mutation-observer",
      };
      writeAndRead({ ...report, operationEvidence: transitionEvidence });

      for (const mutation of [
        {
          operationEvidence: {
            ...operationEvidence,
            steps: [{ ...operationEvidence.steps[0], kind: "arbitrary-settled-state" }],
          },
        },
        {
          operationEvidence: {
            ...operationEvidence,
            steps: [{ ...operationEvidence.steps[0], stateBefore: ["takeRecords"] }],
          },
        },
        {
          operationEvidence: {
            ...operationEvidence,
            steps: [{ ...operationEvidence.steps[0], stateAfter: ["takeRecords"] }],
          },
        },
        {
          operationEvidence: {
            ...operationEvidence,
            steps: [operationEvidence.steps[0], operationEvidence.steps[0]],
          },
        },
        {
          operationEvidence: {
            ...operationEvidence,
            delivery: undefined,
          },
        },
        {
          operationEvidence: {
            ...operationEvidence,
            status: "unknown",
          },
        },
        {
          operationEvidence: Object.fromEntries(
            Object.entries(operationEvidence).filter(([key]) => key !== "status"),
          ),
        },
        {
          operationEvidence: {
            ...operationEvidence,
            state: { before: [], after: [] },
          },
        },
        { state: ["takeRecords"] },
        {
          operationEvidence: {
            ...operationEvidence,
            delivery: "session-load-settled",
            extra: "untyped evidence",
          },
        },
      ]) {
        fs.writeFileSync(reportPath, JSON.stringify({ ...report, ...mutation }));
        expect(() => readKrrDynamicCapture(reportPath, expected)).toThrow();
      }
      const unknownReason = "A separate capture proof is unavailable";
      const unknownReport = expectUnknownCapturePreservesObservedEvidence({
        reportPath,
        report,
        expected,
        operationEvidence,
        operationKind: "script-await-mutation-delivery",
        unknownReason,
      });

      fs.writeFileSync(
        reportPath,
        JSON.stringify({
          ...unknownReport,
          observedOperationEvidence: {
            ...operationEvidence,
            steps: [
              {
                ...operationEvidence.steps[0],
                stateAfter: ["takeRecords"],
              },
            ],
          },
        }),
      );
      expect(() => readKrrDynamicCapture(reportPath, expected)).toThrow();

      fs.writeFileSync(
        reportPath,
        JSON.stringify({
          ...report,
          operationEvidence: { ...operationEvidence, delivery: undefined },
        }),
      );
      expect(() => readKrrDynamicCapture(reportPath, expected)).toThrow();
    } finally {
      fs.rmSync(directory, { recursive: true, force: true });
    }
  });

  test("accepts only resource state settled to the pinned fixture terminal value", () => {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), "dynamic-resource-settled-"));
    const fixture = path.resolve("tests/fixtures/html/aspects/script-resources.html");
    const reportPath = path.join(directory, "catalog-report.json");
    const expectedState = readExpectedState(fixture);
    const sourceSha256 = sha256File(fixture);
    const operationEvidence = {
      schema: "krr-html84-operation-evidence-v1",
      kind: "script-await-resource-font",
      sourceSha256Before: sourceSha256,
      sourceSha256After: sourceSha256,
      preFrame: { generation: 1, contentHeight: 900, scrollY: 0 },
      postFrame: { generation: 1, contentHeight: 900, scrollY: 0 },
      target: { selector: "#state", nodeId: 2, visible: true },
      state: { before: expectedState, after: expectedState },
      steps: [
        {
          kind: "session-load-settled-resource-state",
          stateBefore: expectedState,
          stateAfter: expectedState,
        },
      ],
      delivery: "session-load-settled",
      status: "observed",
    };
    const { report, expected } = strictCatalogCaptureReport({
      directory,
      fixture,
      caseId: "resources",
      boundaryState: "expected-state",
      operationKind: "script-await-resource-font",
      targetSelector: "#state",
      state: expectedState,
      operationEvidence,
    });
    const write = (value: Record<string, JsonValue>): void => {
      fs.writeFileSync(reportPath, JSON.stringify(value));
    };
    try {
      write(report);
      expect(readKrrDynamicCapture(reportPath, expected).status).toBe("observed");
      const wrongBefore = JSON.parse(JSON.stringify(report)) as Record<string, JsonValue>;
      (
        (wrongBefore.operationEvidence as Record<string, JsonValue>).state as Record<
          string,
          JsonValue
        >
      ).before = {
        state: "waiting",
        fonts: "ready",
      };
      const wrongStep = JSON.parse(JSON.stringify(report)) as Record<string, JsonValue>;
      (
        (
          (wrongStep.operationEvidence as Record<string, JsonValue>).steps as Record<
            string,
            JsonValue
          >[]
        )[0] as Record<string, JsonValue>
      ).kind = "arbitrary-settled-state";
      const wrongSource = { ...report, htmlSha256: "c".repeat(64) };
      for (const malformed of [wrongBefore, wrongStep, wrongSource]) {
        write(malformed);
        expect(() => readKrrDynamicCapture(reportPath, expected)).toThrow();
      }
      for (const status of [undefined, "unknown"]) {
        const unobserved = JSON.parse(JSON.stringify(report)) as Record<string, JsonValue>;
        const evidence = unobserved.operationEvidence as Record<string, JsonValue>;
        if (status === undefined) delete evidence.status;
        else evidence.status = status;
        write(unobserved);
        expect(() => readKrrDynamicCapture(reportPath, expected)).toThrow();
      }
    } finally {
      fs.rmSync(directory, { recursive: true, force: true });
    }
  });

  test("does not allow equal fresh-load state for timer operations", () => {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), "dynamic-timer-equal-state-"));
    const fixture = path.resolve("tests/fixtures/html/aspects/script-async.html");
    const reportPath = path.join(directory, "catalog-report.json");
    const state = readExpectedState(fixture);
    const sourceSha256 = sha256File(fixture);
    const operationEvidence = {
      schema: "krr-html84-operation-evidence-v1",
      kind: "script-await-timer-microtask",
      sourceSha256Before: sourceSha256,
      sourceSha256After: sourceSha256,
      preFrame: { generation: 1, contentHeight: 900, scrollY: 0 },
      postFrame: { generation: 1, contentHeight: 900, scrollY: 0 },
      target: { selector: "#state", nodeId: 2, visible: true },
      state: { before: state, after: state },
      steps: [],
      delivery: "session-load-settled",
    };
    const { report, expected } = strictCatalogCaptureReport({
      directory,
      fixture,
      caseId: "async",
      boundaryState: "expected-state",
      operationKind: "script-await-timer-microtask",
      targetSelector: "#state",
      state,
      operationEvidence,
    });
    try {
      fs.writeFileSync(reportPath, JSON.stringify(report));
      expect(() => readKrrDynamicCapture(reportPath, expected)).toThrow();
    } finally {
      fs.rmSync(directory, { recursive: true, force: true });
    }
  });

  test("accepts only exact startup-settled evidence for the pinned async fixture", () => {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), "dynamic-timer-startup-settled-"));
    const fixture = path.resolve("tests/fixtures/html/aspects/script-async.html");
    const reportPath = path.join(directory, "catalog-report.json");
    const state = readExpectedState(fixture);
    const sourceSha256 = sha256File(fixture);
    const operationEvidence = {
      schema: "krr-html84-operation-evidence-v1",
      kind: "script-await-timer-microtask",
      status: "observed",
      sourceSha256Before: sourceSha256,
      sourceSha256After: sourceSha256,
      preFrame: { generation: 1, contentHeight: 900, scrollY: 0 },
      postFrame: { generation: 1, contentHeight: 900, scrollY: 0 },
      target: { selector: "#state", nodeId: 2, visible: true },
      state: { before: state, after: state },
      steps: [
        {
          kind: "host-time-advance",
          elapsedMs: 0,
          timerRan: false,
          microtaskCheckpoint: "runtime-advance-timers",
          stateBefore: state,
          stateAfter: state,
          fixtureStateVerified: true,
        },
      ],
      delivery: "session-load-settled",
    };
    const { report, expected } = strictCatalogCaptureReport({
      directory,
      fixture,
      caseId: "async",
      boundaryState: "expected-state",
      operationKind: "script-await-timer-microtask",
      targetSelector: "#state",
      state,
      operationEvidence,
    });
    const malformed = (mutate: (value: Record<string, JsonValue>) => void) => {
      const value = JSON.parse(JSON.stringify(report)) as Record<string, JsonValue>;
      mutate(value);
      return value;
    };
    const evidenceOf = (value: Record<string, JsonValue>) =>
      value.operationEvidence as Record<string, JsonValue>;
    const stepOf = (value: Record<string, JsonValue>) =>
      (evidenceOf(value).steps as Record<string, JsonValue>[])[0] as Record<string, JsonValue>;
    const invalidReports = [
      malformed((value) => {
        evidenceOf(value).sourceSha256After = "0".repeat(64);
      }),
      malformed((value) => {
        evidenceOf(value).delivery = "timer-and-microtask";
      }),
      malformed((value) => {
        stepOf(value).elapsedMs = 1;
      }),
      malformed((value) => {
        stepOf(value).timerRan = true;
      }),
      malformed((value) => {
        stepOf(value).fixtureStateVerified = false;
      }),
      malformed((value) => {
        const wrongState = "microtask>ready>other";
        value.state = wrongState;
        const evidence = evidenceOf(value);
        evidence.state = { before: wrongState, after: wrongState };
        const step = stepOf(value);
        step.stateBefore = wrongState;
        step.stateAfter = wrongState;
      }),
    ];
    try {
      fs.writeFileSync(reportPath, JSON.stringify(report));
      expect(readKrrDynamicCapture(reportPath, expected).status).toBe("observed");
      for (const value of invalidReports) {
        fs.writeFileSync(reportPath, JSON.stringify(value));
        expect(() => readKrrDynamicCapture(reportPath, expected)).toThrow();
      }
    } finally {
      fs.rmSync(directory, { recursive: true, force: true });
    }
  });

  test("accepts optional actual content height on every generated event pointer frame", () => {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), "dynamic-event-content-height-"));
    const fixture = path.resolve("tests/fixtures/html/aspects/script-events.html");
    const reportPath = path.join(directory, "catalog-report.json");
    const expectedState = readExpectedState(fixture);
    const { evidence } = scriptRealEventsEvidence(fixture, expectedState, true);
    const { report, expected } = strictCatalogCaptureReport({
      directory,
      fixture,
      caseId: "events",
      boundaryState: "expected-state",
      operationKind: "script-real-events",
      targetSelector: "#field",
      state: expectedState,
      operationEvidence: evidence,
    });
    const steps = evidence.steps as Record<string, JsonValue>[];
    const pointerSteps = [...steps.slice(0, 3), ...steps.slice(6, 9)];
    const pointerFrames = pointerSteps.flatMap((step) => [step.preFrame, step.postFrame]);
    try {
      fs.writeFileSync(reportPath, JSON.stringify(report));
      expect(readKrrDynamicCapture(reportPath, expected).status).toBe("observed");
      expect(pointerFrames.length).toBe(12);
      for (const frame of pointerFrames) {
        if (typeof frame !== "object" || frame === null || Array.isArray(frame))
          throw new Error("Generated pointer frame is not an object");
        expect(frame.contentHeight).toBe(900);
      }
      for (const invalidHeight of [-1, "900", null]) {
        const mutatedEvidence = JSON.parse(JSON.stringify(evidence)) as Record<string, JsonValue>;
        const mutatedSteps = mutatedEvidence.steps as Record<string, JsonValue>[];
        const firstStep = mutatedSteps[0];
        if (firstStep === undefined) throw new Error("Generated trace has no pointer step");
        const firstFrame = firstStep.preFrame as Record<string, JsonValue>;
        firstFrame.contentHeight = invalidHeight;
        fs.writeFileSync(
          reportPath,
          JSON.stringify({ ...report, operationEvidence: mutatedEvidence }),
        );
        expect(() => readKrrDynamicCapture(reportPath, expected)).toThrow();
      }
      const extraKeyEvidence = JSON.parse(JSON.stringify(evidence)) as Record<string, JsonValue>;
      const extraKeySteps = extraKeyEvidence.steps as Record<string, JsonValue>[];
      const firstStep = extraKeySteps[0];
      if (firstStep === undefined) throw new Error("Generated trace has no pointer step");
      (firstStep.preFrame as Record<string, JsonValue>).unexpected = 1;
      fs.writeFileSync(
        reportPath,
        JSON.stringify({ ...report, operationEvidence: extraKeyEvidence }),
      );
      expect(() => readKrrDynamicCapture(reportPath, expected)).toThrow();
    } finally {
      fs.rmSync(directory, { recursive: true, force: true });
    }
  });

  test("strictly binds real script events to nine ordered input dispatches", () => {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), "dynamic-real-events-"));
    const fixture = path.resolve("tests/fixtures/html/aspects/script-events.html");
    const reportPath = path.join(directory, "catalog-report.json");
    const expectedState = readExpectedState(fixture);
    const { initial, evidence: operationEvidence } = scriptRealEventsEvidence(
      fixture,
      expectedState,
      false,
    );
    const { report, expected } = strictCatalogCaptureReport({
      directory,
      fixture,
      caseId: "events",
      boundaryState: "expected-state",
      operationKind: "script-real-events",
      targetSelector: "#field",
      state: expectedState,
      operationEvidence,
    });
    const writeAndRead = (value: Record<string, JsonValue>): void => {
      fs.writeFileSync(reportPath, JSON.stringify(value));
      expect(readKrrDynamicCapture(reportPath, expected).status).toBe("observed");
    };
    const eventRecords = operationEvidence.events as Record<string, JsonValue>[];
    const stepRecords = operationEvidence.steps as Record<string, JsonValue>[];
    const eventAt = (index: number): Record<string, JsonValue> => {
      const event = eventRecords[index];
      if (event === undefined) throw new Error(`missing event fixture at ${index}`);
      return event;
    };
    const stepAt = (index: number): Record<string, JsonValue> => {
      const step = stepRecords[index];
      if (step === undefined) throw new Error(`missing step fixture at ${index}`);
      return step;
    };
    try {
      writeAndRead(report);
      const unknownReason = "The independent resource inventory is incomplete";
      const unknownReport = expectUnknownCapturePreservesObservedEvidence({
        reportPath,
        report: {
          ...report,
          resourceProvenance: { inventoryComplete: false, requests: [] },
          fontProvenance: [],
        },
        expected,
        operationEvidence,
        operationKind: "script-real-events",
        unknownReason,
      });

      fs.writeFileSync(
        reportPath,
        JSON.stringify({
          ...unknownReport,
          observedOperationEvidence: {
            ...operationEvidence,
            events: eventRecords.map((event) =>
              event.type === "keydown" ? { ...event, targetNodeId: 99 } : event,
            ),
          },
        }),
      );
      expect(() => readKrrDynamicCapture(reportPath, expected)).toThrow();

      const invalidCases: Record<string, JsonValue>[] = [
        ...invalidOperationStatusEvidence(operationEvidence).map((invalidStatus) => ({
          operationEvidence: invalidStatus,
        })),
        ...[-1, "900", null].map((contentHeight) => ({
          operationEvidence: {
            ...operationEvidence,
            steps: stepRecords.map((step, index) =>
              index === 0
                ? {
                    ...step,
                    preFrame: { ...(step.preFrame as Record<string, JsonValue>), contentHeight },
                  }
                : step,
            ),
          },
        })),
        {
          operationEvidence: {
            ...operationEvidence,
            steps: stepRecords.map((step, index) =>
              index === 0
                ? {
                    ...step,
                    preFrame: { ...(step.preFrame as Record<string, JsonValue>), unexpected: 1 },
                  }
                : step,
            ),
          },
        },
        {
          operationEvidence: {
            ...operationEvidence,
            events: [eventAt(0), eventAt(2), eventAt(1), ...eventRecords.slice(3)],
          },
        },
        {
          operationEvidence: {
            ...operationEvidence,
            events: eventRecords.filter((event) => event.type !== "keyup"),
          },
        },
        {
          operationEvidence: {
            ...operationEvidence,
            events: eventRecords.map((event) =>
              event.type === "keydown" ? { ...event, key: "Control" } : event,
            ),
          },
        },
        {
          operationEvidence: {
            ...operationEvidence,
            events: eventRecords.map((event) =>
              event.type === "input" ? { ...event, targetNodeId: 99 } : event,
            ),
          },
        },
        {
          operationEvidence: {
            ...operationEvidence,
            events: eventRecords.map((event) =>
              event.type === "submit" ? { ...event, stateAfter: initial } : event,
            ),
          },
        },
        {
          operationEvidence: {
            ...operationEvidence,
            steps: [...stepRecords.slice(0, 3), stepAt(4), stepAt(3), ...stepRecords.slice(5)],
          },
        },
        {
          operationEvidence: {
            ...operationEvidence,
            steps: stepRecords.map((step) =>
              step.kind === "text" ? { ...step, text: "wrong" } : step,
            ),
          },
        },
        {
          operationEvidence: {
            ...operationEvidence,
            steps: stepRecords.map((step, index) =>
              index === 0 ? { ...step, targetNodeId: 99 } : step,
            ),
          },
        },
      ];
      for (const mutation of invalidCases) {
        fs.writeFileSync(reportPath, JSON.stringify({ ...report, ...mutation }));
        expect(() => readKrrDynamicCapture(reportPath, expected)).toThrow();
      }
    } finally {
      fs.rmSync(directory, { recursive: true, force: true });
    }
  });

  test("validates each all-TOC click, frame, hash and distinct capture file", () => {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), "dynamic-all-toc-"));
    const fixture = path.resolve("tests/fixtures/html/aspects/navigation.html");
    const reportPath = path.join(directory, "catalog-report.json");
    const samePngBytes = "valid identical destination pixels";
    const pointerTrace = (targetNodeId: number, base: number, finalScroll: number): JsonValue[] =>
      ["move", "down", "up"].map((kind, index) => ({
        kind,
        targetNodeId,
        x: base,
        y: base,
        preFrame: { generation: 1 + index, scrollY: index },
        postFrame: {
          generation: 2 + index,
          scrollY: index === 2 ? finalScroll : index + 1,
        },
      }));
    const destinations = ["#a", "#b", "#c"];
    const links = destinations.map((destination, index) => {
      const capturePath = path.join(directory, `toc-${index}.png`);
      fs.writeFileSync(capturePath, samePngBytes);
      const scrollY = 30 + index;
      const trace = pointerTrace(10 + index, 20 + index * 10, scrollY);
      return {
        destination,
        hash: destination,
        scrollY,
        capturePath,
        captureSha256: sha256File(capturePath),
        targetVisible: true,
        hashBefore: index === 0 ? "" : (destinations[index - 1] ?? ""),
        hashAfter: destination,
        frame: { generation: 4, contentHeight: 2000, scrollY },
        pointerTrace: trace,
        sourceSha256: sha256File(fixture),
      };
    });
    const state = { tag: "A", text: "目次" };
    const operationEvidence: Record<string, JsonValue> = {
      schema: "krr-html84-operation-evidence-v1",
      kind: "navigation-all-pointer-clicks",
      status: "observed",
      sourceSha256Before: sha256File(fixture),
      sourceSha256After: sha256File(fixture),
      preFrame: { generation: 1, scrollY: 0 },
      postFrame: { generation: 20, scrollY: 33 },
      target: {
        selector: ".sidebar .toc-item[href^='#']",
        nodeId: 5,
        visible: true,
      },
      state: { before: state, after: state },
      steps: links.flatMap((link) => link.pointerTrace),
      delivery: "session-load",
      links,
    };
    const { report, expected } = strictCatalogCaptureReport({
      directory,
      fixture,
      caseId: "navigation",
      boundaryState: "all-toc-links",
      operationKind: "navigation-all-pointer-clicks",
      targetSelector: ".sidebar .toc-item[href^='#']",
      state,
      operationEvidence,
    });
    const writeAndRead = (value: Record<string, JsonValue>): void => {
      fs.writeFileSync(reportPath, JSON.stringify(value));
      expect(readKrrDynamicCapture(reportPath, expected).status).toBe("observed");
    };
    const clone = (): Record<string, JsonValue> => JSON.parse(JSON.stringify(report));
    const cloneLinks = (): Record<string, JsonValue>[] =>
      JSON.parse(JSON.stringify(links)) as Record<string, JsonValue>[];
    const at = (items: Record<string, JsonValue>[], index: number): Record<string, JsonValue> => {
      const item = items[index];
      if (item === undefined) throw new Error(`missing link fixture at ${index}`);
      return item;
    };
    try {
      expect(new Set(links.map((link) => link.captureSha256)).size).toBe(1);
      writeAndRead(report);
      const unknownReason = "The independent resource inventory is incomplete";
      const unknownReport = unknownCaptureWithObservedEvidence({
        report: clone(),
        operationEvidence,
        operationKind: "navigation-all-pointer-clicks",
        unknownReason,
      });
      unknownReport.resourceProvenance = { inventoryComplete: false, requests: [] };
      unknownReport.fontProvenance = [];
      fs.writeFileSync(reportPath, JSON.stringify(unknownReport));
      const observedUnknown = readKrrDynamicCapture(reportPath, expected);
      expect(observedUnknown.status).toBe("unknown");
      if (observedUnknown.status === "unknown")
        expect(observedUnknown.observedOperationEvidence).toEqual(operationEvidence);
      const wrongTarget = JSON.parse(JSON.stringify(unknownReport)) as Record<string, JsonValue>;
      wrongTarget.observedOperationEvidence = {
        ...operationEvidence,
        target: { selector: "#wrong", nodeId: 5, visible: true },
      };
      fs.writeFileSync(reportPath, JSON.stringify(wrongTarget));
      expect(() => readKrrDynamicCapture(reportPath, expected)).toThrow();
      const invalid = [
        ...invalidOperationStatusEvidence(operationEvidence).map(
          (invalidStatus) => (candidate: Record<string, JsonValue>) => {
            candidate.operationEvidence = invalidStatus;
          },
        ),
        (candidate: Record<string, JsonValue>) => {
          (candidate.operationEvidence as Record<string, JsonValue>).links = links.slice(0, 2);
        },
        (candidate: Record<string, JsonValue>) => {
          (candidate.operationEvidence as Record<string, JsonValue>).links = [...links].reverse();
        },
        (candidate: Record<string, JsonValue>) => {
          const next = cloneLinks();
          const pointerTrace = at(next, 0).pointerTrace;
          if (pointerTrace === undefined) throw new Error("missing link pointer trace");
          at(next, 1).pointerTrace = pointerTrace;
          (candidate.operationEvidence as Record<string, JsonValue>).links = next;
        },
        (candidate: Record<string, JsonValue>) => {
          const next = cloneLinks();
          const capturePath = at(next, 0).capturePath;
          if (capturePath === undefined) throw new Error("missing link capture path");
          at(next, 1).capturePath = capturePath;
          (candidate.operationEvidence as Record<string, JsonValue>).links = next;
        },
        (candidate: Record<string, JsonValue>) => {
          const next = cloneLinks();
          at(next, 0).hashAfter = "#wrong";
          (candidate.operationEvidence as Record<string, JsonValue>).links = next;
        },
        (candidate: Record<string, JsonValue>) => {
          const next = cloneLinks();
          at(next, 1).captureSha256 = "f".repeat(64);
          (candidate.operationEvidence as Record<string, JsonValue>).links = next;
        },
        (candidate: Record<string, JsonValue>) => {
          const next = cloneLinks();
          at(next, 2).frame = { generation: 0, contentHeight: 2000, scrollY: 32 };
          (candidate.operationEvidence as Record<string, JsonValue>).links = next;
        },
        (candidate: Record<string, JsonValue>) => {
          const next = cloneLinks();
          at(next, 0).sourceSha256 = "f".repeat(64);
          (candidate.operationEvidence as Record<string, JsonValue>).links = next;
        },
        (candidate: Record<string, JsonValue>) => {
          (candidate.operationEvidence as Record<string, JsonValue>).steps = [];
        },
      ];
      for (const mutate of invalid) {
        const candidate = clone();
        mutate(candidate);
        fs.writeFileSync(reportPath, JSON.stringify(candidate));
        expect(() => readKrrDynamicCapture(reportPath, expected)).toThrow();
      }
    } finally {
      fs.rmSync(directory, { recursive: true, force: true });
    }
  });

  test("requires exact finite identity, actual operation, frame, PNG and resource/font byte evidence", async () => {
    await withDynamicFixture((fixture, directory) => {
      const capturePath = path.join(directory, "capture.png");
      const reportPath = path.join(directory, "catalog-report.json");
      fs.writeFileSync(capturePath, "independent unit artifact");
      const fontPath = path.join(directory, "fixture-font.bin");
      fs.writeFileSync(fontPath, "actual unit fixture font bytes");
      const sha = (file: string): string => sha256File(file);
      const sha256 = sha;
      const sourceSha256 = sha256(fixture);
      const report = {
        schema: "krr-html84-raw-condition-v1",
        status: "observed",
        conditionId: "dynamic-dom/expected-state/1280x900@1",
        caseId: "dynamic-dom",
        boundaryState: "expected-state",
        operationKind: "script-load-observe",
        executionNonce: "catalog-nonce",
        htmlSha256: sourceSha256,
        binarySha256: "b".repeat(64),
        capturePath,
        captureSha256: sha256(capturePath),
        viewport: { width: 1280, height: 900, scale: 1 },
        state: { ready: true },
        geometry: [{ nodeId: 1, x: 1, y: 1, width: 10, height: 10 }],
        operationEvidence: {
          schema: "krr-html84-operation-evidence-v1",
          kind: "script-load-observe",
          delivery: "session-load",
          sourceSha256Before: sourceSha256,
          sourceSha256After: sourceSha256,
          preFrame: { generation: 1, scrollY: 0 },
          postFrame: { generation: 1, scrollY: 0 },
          target: { selector: "#state", nodeId: 1, visible: true },
          state: { before: { ready: true }, after: { ready: true } },
        },
        frame: { generation: 1, contentHeight: 900, scrollY: 0 },
        targetVisible: true,
        resourceProvenance: {
          inventoryComplete: true,
          requests: [
            {
              url: `file://${fixture}`,
              bodyPath: fixture,
              bytesSha256: sourceSha256,
              loaded: true,
            },
          ],
        },
        fontProvenance: [
          {
            family: "sans",
            scope: "node",
            source: "system",
            filePath: fontPath,
            fileSha256: sha(fontPath),
            faceIndex: 0,
            selectedNodeId: 1,
            selected: true,
            fileBytesVerified: true,
          },
        ],
      };
      fs.writeFileSync(reportPath, JSON.stringify(report));
      const expected = {
        conditionId: report.conditionId,
        caseId: report.caseId,
        state: report.boundaryState,
        operationKind: report.operationKind,
        nonce: report.executionNonce,
        sourceSha256,
        binarySha256: report.binarySha256,
        viewport: report.viewport,
        targetSelector: "#state",
      };
      expect(readKrrDynamicCapture(reportPath, expected).stateValue).toEqual({ ready: true });
      for (const mutation of [
        { conditionId: "other" },
        { operationEvidence: { kind: report.operationKind, executed: true } },
        { frame: { generation: 0, contentHeight: 900, scrollY: 0 } },
        { fontProvenance: [] },
        {
          fontProvenance: [{ ...(report.fontProvenance[0] ?? {}), faceIndex: undefined }],
        },
        { fontProvenance: [{ ...(report.fontProvenance[0] ?? {}), faceIndex: -1 }] },
        { fontProvenance: [{ ...(report.fontProvenance[0] ?? {}), faceIndex: 0.5 }] },
        { resourceProvenance: { inventoryComplete: false, requests: [] } },
        { targetVisible: false },
      ]) {
        fs.writeFileSync(reportPath, JSON.stringify({ ...report, ...mutation }));
        expect(() => readKrrDynamicCapture(reportPath, expected)).toThrow();
      }

      const unknownReason = "The runtime does not expose mutation delivery provenance";
      const unknown = {
        ...report,
        status: "unknown",
        unknownReason,
        targetVisible: false,
        geometry: [],
        operationEvidence: {
          schema: "krr-html84-operation-evidence-v1",
          kind: report.operationKind,
          status: "unknown",
          reason: unknownReason,
        },
        resourceProvenance: { inventoryComplete: false, requests: [] },
        fontProvenance: [],
      };
      fs.writeFileSync(reportPath, JSON.stringify(unknown));
      const observedUnknown = readKrrDynamicCapture(reportPath, expected);
      expect(observedUnknown.status).toBe("unknown");
      if (observedUnknown.status === "unknown") {
        expect(observedUnknown.unknownReason).toBe(unknownReason);
        expect(observedUnknown.geometry).toEqual([]);
        expect(observedUnknown.targetVisible).toBe(false);
        expect(observedUnknown.resourceProvenance).toEqual({
          inventoryComplete: false,
          requests: [],
        });
        expect(observedUnknown.fontProvenance).toEqual([]);
      }
      for (const mutation of [
        { unknownReason: "" },
        { operationEvidence: { ...unknown.operationEvidence, reason: "different" } },
        { htmlSha256: "e".repeat(64) },
      ]) {
        fs.writeFileSync(reportPath, JSON.stringify({ ...unknown, ...mutation }));
        expect(() => readKrrDynamicCapture(reportPath, expected)).toThrow();
      }
    });
  });

  test("binds response bytes to an unknown source-pinned condition without hiding failures", async () => {
    await withDynamicFixture(async (_original, directory) => {
      const sourceSha256 = sha256File(_original);
      const successfulUrl = "file:///fixture/resource.js";
      const actualBytes = Buffer.from("observed response body bytes");
      const collectScenario = async (
        conditionId: string,
        request: Parameters<typeof emitChromeResource>[1],
      ): Promise<ChromeResourceProvenance> => {
        const page = new EventEmitter();
        const collector = new ChromeResourceBodyCollector(
          page as unknown as Page,
          conditionId,
          sourceSha256,
          path.join(directory, `${conditionId.replaceAll("/", "-")}.png`),
        );
        emitChromeResource(page, request);
        return collector.finish([request.url]);
      };
      const conditionId = "resources/success/1280x900@1";
      const resources = await collectScenario(conditionId, {
        url: successfulUrl,
        body: actualBytes,
      });
      expect(resources.inventoryComplete).toBe(true);
      expect(resources.requests).toHaveLength(1);
      const [resource] = resources.requests;
      expect(resource?.url).toBe(successfulUrl);
      expect(resource && fs.readFileSync(resource.bodyPath)).toEqual(actualBytes);
      expect(resource && sha256File(resource.bodyPath)).toBe(resource?.bytesSha256);
      expect(resource?.bodyPath).toContain(
        `${conditionId.replace(/[^a-zA-Z0-9.-]+/g, "_")}-resources-${sourceSha256}`,
      );

      const collisionCondition = "resources/output-collision/1280x900@1";
      const collisionDirectory = path.join(
        directory,
        `${collisionCondition.replace(/[^a-zA-Z0-9.-]+/g, "_")}-resources-${sourceSha256}`,
      );
      fs.mkdirSync(collisionDirectory);
      const bodySha256 = createHash("sha256").update(actualBytes).digest("hex");
      const collisionPath = path.join(collisionDirectory, `0000-${bodySha256}.body`);
      fs.writeFileSync(collisionPath, "preserve existing bytes");
      await expect(
        collectScenario(collisionCondition, { url: successfulUrl, body: actualBytes }),
      ).rejects.toThrow();
      expect(fs.readFileSync(collisionPath, "utf8")).toBe("preserve existing bytes");

      const incompleteScenarios: Array<{
        url: string;
        status?: number;
        bodyFailure?: boolean;
        failed?: boolean;
        pending?: boolean;
      }> = [
        { url: "file:///fixture/missing.svg", status: 404 },
        { url: "file:///fixture/body-unavailable.css", bodyFailure: true },
        { url: "file:///fixture/pending.png", pending: true },
        { url: "file:///fixture/failed.js", failed: true },
      ];
      let incompleteResources: ChromeResourceProvenance | undefined;
      for (const [index, scenario] of incompleteScenarios.entries()) {
        const observed = await collectScenario(`resources/failure-${index}/1280x900@1`, scenario);
        expect(observed.inventoryComplete).toBe(false);
        expect(observed.requests).toHaveLength(0);
        if (scenario.bodyFailure) incompleteResources = observed;
      }

      const { report, expected } = strictCatalogCaptureReport({
        directory,
        fixture: _original,
        caseId: "resources",
        boundaryState: "expected-state",
        operationKind: "script-load-observe",
        targetSelector: "#state",
        state: { loaded: true },
        operationEvidence: { kind: "script-load-observe", executed: true },
      });
      if (!incompleteResources) throw new Error("body failure scenario was not observed");
      report.resourceProvenance = incompleteResources as unknown as JsonValue;
      report.fontProvenance = [];
      const unknownReason = "Chrome selected font identity remains unknown";
      const unknown = {
        ...report,
        status: "unknown",
        unknownReason,
        operationEvidence: {
          schema: "krr-html84-operation-evidence-v1",
          kind: "script-load-observe",
          status: "unknown",
          reason: unknownReason,
        },
      };
      const reportPath = path.join(directory, "resource-condition-unknown.json");
      fs.writeFileSync(reportPath, JSON.stringify(unknown));
      const parsed = readKrrDynamicCapture(reportPath, expected);
      expect(parsed.status).toBe("unknown");
      if (parsed.status === "unknown") {
        expect(parsed.htmlSha256).toBe(sourceSha256);
        expect(parsed.conditionId).toBe(expected.conditionId);
        expect(JSON.stringify(parsed.resourceProvenance)).toBe(JSON.stringify(incompleteResources));
        expect(parsed.fontProvenance).toEqual([]);
      }
    });
  });

  test("pins the same HTML identity for both captures and result evidence", async () => {
    await withDynamicFixture(async (fixture) => {
      const expectedSha = sha256File(fixture);
      const expected = readExpectedState(fixture);
      const captureOrder: string[] = [];
      const pair = await captureDynamicStatePair(
        fixture,
        async () => {
          captureOrder.push("chrome");
          return expected;
        },
        (sourceSha) => {
          captureOrder.push("krr");
          expect(sourceSha).toBe(expectedSha);
          return expected;
        },
      );
      expect(captureOrder).toEqual(["chrome", "krr"]);
      expect(pair).toEqual({
        sourceSha256: expectedSha,
        chromeState: expected,
        krrState: expected,
      });
    });
  });

  for (const mutationPhase of ["chrome", "krr"] as const) {
    test(`rejects a non-painted comment mutation during ${mutationPhase} capture`, async () => {
      await withDynamicFixture(async (fixture) => {
        const originalSha = sha256File(fixture);
        const expected = readExpectedState(fixture);
        let krrCaptures = 0;
        const mutate = (): JsonValue => {
          fs.appendFileSync(fixture, SOURCE_COMMENT);
          expect(sha256File(fixture)).not.toBe(originalSha);
          expect(readExpectedState(fixture)).toEqual(expected);
          return expected;
        };
        await expect(
          captureDynamicStatePair(
            fixture,
            async () => (mutationPhase === "chrome" ? mutate() : expected),
            () => {
              krrCaptures += 1;
              return mutationPhase === "krr" ? mutate() : expected;
            },
          ),
        ).rejects.toThrow("HTML input SHA changed");
        expect(krrCaptures).toBe(mutationPhase === "chrome" ? 0 : 1);
      });
    });
  }

  test("rejects failed Chrome capture without starting KRR", async () => {
    await withDynamicFixture(async (fixture) => {
      let krrStarted = false;
      await expect(
        captureDynamicStatePair(
          fixture,
          async () => {
            throw new Error("Chrome capture failed");
          },
          () => {
            krrStarted = true;
            return null;
          },
        ),
      ).rejects.toThrow("Chrome capture failed");
      expect(krrStarted).toBe(false);
    });
  });

  test("rejects changed pinned input before touching the KRR executable", async () => {
    await withDynamicFixture((fixture, directory) => {
      const pinnedSha = sha256File(fixture);
      fs.appendFileSync(fixture, SOURCE_COMMENT);
      expect(() =>
        captureKrrState(
          path.join(directory, "missing-probe"),
          fixture,
          "null",
          path.join(directory, "unused-report.json"),
          "dynamic-dom",
          { width: 1280, height: 900 },
          pinnedSha,
        ),
      ).toThrow("HTML input SHA changed");
    });
  });

  test("rejects a producer report hash differing from the pinned HTML", async () => {
    await withDynamicFixture((fixture, directory) => {
      const reportPath = path.join(directory, "report.json");
      const capturePath = path.join(directory, "capture.png");
      const sourceSha = sha256File(fixture);
      const state = readExpectedState(fixture);
      const report = {
        executionNonce: "capture-nonce",
        htmlSha256: sourceSha,
        binarySha256: "b".repeat(64),
        state,
        capturePath,
      };
      fs.writeFileSync(capturePath, "unit capture-presence artifact");
      fs.writeFileSync(reportPath, JSON.stringify(report));
      expect(
        readKrrDynamicState(reportPath, report.executionNonce, sourceSha, report.binarySha256),
      ).toEqual(state);
      fs.appendFileSync(fixture, SOURCE_COMMENT);
      fs.writeFileSync(reportPath, JSON.stringify({ ...report, htmlSha256: sha256File(fixture) }));
      expect(() =>
        readKrrDynamicState(reportPath, report.executionNonce, sourceSha, report.binarySha256),
      ).toThrow("Stale dynamic report");
    });
  });
});

describe("dynamic HTML fidelity phases", () => {
  test("accepts only immutable final and interim thresholds before browser capture", async () => {
    await expect(runDynamicFidelity("", "", 90)).rejects.toThrow(/final.*99/);
    await expect(runDynamicFidelity("", "", 99, { phase: "interim" })).rejects.toThrow(
      /interim.*90/,
    );
    await expect(
      runDynamicFidelity("", "", 90, { phase: "unrecognized" } as never),
    ).rejects.toThrow(/unsupported.*phase/);
  });

  test("final feature consumer rejects an interim envelope and accepts the final array", () => {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), "dynamic-fidelity-phase-"));
    try {
      const catalog = loadFeatureCatalog();
      const toResult = (entry: (typeof catalog.visualAspects)[number]): FeatureCaseResult => ({
        caseId: entry.id,
        status: "passed",
        features: Object.fromEntries(entry.features.map((feature) => [feature, "passed"])),
      });
      const visualResults = catalog.visualAspects.map(toResult);
      const scriptResults = catalog.scriptCases.map(toResult);
      const visualPath = path.join(directory, "visual-results.json");
      const finalScriptPath = path.join(directory, "final-script-results.json");
      const interimScriptPath = path.join(directory, "interim-script-results.json");
      fs.writeFileSync(visualPath, JSON.stringify(visualResults));
      fs.writeFileSync(finalScriptPath, JSON.stringify(scriptResults));
      fs.writeFileSync(
        interimScriptPath,
        JSON.stringify({ phase: "interim", threshold: 90, results: scriptResults }),
      );

      expect(() =>
        assertFeatureResultReportsComplete(catalog, visualPath, finalScriptPath),
      ).not.toThrow();
      expect(() =>
        assertFeatureResultReportsComplete(catalog, visualPath, interimScriptPath),
      ).toThrow(/invalid feature results/);
    } finally {
      fs.rmSync(directory, { recursive: true, force: true });
    }
  });
});

describe("fixed fidelity acceptance CLI routes", () => {
  test.each([
    ["input-state-final", "input-state", "tmp/html-fidelity/input-state", 99, undefined],
    [
      "input-state-interim",
      "input-state",
      "tmp/html-fidelity/input-state-interim",
      90,
      { phase: "interim" },
    ],
    ["dynamic-final", "dynamic", "tmp/html-fidelity/dynamic", 99, undefined],
    ["dynamic-interim", "dynamic", "tmp/html-fidelity/dynamic-interim", 90, { phase: "interim" }],
  ] as const)(
    "%s binds its fixed input, output, score and phase",
    (mode, kind, output, threshold, options) => {
      const buildReport = `${output}/test-build.jsonl`;
      const expected = { kind, buildReport, output, threshold, ...(options ? { options } : {}) };
      expect(fidelityAcceptancePlan([mode, buildReport])).toEqual(expected);
    },
  );

  test("catalog capture routes only an explicit build, exact plan and output", () => {
    expect(
      fidelityAcceptancePlan([
        "catalog-stage90-capture",
        "tmp/html-fidelity/catalog/test-build.jsonl",
        "tmp/html-fidelity/catalog/exact84-plan.json",
        "tmp/html-fidelity/catalog/captures",
      ]),
    ).toEqual({
      kind: "catalog-stage90-capture",
      buildReport: "tmp/html-fidelity/catalog/test-build.jsonl",
      conditionPlan: "tmp/html-fidelity/catalog/exact84-plan.json",
      output: "tmp/html-fidelity/catalog/captures",
      threshold: 90,
      acceptance: false,
    });
    expect(() => fidelityAcceptancePlan(["catalog-stage90-capture", "build.jsonl"])).toThrow();
    expect(() =>
      fidelityAcceptancePlan([
        "catalog-stage90-capture",
        "build.jsonl",
        "plan.json",
        "out",
        "extra",
      ]),
    ).toThrow();
  });

  test("catalog resume requires both prior manifests and preserves the complete capture contract", () => {
    const argv = ["catalog-stage90-capture", "build.jsonl", "plan.json", "out"];
    const original = fidelityAcceptancePlan(argv);
    if (original.kind !== "catalog-stage90-capture")
      throw new Error("expected the catalog capture plan");
    const observationManifestPath = "prior-observation.json";
    const finalizedManifestPath = "prior-finalized.json";
    expect(
      fidelityAcceptancePlan([...argv, observationManifestPath, finalizedManifestPath]),
    ).toEqual({
      ...original,
      resume: { observationManifestPath, finalizedManifestPath },
    });
    for (const incomplete of [
      [observationManifestPath],
      [observationManifestPath, ""],
      ["", finalizedManifestPath],
      [observationManifestPath, finalizedManifestPath, "extra"],
    ])
      expect(() => fidelityAcceptancePlan([...argv, ...incomplete])).toThrow();

    const chromePath = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
    const chromeSha256 = "a".repeat(64);
    const pinnedChrome = [
      "--chrome-executable",
      chromePath,
      "--chrome-executable-sha256",
      chromeSha256,
    ];
    expect(fidelityAcceptancePlan([...argv, ...pinnedChrome, "--font-observer"])).toMatchObject({
      chromeExecutable: {
        executablePath: chromePath,
        executableSha256: chromeSha256,
        fontObserver: true,
      },
    });
    for (const invalid of [
      [...argv, "--font-observer"],
      [...argv, "--font-observer", ...pinnedChrome],
      [...argv, ...pinnedChrome, "--font-observer", "--font-observer"],
    ])
      expect(() => fidelityAcceptancePlan(invalid)).toThrow();
    expect(
      fidelityAcceptancePlan([
        ...argv,
        "--chrome-executable",
        chromePath,
        "--chrome-executable-sha256",
        chromeSha256,
      ]),
    ).toMatchObject({
      chromeExecutable: { executablePath: chromePath, executableSha256: chromeSha256 },
    });
    expect(() => fidelityAcceptancePlan([...argv, "--chrome-executable", chromePath])).toThrow();
    expect(() =>
      fidelityAcceptancePlan([
        ...argv,
        observationManifestPath,
        finalizedManifestPath,
        "--chrome-executable",
        chromePath,
        "--chrome-executable-sha256",
        "not-a-sha256",
      ]),
    ).toThrow();
  });

  test("observer resume rejects an old receipt without a trace binding", () => {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), "catalog-observer-resume-"));
    try {
      const plan = createCatalogStage90ConditionPlan();
      const tuple = plan.conditions[0];
      if (!tuple) throw new Error("catalog plan has no conditions");
      const receiptPath = path.join(directory, "chrome-receipt.json");
      const receipt = {
        schema: "krr-html84-chrome-condition-v1",
        status: "observed",
        conditionId: tuple.conditionId,
        caseId: tuple.caseId,
        boundaryState: tuple.state,
        operationKind: tuple.operationKind,
        sourceSha256: tuple.sourceSha256,
        binarySha256: sha256File(process.execPath),
        viewport: tuple.viewport,
      };
      fs.writeFileSync(receiptPath, JSON.stringify(receipt));
      const row = {
        ...tuple,
        binarySha256: receipt.binarySha256,
        chromeReceiptPath: receiptPath,
        chromeReceiptSha256: sha256File(receiptPath),
      } as unknown as JsonValue;
      const result = filterObserverReusableConditions(
        plan,
        [tuple.conditionId],
        new Map([[tuple.conditionId, row]]),
        new Map([[tuple.conditionId, row]]),
        {
          chromeExecutablePath: process.execPath,
          chromeExecutableSha256: receipt.binarySha256,
        },
      );
      expect(result.reusableConditionIds).toEqual([]);
      expect(result.rejected).toEqual([
        {
          conditionId: tuple.conditionId,
          reason: "finalized Chrome receipt has no observer trace binding",
        },
      ]);
    } finally {
      fs.rmSync(directory, { recursive: true, force: true });
    }
  });

  test("observer resume accepts only a finalized receipt with the same trace binding", () => {
    const directory = fs.mkdtempSync(path.resolve("tmp/catalog-observer-resume-positive-"));
    try {
      const plan = createCatalogStage90ConditionPlan();
      const tuple = plan.conditions[0];
      if (!tuple) throw new Error("catalog plan has no conditions");
      const executablePath = fs.realpathSync(process.execPath);
      const executableSha256 = sha256File(executablePath);
      const capturePath = path.join(directory, "capture.png");
      const tracePath = path.join(directory, "trace.json");
      fs.writeFileSync(capturePath, "observer capture");
      const sessionId = "observer-resume-session";
      const descriptorSha256 = "0".repeat(64);
      const trace = buildJoinedStaticFontTrace({
        sessionId,
        processId: 1,
        physicalFilePath: executablePath,
        physicalFileSha256: executableSha256,
        descriptorSha256,
        descriptorLength: 1,
        postScriptName: "Bun-Regular",
      });
      fs.writeFileSync(tracePath, JSON.stringify({ traceEvents: trace.events }));
      const traceSha256 = sha256File(tracePath);
      const binding = {
        schema: "krr-chromium-font-capture-v1",
        conditionId: tuple.conditionId,
        sourceSha256: tuple.sourceSha256,
        viewport: tuple.viewport,
        browserExecutablePath: executablePath,
        browserExecutableSha256: executableSha256,
        capturePath: fs.realpathSync(capturePath),
        captureSha256: sha256File(capturePath),
        tracePath: fs.realpathSync(tracePath),
        traceSha256,
        sessionId,
        argv: [
          executablePath,
          "--trace-startup=katana.font-observer",
          "--trace-startup-format=json",
          `--trace-startup-file=${fs.realpathSync(tracePath)}`,
          `--katana-actual-font-observer-session=${sessionId}`,
        ],
        processId: 1,
        actualExitCode: 0,
      };
      const receiptBase = {
        schema: "krr-html84-chrome-condition-v1",
        status: "observed",
        conditionId: tuple.conditionId,
        caseId: tuple.caseId,
        boundaryState: tuple.state,
        operationKind: tuple.operationKind,
        sourceSha256: tuple.sourceSha256,
        binarySha256: executableSha256,
        viewport: tuple.viewport,
        browserExecutablePath: executablePath,
        browserExecutableSha256: executableSha256,
        capturePath: fs.realpathSync(capturePath),
        captureSha256: sha256File(capturePath),
        state: {},
        boundary: {},
        resourceProvenance: { inventoryComplete: true, requests: [] },
        fontProvenance: [
          {
            scope: "capture",
            family: "fixture",
            source: "fixture",
            filePath: executablePath,
            fileSha256: executableSha256,
            faceIndex: 0,
            selected: true,
            fileBytesVerified: true,
          },
        ],
        traceBinding: binding,
      };
      const finalizedReceiptPath = path.join(directory, "finalized-receipt.json");
      const observationReceiptPath = path.join(directory, "observation-receipt.json");
      const observationReceipt = {
        ...receiptBase,
        status: "unknown",
        unknownReason: "fixture unknown receipt",
        fontProvenance: [],
      };
      fs.writeFileSync(finalizedReceiptPath, JSON.stringify(receiptBase));
      fs.writeFileSync(observationReceiptPath, JSON.stringify(observationReceipt));
      const finalizedRow = {
        ...tuple,
        binarySha256: executableSha256,
        chromeReceiptPath: finalizedReceiptPath,
        chromeReceiptSha256: sha256File(finalizedReceiptPath),
      } as unknown as JsonValue;
      const observationRow = {
        ...tuple,
        binarySha256: executableSha256,
        chromeReceiptPath: observationReceiptPath,
        chromeReceiptSha256: sha256File(observationReceiptPath),
      } as unknown as Record<string, JsonValue>;
      const accepted = filterObserverReusableConditions(
        plan,
        [tuple.conditionId],
        new Map([[tuple.conditionId, finalizedRow]]),
        new Map([[tuple.conditionId, observationRow]]),
        { chromeExecutablePath: executablePath, chromeExecutableSha256: executableSha256 },
      );
      expect(accepted.rejected).toEqual([]);
      expect(accepted.reusableConditionIds).toEqual([tuple.conditionId]);

      const changed = {
        ...observationReceipt,
        traceBinding: { ...binding, sessionId: "changed" },
      };
      fs.writeFileSync(observationReceiptPath, JSON.stringify(changed));
      const rejected = filterObserverReusableConditions(
        plan,
        [tuple.conditionId],
        new Map([[tuple.conditionId, finalizedRow]]),
        new Map([
          [
            tuple.conditionId,
            {
              ...observationRow,
              chromeReceiptSha256: sha256File(observationReceiptPath),
            } as unknown as JsonValue,
          ],
        ]),
        { chromeExecutablePath: executablePath, chromeExecutableSha256: executableSha256 },
      );
      expect(rejected.reusableConditionIds).toEqual([]);
      expect(rejected.rejected[0]?.reason).toBe(
        "prior Chrome receipt trace binding differs from finalized receipt",
      );
    } finally {
      fs.rmSync(directory, { recursive: true, force: true });
    }
  });

  test("catalog plan CLI writes the exact physical 84 tuples without overwriting", () => {
    const directory = fs.mkdtempSync(path.resolve("tmp/html84-plan-cli-"));
    const output = path.join(directory, "exact84-plan.json");
    try {
      expectCatalogPlanArgumentsRejected(output);
      expect(runCatalogPlanCli(output).exitCode).toBe(0);
      expectPhysicalCatalogPlan(output);
      expectCatalogPlanCollisionPreservesBytes(output);
    } finally {
      fs.rmSync(directory, { recursive: true, force: true });
    }
  });

  test("catalog plan rejects sparse output arguments", () => {
    const argv: string[] = [];
    argv.length = 2;
    argv[0] = "catalog-stage90-plan";
    expect(() => fidelityAcceptancePlan(argv)).toThrow();
  });

  test("catalog validation binds candidate and independent trusted plan", () => {
    expect(
      fidelityAcceptancePlan([
        "catalog-stage90-validate",
        "candidate-suite.json",
        "independent-trusted-plan.json",
      ]),
    ).toEqual({
      kind: "catalog-stage90-validate",
      candidateSuite: "candidate-suite.json",
      trustedPlan: "independent-trusted-plan.json",
    });
    expect(() => fidelityAcceptancePlan(["catalog-stage90-validate", "candidate.json"])).toThrow();
  });

  test("catalog Chrome validation binds exactly one manifest and plan", () => {
    expect(
      fidelityAcceptancePlan([
        "catalog-stage90-validate-captures",
        "captures.json",
        "condition-plan.json",
      ]),
    ).toEqual({
      kind: "catalog-stage90-validate-captures",
      captureManifest: "captures.json",
      conditionPlan: "condition-plan.json",
    });
    expect(() =>
      fidelityAcceptancePlan(["catalog-stage90-validate-captures", "captures.json"]),
    ).toThrow();
    expect(() =>
      fidelityAcceptancePlan([
        "catalog-stage90-validate-captures",
        "captures.json",
        "condition-plan.json",
        "extra",
      ]),
    ).toThrow();
  });

  test("catalog Chrome validation rejects unverified captures before accepting transported files", async () => {
    const directory = fs.mkdtempSync(path.resolve("tmp/html84-chrome-validation-cli-"));
    const planPath = path.join(directory, "plan.json");
    const manifestPath = path.join(directory, "captures.json");
    const fontRequestPath = path.join(directory, "font-request.json");
    const fontResponsePath = path.join(directory, "font-response.json");
    try {
      expect(runCatalogPlanCli(planPath).exitCode).toBe(0);
      fs.writeFileSync(fontRequestPath, "{}");
      const fontRequestSha256 = sha256File(fontRequestPath);
      fs.writeFileSync(
        fontResponsePath,
        JSON.stringify({
          schema: "krr-chrome-font-identity-response-v1",
          requestSha256: fontRequestSha256,
          inventoryMethod: "fixture-method",
          inventoryLimit: "fixture-limit",
          captures: [],
        }),
      );
      const fontResponseSha256 = sha256File(fontResponsePath);
      const fontResponseSizeBytes = fs.statSync(fontResponsePath).size;
      const plan = JSON.parse(fs.readFileSync(planPath, "utf8")) as {
        catalogPath: string;
        catalogSha256: string;
        conditions: Array<Record<string, unknown>>;
      };
      fs.writeFileSync(
        manifestPath,
        JSON.stringify({
          profile: "catalog-stage90-captures",
          threshold: 90,
          acceptance: false,
          catalogPath: plan.catalogPath,
          catalogSha256: plan.catalogSha256,
          planPath,
          planSha256: sha256File(planPath),
          binaryPath: fs.realpathSync(process.execPath),
          binarySha256: sha256File(process.execPath),
          fontIdentityProbe: {
            request: { path: fontRequestPath, sha256: fontRequestSha256, sizeBytes: 2 },
            response: {
              path: fontResponsePath,
              sha256: fontResponseSha256,
              sizeBytes: fontResponseSizeBytes,
            },
            binary: {
              path: fs.realpathSync(process.execPath),
              sha256: sha256File(process.execPath),
            },
          },
          conditions: plan.conditions.map((condition) => ({
            ...condition,
            status: "unverified",
          })),
        }),
      );
      await expect(
        runFidelityAcceptance(["catalog-stage90-validate-captures", manifestPath, planPath]),
      ).rejects.toThrow("catalog raw capture is not verified");
    } finally {
      fs.rmSync(directory, { recursive: true, force: true });
    }
  });

  test.each([
    [
      "reports-final",
      "final",
      "tmp/html-fidelity/visual-feature-results.json",
      "tmp/html-fidelity/dynamic/script-feature-results.json",
    ],
    [
      "reports-interim",
      "interim",
      "tmp/html-fidelity/interim/interim-visual-feature-results.json",
      "tmp/html-fidelity/dynamic-interim/interim-script-feature-results.json",
    ],
  ] as const)("%s binds both report paths", (mode, phase, visualReport, scriptReport) => {
    expect(fidelityAcceptancePlan([mode])).toEqual({
      kind: "reports",
      phase,
      visualReport,
      scriptReport,
    });
  });

  test("returned capture plans cannot change the fixed routes", () => {
    const argv = ["dynamic-interim", "tmp/html-fidelity/dynamic-interim/test-build.jsonl"];
    const expected = fidelityAcceptancePlan(argv);
    const changed = fidelityAcceptancePlan(argv);
    if (changed.kind !== "dynamic" && changed.kind !== "input-state")
      throw new Error("expected dynamic capture route");
    changed.output = "alternative-output";
    changed.threshold = 99;
    if (changed.options) Object.assign(changed.options, { phase: "final" });
    expect(fidelityAcceptancePlan(argv)).toEqual(expected);
  });

  test.each([
    { argv: [] },
    { argv: ["unsupported"] },
    { argv: ["input-state-final"] },
    { argv: ["dynamic-interim", "tmp/html-fidelity/dynamic/test-build.jsonl"] },
    { argv: ["dynamic-final", ""] },
    { argv: ["dynamic-final", "tmp/html-fidelity/dynamic/test-build.jsonl", "--min-score", "90"] },
    { argv: ["reports-final", "--phase", "interim"] },
    { argv: ["reports-interim", "alternative-report.json"] },
  ])("rejects unsupported arguments %j before any capture", async ({ argv }) => {
    expect(() => fidelityAcceptancePlan(argv)).toThrow();
    await expect(runFidelityAcceptance(argv)).rejects.toThrow();
  });
});

describe("CoreText font provenance", () => {
  test("accepts legacy physical faces and strictly validates descriptor bounds and paths", async () => {
    await withDynamicFixture((_fixture, directory) => {
      const fontPath = path.join(directory, "font.bin");
      fs.writeFileSync(fontPath, "font bytes");
      const legacy = coreTextFace(fontPath);
      delete legacy.coreTextDescriptor;
      expect(validateFontProvenance([legacy])).toHaveLength(1);
      expect(validateFontProvenance([{ ...legacy, selectedNodeId: null }])).toHaveLength(1);
      for (const nodeId of [null, 0, -1])
        expect(() =>
          validateFontProvenance([{ ...legacy, scope: "node", selectedNodeId: nodeId }]),
        ).toThrow();
      const valid = coreTextFace(fontPath);
      expect(validateFontProvenance([valid])[0]?.coreTextDescriptor?.postScriptName).toBe(
        "Fixture-Regular",
      );
      for (const mutation of invalidCoreTextDescriptors(fontPath, directory))
        expect(() => validateFontProvenance([mutation])).toThrow();
    });
  });

  test("binds descriptor PostScript names to the selected-font order", async () => {
    await withDynamicFixture((_fixture, directory) => {
      const fontPath = path.join(directory, "font.bin");
      fs.writeFileSync(fontPath, "font bytes");
      const faces = [coreTextFace(fontPath), coreTextFace(fontPath, "Fixture-Italic")];
      const validated = validateFontProvenance(faces);
      expect(() =>
        assertCoreTextPostScriptOrder(validated, ["Fixture-Regular", "Fixture-Italic"]),
      ).not.toThrow();
      expect(() =>
        assertCoreTextPostScriptOrder(validated, ["Fixture-Italic", "Fixture-Regular"]),
      ).toThrow(/ordered selected-font request/);
      const legacy = validateFontProvenance(
        faces.map((face) => {
          const copy = { ...face };
          delete copy.coreTextDescriptor;
          return copy;
        }),
      );
      expect(() =>
        assertCoreTextPostScriptOrder(legacy, ["Legacy-Regular", "Legacy-Italic"]),
      ).not.toThrow();
    });
  });
});

type CoreTextTestFace = Record<string, JsonValue> & { coreTextDescriptor?: JsonValue };

function coreTextFace(fontPath: string, postScriptName = "Fixture-Regular"): CoreTextTestFace {
  const canonicalPath = fs.realpathSync(fontPath);
  return {
    family: "Fixture Sans",
    scope: "capture",
    source: "CoreText physical descriptor",
    filePath: canonicalPath,
    fileSha256: createHash("sha256").update(fs.readFileSync(fontPath)).digest("hex"),
    faceIndex: 0,
    selected: true,
    fileBytesVerified: true,
    coreTextDescriptor: {
      urlPath: canonicalPath,
      postScriptName,
      variationAxes: [
        {
          identifier: CORETEXT_TEST_AXIS_ID,
          minimumValue: CORETEXT_TEST_AXIS_MINIMUM,
          maximumValue: CORETEXT_TEST_AXIS_MAXIMUM,
          defaultValue: CORETEXT_TEST_AXIS_DEFAULT,
        },
      ],
      variationCoordinates: [
        { identifier: CORETEXT_TEST_AXIS_ID, value: CORETEXT_TEST_AXIS_DEFAULT },
      ],
    },
  };
}

function invalidCoreTextDescriptors(
  fontPath: string,
  directory: string,
): Record<string, JsonValue>[] {
  const face = coreTextFace(fontPath);
  const descriptor = face.coreTextDescriptor as Record<string, JsonValue>;
  const axis = (descriptor.variationAxes as JsonValue[])[0] as Record<string, JsonValue>;
  const coordinate = (descriptor.variationCoordinates as JsonValue[])[0] as Record<
    string,
    JsonValue
  >;
  const otherPath = path.join(directory, "other-font.bin");
  fs.writeFileSync(otherPath, "other bytes");
  const canonicalOtherPath = fs.realpathSync(otherPath);
  return [
    { ...face, coreTextDescriptor: { ...descriptor, extra: true } },
    { ...face, coreTextDescriptor: { ...descriptor, urlPath: "relative/font.ttf" } },
    { ...face, coreTextDescriptor: { ...descriptor, urlPath: canonicalOtherPath } },
    { ...face, coreTextDescriptor: { ...descriptor, variationAxes: [] } },
    { ...face, coreTextDescriptor: { ...descriptor, variationCoordinates: [] } },
    { ...face, coreTextDescriptor: { ...descriptor, variationAxes: [axis, axis] } },
    {
      ...face,
      coreTextDescriptor: { ...descriptor, variationCoordinates: [coordinate, coordinate] },
    },
    {
      ...face,
      coreTextDescriptor: {
        ...descriptor,
        variationAxes: [{ ...axis, identifier: CORETEXT_TEST_U32_OVERFLOW }],
      },
    },
    {
      ...face,
      coreTextDescriptor: {
        ...descriptor,
        variationAxes: [{ ...axis, extra: true }],
      },
    },
    {
      ...face,
      coreTextDescriptor: {
        ...descriptor,
        variationAxes: [{ ...axis, minimumValue: CORETEXT_TEST_AXIS_OUT_OF_RANGE }],
      },
    },
    {
      ...face,
      coreTextDescriptor: {
        ...descriptor,
        variationAxes: [{ ...axis, defaultValue: CORETEXT_TEST_AXIS_OUT_OF_RANGE }],
      },
    },
    {
      ...face,
      coreTextDescriptor: {
        ...descriptor,
        variationCoordinates: [{ identifier: 1, value: CORETEXT_TEST_AXIS_DEFAULT }],
      },
    },
    {
      ...face,
      coreTextDescriptor: {
        ...descriptor,
        variationCoordinates: [{ ...coordinate, value: CORETEXT_TEST_AXIS_OUT_OF_RANGE }],
      },
    },
    {
      ...face,
      coreTextDescriptor: {
        ...descriptor,
        variationAxes: [{ ...axis, maximumValue: Number.POSITIVE_INFINITY }],
      },
    },
  ];
}
