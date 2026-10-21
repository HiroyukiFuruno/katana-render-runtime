import { expect, test } from "bun:test";
import fs from "node:fs";
import path from "node:path";
import { digestFileRecords, type FileRecord } from "../mermaid/native-build-receipt";
import { collectRuntimeBundleToolFingerprint } from "../runtime-bundles/runtime-bundle-tool-fingerprint";
import type { CatalogCaptureProducerContext } from "./catalog-capture-producer-context";
import type { JsonValue } from "./dynamic-state";
import { type CatalogCase, loadFeatureCatalog, readExpectedState } from "./feature-catalog";
import { jsonResourceFiles, validateCatalogChromeCaptures } from "./fidelity-acceptance-cli";
import { sha256File } from "./reference-capture";
import {
  assertCatalogCaptureIdentity,
  assertCatalogStage90ConditionPlan,
  assertStageScoreSuite,
  createCatalogStage90ConditionPlan,
  inspectCatalogRawStage90Reuse,
  requiredStageScoreTuples,
  validateCatalogRawStage90Captures,
} from "./stage-score-suite";

const CORETEXT_FIXTURE_AXIS_ID = 2_003_265_652;
const CORETEXT_FIXTURE_AXIS_MINIMUM = 100;
const CORETEXT_FIXTURE_AXIS_MAXIMUM = 900;
const CORETEXT_FIXTURE_AXIS_DEFAULT = 400;

type JsonObject = { [key: string]: JsonValue };
const producerTools = collectRuntimeBundleToolFingerprint();
type NavigationLinkFixture = JsonObject & {
  pointerTrace: JsonValue[];
  frame: { generation: number; contentHeight: number; scrollY: number };
  scrollY: number;
};
type NavigationFixture = { links: NavigationLinkFixture[]; targetNodeId: number };
type PointerTraceOptions = {
  startGeneration: number;
  startScrollY: number;
  finalScrollY: number;
};
type Viewport = { width: number; height: number; scale: number };

const viewports: Viewport[] = [
  { width: 1280, height: 900, scale: 1 },
  { width: 960, height: 720, scale: 1 },
  { width: 620, height: 900, scale: 1 },
];

test("exports the same exact catalog-authoritative 84 condition tuples", () => {
  const tuples = requiredStageScoreTuples();
  expect(tuples).toHaveLength(84);
  expect(new Set(tuples.map((tuple) => tuple.conditionId)).size).toBe(84);
  expect(tuples).toContainEqual({
    conditionId: "typography/ja-en-mixed/1280x900@1",
    caseId: "typography",
    state: "ja-en-mixed",
    operationKind: "load-and-observe-target",
    targetSelector: ".mixed",
    viewport: { width: 1280, height: 900, scale: 1 },
  });
  expect(tuples).toContainEqual({
    conditionId: "resources/expected-state/620x900@1",
    caseId: "resources",
    state: "expected-state",
    operationKind: "script-await-resource-font",
    viewport: { width: 620, height: 900, scale: 1 },
  });
});

test("pins the physical exact-84 plan to current fixture and catalog bytes", () => {
  const plan = createCatalogStage90ConditionPlan();
  expect(plan.conditions).toHaveLength(84);
  expect(() => assertCatalogStage90ConditionPlan(plan as unknown as JsonValue)).not.toThrow();
  expect(() =>
    assertCatalogStage90ConditionPlan({
      ...plan,
      conditions: plan.conditions.slice(1),
    } as unknown as JsonValue),
  ).toThrow(/coverage is incomplete/);
  expect(() =>
    assertCatalogStage90ConditionPlan({
      ...plan,
      conditions: plan.conditions.map((condition, index) =>
        index === 0 ? { ...condition, sourceSha256: "0".repeat(64) } : condition,
      ),
    } as unknown as JsonValue),
  ).toThrow(/source differs/);
});

function suiteFixture(): { value: JsonObject; trustedPlan: JsonObject; cleanup: () => void } {
  const temporary = fs.mkdtempSync(path.resolve("tmp/stage-score-suite-"));
  const sourceRoot = path.resolve("tests/fixtures/html");
  const binary = process.execPath;
  const binarySha = sha256File(binary);
  const catalog = loadFeatureCatalog();
  const conditions: JsonObject[] = [];
  const trustedConditions: JsonObject[] = [];
  let index = 0;
  for (const entry of [...catalog.visualAspects, ...catalog.scriptCases]) {
    const states =
      entry.mode === "script"
        ? [entry.expectedStateId as string]
        : (entry.boundaryStates as string[]);
    for (const state of states) {
      for (const viewport of viewports) {
        const condition = conditionFixture(
          entry,
          state,
          viewport,
          index,
          temporary,
          sourceRoot,
          binarySha,
        );
        conditions.push(condition.candidate);
        trustedConditions.push(condition.trusted);
        index += 1;
      }
    }
  }
  const catalogPath = path.resolve("tests/fixtures/html/feature-catalog.json");
  return {
    value: {
      profile: "stage90-suite",
      threshold: 90,
      acceptance: false,
      catalogPath,
      catalogSha256: sha256File(catalogPath),
      conditions,
    },
    trustedPlan: {
      profile: "stage90-trusted-plan",
      conditions: trustedConditions,
    },
    cleanup: () => fs.rmSync(temporary, { recursive: true, force: true }),
  };
}

function conditionFixture(
  entry: CatalogCase,
  state: string,
  viewport: Viewport,
  index: number,
  temporary: string,
  sourceRoot: string,
  binarySha: string,
): { candidate: JsonObject; trusted: JsonObject } {
  const png = viewportPng(temporary, viewport);
  const sourceSha = sha256File(path.resolve(sourceRoot, entry.fixture));
  const nonce = `00000000-0000-4000-8000-${index.toString(16).padStart(12, "0")}`;
  const operation = requiredStageScoreTuples().find(
    (tuple) =>
      tuple.caseId === entry.id &&
      tuple.state === state &&
      tuple.viewport.width === viewport.width &&
      tuple.viewport.height === viewport.height &&
      tuple.viewport.scale === viewport.scale,
  );
  if (!operation) throw new Error(`missing test operation ${entry.id}/${state}`);
  const operationEvidence = operationFixture(
    operation.operationKind,
    operation.targetSelector,
    sourceSha,
    png,
    index,
    temporary,
    viewport.height,
  );
  const operationFrame = operationFinalFrame(operationEvidence);
  const actualScrollY = operationFrame.scrollY;
  const key = `${entry.id}/${viewport.width}x${viewport.height}@${viewport.scale}/${state}/scroll/${actualScrollY}`;
  const evidencePath = path.join(
    temporary,
    `${entry.id}-${viewport.width}x${viewport.height}-${state}.json`,
  );
  const evidence: JsonObject = {
    status: "matched",
    sourceSha256: sourceSha,
    binarySha256: binarySha,
    caseId: entry.id,
    state,
    actualScrollY,
    maxScrollY: Math.max(actualScrollY, 0),
    targetVisible: true,
    nonce,
    candidateSha256: sha256File(png),
    key,
    kind: "scroll",
    viewport,
    requestedY: actualScrollY,
    observedHash: "#target",
  };
  fs.writeFileSync(evidencePath, JSON.stringify(evidence));
  const image: JsonObject = {
    key,
    caseId: entry.id,
    state,
    kind: "scroll",
    actualScrollY,
    maxScrollY: Math.max(actualScrollY, 0),
    requestedY: actualScrollY,
    viewportHeight: viewport.height,
    viewport,
    sourceSha256: sourceSha,
    binarySha256: binarySha,
    targetVisible: true,
    nonce,
    observedHash: "#target",
    referencePath: png,
    referenceSha256: sha256File(png),
    candidatePath: png,
    candidateSha256: sha256File(png),
    rmseScore: 100,
    maeScore: 100,
    operationEvidencePath: evidencePath,
    operationEvidenceSha256: sha256File(evidencePath),
    operationState: "matched",
    evidenceId: `evidence-${index}`,
  };
  const images = [image];
  if (actualScrollY > 0) {
    const topKey = `${entry.id}/${viewport.width}x${viewport.height}@${viewport.scale}/${state}/scroll/0`;
    const topEvidencePath = path.join(temporary, `${entry.id}-${index}-scroll-top.json`);
    fs.writeFileSync(
      topEvidencePath,
      JSON.stringify({
        ...evidence,
        actualScrollY: 0,
        requestedY: 0,
        key: topKey,
        candidateSha256: sha256File(png),
      }),
    );
    images.unshift({
      ...image,
      key: topKey,
      actualScrollY: 0,
      requestedY: 0,
      operationEvidencePath: topEvidencePath,
      operationEvidenceSha256: sha256File(topEvidencePath),
      evidenceId: `evidence-${index}-scroll-top`,
    });
  }
  const performance: JsonObject = {
    environment: "test",
    conditionId: operation.conditionId,
    phases: ["cold", "warm", "re-display", "full-Y", "post-operation"].map((name) => ({
      name,
      observedMs: 1,
      baselineMs: 1,
      observedMemoryBytes: 1,
      baselineMemoryBytes: 1,
      measurementPairs: 1,
    })),
  };
  const manifest: JsonObject = {
    profile: "stage90",
    threshold: 90,
    sourceSha256: sourceSha,
    binarySha256: binarySha,
    images,
    tocKeys: [],
    performance,
  };
  const rawPath = path.join(temporary, `raw-${index}.json`);
  const chromeReceiptPath = path.join(temporary, `chrome-${index}.json`);
  const sourceFile = path.resolve(sourceRoot, entry.fixture);
  const fontPath = path.join(temporary, "font-bytes.bin");
  const resourcePath = path.join(temporary, "resource-bytes.bin");
  fs.writeFileSync(fontPath, "selected font bytes");
  fs.writeFileSync(resourcePath, "loaded resource bytes");
  const canonicalFontPath = fs.realpathSync(fontPath);
  const rawState: JsonValue =
    entry.mode === "script"
      ? readExpectedState(sourceFile, entry.expectedStateId)
      : { observed: state, value: index };
  const physicalProvenance = {
    resourceProvenance: {
      inventoryComplete: true,
      requests: [
        {
          url: `file://${sourceFile}`,
          bodyPath: sourceFile,
          bytesSha256: sha256File(sourceFile),
          loaded: true,
        },
      ],
    },
    fontProvenance: [
      {
        scope: "capture",
        family: "fixture-face",
        source: "system-physical-fixture",
        filePath: canonicalFontPath,
        fileSha256: sha256File(fontPath),
        faceIndex: 0,
        selected: true,
        fileBytesVerified: true,
        coreTextDescriptor: {
          urlPath: canonicalFontPath,
          postScriptName: "Fixture-Regular",
          variationAxes: [
            {
              identifier: CORETEXT_FIXTURE_AXIS_ID,
              minimumValue: CORETEXT_FIXTURE_AXIS_MINIMUM,
              maximumValue: CORETEXT_FIXTURE_AXIS_MAXIMUM,
              defaultValue: CORETEXT_FIXTURE_AXIS_DEFAULT,
            },
          ],
          variationCoordinates: [
            { identifier: CORETEXT_FIXTURE_AXIS_ID, value: CORETEXT_FIXTURE_AXIS_DEFAULT },
          ],
        },
      },
    ],
  };
  const rawReport = {
    schema: "krr-html84-raw-condition-v1",
    status: "observed",
    conditionId: operation.conditionId,
    caseId: entry.id,
    boundaryState: state,
    operationKind: operation.operationKind,
    executionNonce: nonce,
    htmlSha256: sourceSha,
    binarySha256: binarySha,
    capturePath: png,
    captureSha256: sha256File(png),
    viewport,
    state: rawState,
    geometry: [{ nodeId: 1, x: 1, y: 1, width: 10, height: 10 }],
    operationEvidence,
    frame: {
      ...operationFrame,
      contentHeight: viewport.height + Math.max(actualScrollY, 0),
    },
    targetVisible: true,
    ...physicalProvenance,
  };
  fs.writeFileSync(rawPath, JSON.stringify(rawReport));
  const chromeReceipt = {
    schema: "krr-html84-chrome-condition-v1",
    status: "observed",
    conditionId: operation.conditionId,
    caseId: entry.id,
    boundaryState: state,
    operationKind: operation.operationKind,
    sourceSha256: sourceSha,
    binarySha256: binarySha,
    viewport,
    browserExecutablePath: process.execPath,
    browserExecutableSha256: binarySha,
    capturePath: png,
    captureSha256: sha256File(png),
    state: rawState,
    boundary: { state },
    ...physicalProvenance,
  };
  fs.writeFileSync(chromeReceiptPath, JSON.stringify(chromeReceipt));
  const condition = {
    conditionId: operation.conditionId,
    caseId: entry.id,
    state,
    operationKind: operation.operationKind,
    ...(operation.targetSelector ? { targetSelector: operation.targetSelector } : {}),
    viewport,
    sourceSha256: sourceSha,
    binarySha256: binarySha,
    binaryPath: process.execPath,
    rawKrrReportPath: rawPath,
    rawKrrReportSha256: sha256File(rawPath),
    candidate: manifest,
  };
  const { candidate, rawKrrReportPath, rawKrrReportSha256, ...tuple } = condition;
  void candidate;
  void rawKrrReportPath;
  void rawKrrReportSha256;
  return {
    candidate: condition,
    trusted: {
      ...tuple,
      chromeReceiptPath,
      chromeReceiptSha256: sha256File(chromeReceiptPath),
      trusted: manifest,
    },
  };
}

function objectConditions(value: JsonObject): JsonObject[] {
  const conditions = value.conditions;
  if (!Array.isArray(conditions)) throw new Error("missing conditions");
  return conditions.map((condition) => {
    if (typeof condition !== "object" || condition === null || Array.isArray(condition))
      throw new Error("invalid condition");
    return condition as JsonObject;
  });
}

function captureForCondition(
  fixture: ReturnType<typeof rawCaptureFixture>,
  condition: JsonObject,
): JsonObject {
  const conditionId = requiredString(condition.conditionId, "condition ID");
  const capture = objectConditions(fixture.manifest).find(
    (candidate) => candidate.conditionId === conditionId,
  );
  if (!capture) throw new Error(`missing raw fixture row: ${conditionId}`);
  return capture;
}

function rawCaptureFixture(): {
  manifest: JsonObject;
  plan: JsonObject;
  planPath: string;
  producerContext: CatalogCaptureProducerContext;
  cleanup: () => void;
} {
  const suite = suiteFixture();
  const plan = createCatalogStage90ConditionPlan();
  const planPath = path.join(
    path.dirname(objectConditions(suite.value)[0]?.rawKrrReportPath as string),
    "plan.json",
  );
  fs.writeFileSync(planPath, JSON.stringify(plan));
  const sourceFile: FileRecord = {
    path: process.execPath,
    sha256: sha256File(process.execPath),
    size: fs.statSync(process.execPath).size,
  };
  const sourceManifestPath = path.join(path.dirname(planPath), "source-manifest.json");
  const producerContext: CatalogCaptureProducerContext = {
    schema: "krr-html84-capture-producer-context-v1",
    role: "before",
    sourceManifest: {
      path: sourceManifestPath,
      sha256: "",
      digest: digestFileRecords([sourceFile]),
    },
    tools: {} as CatalogCaptureProducerContext["tools"],
    playwrightFiles: [],
    os: { release: "test", version: "test" },
    fontEnvironment: {
      schema: "krr-font-environment-v1",
      cacheable: true,
      platform: "test",
      inventorySha256: "1".repeat(64),
      recordCount: 1,
      missReasons: [],
      pythonVersion: "3.14.3",
      pythonExecutableSha256: "2".repeat(64),
    },
  };
  const sourceManifest = {
    schema: "krr-native-build-source-manifest-v1",
    digest: producerContext.sourceManifest.digest,
    files: [sourceFile],
  };
  fs.writeFileSync(sourceManifestPath, `${JSON.stringify(sourceManifest)}\n`);
  producerContext.sourceManifest.sha256 = sha256File(sourceManifestPath);
  producerContext.tools = producerTools;
  const trustedById = new Map(
    objectConditions(suite.trustedPlan).map((condition) => [
      condition.conditionId as string,
      condition,
    ]),
  );
  const conditions = objectConditions(suite.value).map((condition) =>
    rawCaptureCondition(condition, trustedById.get(condition.conditionId as string)),
  );
  const catalogPath = path.resolve("tests/fixtures/html/feature-catalog.json");
  return {
    manifest: {
      profile: "catalog-stage90-captures",
      threshold: 90,
      acceptance: false,
      catalogPath,
      catalogSha256: sha256File(catalogPath),
      planPath,
      planSha256: sha256File(planPath),
      binaryPath: process.execPath,
      binarySha256: sha256File(process.execPath),
      chromeRuntimeVersion: "chrome-test-runtime/1",
      producerContext: producerContext as unknown as JsonValue,
      conditions,
    },
    plan: plan as unknown as JsonObject,
    planPath,
    producerContext,
    cleanup: suite.cleanup,
  };
}

function reuseContext(fixture: ReturnType<typeof rawCaptureFixture>) {
  return {
    planPath: fixture.planPath,
    binaryPath: process.execPath,
    chromeExecutablePath: process.execPath,
    chromeExecutableSha256: sha256File(process.execPath),
    chromeRuntimeVersion: "chrome-test-runtime/1",
    producerContext: fixture.producerContext,
  };
}

function rawCaptureCondition(condition: JsonObject, trusted: JsonObject | undefined): JsonObject {
  if (!trusted) throw new Error("missing trusted fixture condition");
  const stage = condition.candidate as JsonObject;
  const image = firstImage(stage.images);
  const chromeReceiptPath = requiredString(trusted.chromeReceiptPath, "Chrome receipt path");
  const receipt = JSON.parse(fs.readFileSync(chromeReceiptPath, "utf8")) as JsonObject;
  return {
    conditionId: requiredString(condition.conditionId, "condition ID"),
    caseId: requiredString(condition.caseId, "case ID"),
    state: requiredString(condition.state, "condition state"),
    ...(condition.targetSelector ? { targetSelector: condition.targetSelector } : {}),
    operationKind: requiredString(condition.operationKind, "operation kind"),
    viewport: requiredValue(condition.viewport, "condition viewport"),
    sourceSha256: requiredString(condition.sourceSha256, "condition source SHA"),
    binarySha256: requiredString(condition.binarySha256, "binary SHA"),
    status: "captured",
    fontIdentityRequestConditionId: requiredString(condition.conditionId, "condition ID"),
    fontIdentityObservation: { status: "observed", unknownReasons: [] },
    krrReportPath: requiredString(condition.rawKrrReportPath, "KRR report path"),
    krrReportSha256: requiredString(condition.rawKrrReportSha256, "KRR report SHA"),
    krrCapturePath: requiredString(image.candidatePath, "KRR capture path"),
    krrCaptureSha256: requiredString(image.candidateSha256, "KRR capture SHA"),
    chromeCapturePath: requiredString(receipt.capturePath, "Chrome capture path"),
    chromeCaptureSha256: requiredString(receipt.captureSha256, "Chrome capture SHA"),
    chromeReceiptPath,
    chromeReceiptSha256: requiredString(trusted.chromeReceiptSha256, "Chrome receipt SHA"),
    chromeState: requiredValue(receipt.state, "Chrome state"),
  };
}

function requiredValue(value: JsonValue | undefined, label: string): JsonValue {
  if (value === undefined) throw new Error(`${label} is missing`);
  return value;
}

function requiredString(value: JsonValue | undefined, label: string): string {
  if (typeof value !== "string") throw new Error(`${label} is missing`);
  return value;
}

function firstImage(value: JsonValue | undefined): JsonObject {
  if (!Array.isArray(value) || value.length === 0) throw new Error("missing candidate image");
  const image = value[0];
  if (typeof image !== "object" || image === null || Array.isArray(image))
    throw new Error("invalid candidate image");
  return image as JsonObject;
}

test("validates 84 real receipt pairs and scores every independent raw image", () => {
  const fixture = rawCaptureFixture();
  try {
    const results = validateCatalogRawStage90Captures(
      fixture.manifest,
      fixture.plan,
      fixture.planPath,
    );
    expect(results).toHaveLength(84);
    expect(results.every((result) => result.score === 100)).toBe(true);
  } finally {
    fixture.cleanup();
  }
});

test.each([
  ["matching physical font", false],
  ["physical font mismatch", true],
] as const)("CLI validates %s evidence", (_label, mismatch) => {
  const fixture = rawCaptureFixture();
  try {
    const paths = addFontIdentityEvidence(fixture);
    if (mismatch) introducePhysicalFontMismatch(fixture, paths);
    const result = requireJsonObject(
      validateCatalogChromeCaptures(paths.manifestPath, fixture.planPath),
      "CLI result",
    );
    expect(result.acceptance).toBe(!mismatch);
    expect(result.fontIdentityEquivalence).toBe(mismatch ? "not-established" : "established");
    if (mismatch) expect(result.fontIdentityDiagnostics).not.toEqual([]);
    else expect(result.fontIdentityDiagnostics).toEqual([]);
  } finally {
    fixture.cleanup();
  }
});

test("raw stage90 adapter rejects a capture that is not strictly observed", () => {
  const fixture = rawCaptureFixture();
  try {
    const first = objectConditions(fixture.manifest)[0];
    if (!first) throw new Error("missing raw fixture row");
    first.status = "unverified";
    expect(() =>
      validateCatalogRawStage90Captures(fixture.manifest, fixture.plan, fixture.planPath),
    ).toThrow(/not verified/);
  } finally {
    fixture.cleanup();
  }
});

interface FontIdentityFixturePaths {
  manifestPath: string;
  requestPath: string;
  responsePath: string;
}

function introducePhysicalFontMismatch(
  fixture: ReturnType<typeof rawCaptureFixture>,
  paths: FontIdentityFixturePaths,
): void {
  const first = objectConditions(fixture.manifest)[0];
  if (!first) throw new Error("missing mismatch fixture condition");
  const alternate = path.resolve(
    "crates/katana-render-runtime/assets/test-fixtures/font-probes/recursive-latin-crsv-normal.ttf",
  );
  const receiptPath = requiredString(first.chromeReceiptPath, "Chrome receipt path");
  const receipt = readJsonObject(receiptPath);
  receipt.fontProvenance = [physicalFont(alternate)];
  writePinnedJson(receipt, receiptPath);
  first.chromeReceiptSha256 = sha256File(receiptPath);
  const response = readJsonObject(paths.responsePath);
  const responseRows = response.captures;
  if (!Array.isArray(responseRows)) throw new Error("missing font identity response rows");
  const firstResponse = responseRows[0];
  if (typeof firstResponse !== "object" || firstResponse === null || Array.isArray(firstResponse))
    throw new Error("invalid font identity response row");
  firstResponse.fontProvenance = receipt.fontProvenance;
  writePinnedJson(response, paths.responsePath);
  const responseSha256 = sha256File(paths.responsePath);
  for (const condition of objectConditions(fixture.manifest)) {
    condition.fontIdentityResponseSha256 = responseSha256;
    condition.fontIdentityResponseSizeBytes = fs.statSync(paths.responsePath).size;
  }
  const probe = requireJsonObject(fixture.manifest.fontIdentityProbe, "font identity probe");
  const responsePin = requireJsonObject(probe.response, "font response pin");
  responsePin.sha256 = responseSha256;
  responsePin.sizeBytes = fs.statSync(paths.responsePath).size;
  writePinnedManifest(fixture, paths.manifestPath);
}

function addFontIdentityEvidence(
  fixture: ReturnType<typeof rawCaptureFixture>,
): FontIdentityFixturePaths {
  const capturesById = new Map(
    objectConditions(fixture.manifest).map((condition) => [condition.conditionId, condition]),
  );
  fixture.manifest.conditions = objectConditions(fixture.plan).map((condition) => {
    const capture = capturesById.get(condition.conditionId);
    if (!capture) throw new Error("missing planned font fixture condition");
    return capture;
  });
  const directory = path.dirname(fixture.planPath);
  const requestPath = path.join(directory, "font-request.json");
  const responsePath = path.join(directory, "font-response.json");
  const requestRows: JsonObject[] = [];
  const responseRows: JsonObject[] = [];
  const fontPath = path.resolve("crates/katana-render-runtime/assets/fonts/NotoSans-Regular.ttf");
  const font = physicalFont(fontPath);
  const selectedFont: JsonObject = {
    familyName: "Noto Sans",
    postScriptName: "NotoSans-Regular",
    glyphCount: 1,
    isCustomFont: true,
  };
  for (const condition of objectConditions(fixture.manifest)) {
    const krrPath = requiredString(condition.krrReportPath, "KRR report path");
    const krr = readJsonObject(krrPath);
    krr.fontProvenance = [font];
    writePinnedJson(krr, krrPath);
    condition.krrReportSha256 = sha256File(krrPath);
    const chromePath = requiredString(condition.chromeReceiptPath, "Chrome receipt path");
    const chrome = readJsonObject(chromePath);
    chrome.fontProvenance = [font];
    writePinnedJson(chrome, chromePath);
    condition.chromeReceiptSha256 = sha256File(chromePath);
    const resourceProvenance = requireJsonObject(
      chrome.resourceProvenance,
      "Chrome resource provenance",
    );
    condition.resourceProvenance = resourceProvenance;
    condition.fontIdentityObservation = {
      status: "observed",
      selectedFonts: [selectedFont],
      unknownReasons: [],
    };
    condition.fontIdentityUnknownReasons = [];
    requestRows.push({
      conditionId: requiredString(condition.conditionId, "condition ID"),
      selectedFonts: [selectedFont],
      resourceFiles: jsonResourceFiles(resourceProvenance.requests) as unknown as JsonValue,
    });
    responseRows.push({
      conditionId: requiredString(condition.conditionId, "condition ID"),
      status: "resolved",
      fontProvenance: [font],
      unknownReasons: [],
    });
  }
  writePinnedJson(
    { schema: "krr-chrome-font-identity-request-v1", captures: requestRows },
    requestPath,
  );
  const requestSha256 = sha256File(requestPath);
  writePinnedJson(
    {
      schema: "krr-chrome-font-identity-response-v1",
      requestSha256,
      inventoryMethod: "fixture",
      inventoryLimit: "fixture physical inventory",
      captures: responseRows,
    },
    responsePath,
  );
  const responseSha256 = sha256File(responsePath);
  for (const condition of objectConditions(fixture.manifest)) {
    condition.fontIdentityRequestConditionId = requiredString(
      condition.conditionId,
      "condition ID",
    );
    condition.fontIdentityRequestPath = requestPath;
    condition.fontIdentityRequestSha256 = requestSha256;
    condition.fontIdentityRequestSizeBytes = fs.statSync(requestPath).size;
    condition.fontIdentityResponsePath = responsePath;
    condition.fontIdentityResponseSha256 = responseSha256;
    condition.fontIdentityResponseSizeBytes = fs.statSync(responsePath).size;
  }
  fixture.manifest.fontIdentityProbe = {
    request: { path: requestPath, sha256: requestSha256, sizeBytes: fs.statSync(requestPath).size },
    response: {
      path: responsePath,
      sha256: responseSha256,
      sizeBytes: fs.statSync(responsePath).size,
    },
    binary: { path: fs.realpathSync(process.execPath), sha256: sha256File(process.execPath) },
  };
  const manifestPath = path.join(directory, "capture-manifest.json");
  writePinnedManifest(fixture, manifestPath);
  return { manifestPath, requestPath, responsePath };
}

function physicalFont(filePath: string): JsonObject {
  return {
    scope: "capture",
    family: "Noto Sans",
    source: "test-fixture-font",
    filePath,
    fileSha256: sha256File(filePath),
    faceIndex: 0,
    selected: true,
    fileBytesVerified: true,
  };
}

function readJsonObject(filePath: string): JsonObject {
  const value = JSON.parse(fs.readFileSync(filePath, "utf8")) as JsonValue;
  if (typeof value !== "object" || value === null || Array.isArray(value))
    throw new Error("fixture JSON is not an object");
  return value;
}

function requireJsonObject(value: JsonValue | undefined, label: string): JsonObject {
  if (typeof value !== "object" || value === null || Array.isArray(value))
    throw new Error(`${label} is not an object`);
  return value;
}

function writePinnedJson(value: JsonObject, filePath: string): void {
  fs.writeFileSync(filePath, `${JSON.stringify(value)}\n`);
}

function writePinnedManifest(
  fixture: ReturnType<typeof rawCaptureFixture>,
  manifestPath: string,
): string {
  writePinnedJson(fixture.manifest, manifestPath);
  return manifestPath;
}

test("raw stage90 reuse inspection accepts only current binary and Chrome receipts", () => {
  const fixture = rawCaptureFixture();
  try {
    const context = reuseContext(fixture);
    const result = inspectCatalogRawStage90Reuse(fixture.manifest, fixture.plan, context);
    expect(result.reusableConditionIds).toHaveLength(84);
    expect(result.rejected).toHaveLength(0);
  } finally {
    fixture.cleanup();
  }
});

const identityMutations: ReadonlyArray<readonly [string, (capture: JsonObject) => void]> = [
  ["missing selector", (capture: JsonObject) => delete capture.targetSelector],
  ["different selector", (capture: JsonObject) => (capture.targetSelector = ".other")],
  ["different viewport", (capture: JsonObject) => (capture.viewport = { width: 1 })],
  ["different source", (capture: JsonObject) => (capture.sourceSha256 = "0".repeat(64))],
  ["different binary", (capture: JsonObject) => (capture.binarySha256 = "0".repeat(64))],
];

function identityFixture(): {
  fixture: ReturnType<typeof rawCaptureFixture>;
  condition: Parameters<typeof assertCatalogCaptureIdentity>[0];
  capture: JsonObject;
} {
  const fixture = rawCaptureFixture();
  const conditionValue = objectConditions(fixture.plan).find(
    (value: JsonObject) => value.targetSelector !== undefined,
  );
  const condition = conditionValue as unknown as Parameters<typeof assertCatalogCaptureIdentity>[0];
  if (!condition || !conditionValue) {
    fixture.cleanup();
    throw new Error("missing raw fixture row");
  }
  return { fixture, condition, capture: captureForCondition(fixture, conditionValue) };
}

test.each(identityMutations)("raw stage90 identity rejects %s", (_label, mutate) => {
  const { fixture, condition, capture } = identityFixture();
  try {
    mutate(capture);
    expect(() =>
      assertCatalogCaptureIdentity(condition, capture, sha256File(process.execPath)),
    ).toThrow(/catalog raw capture identity differs/);
  } finally {
    fixture.cleanup();
  }
});

test("raw stage90 identity validation leaves late metadata checks to the strict reader", () => {
  const { fixture, condition, capture } = identityFixture();
  try {
    const binarySha256 = requiredString(capture.binarySha256, "binary SHA");
    capture.status = "unverified";
    expect(() => assertCatalogCaptureIdentity(condition, capture, binarySha256)).not.toThrow();
    expect(() =>
      validateCatalogRawStage90Captures(fixture.manifest, fixture.plan, fixture.planPath),
    ).toThrow(/not verified/);
  } finally {
    fixture.cleanup();
  }
});

test("raw stage90 reuse inspection rejects an invalid current plan before cache review", () => {
  const fixture = rawCaptureFixture();
  try {
    const invalidPlan = { ...fixture.plan, conditions: [] };
    expect(() =>
      inspectCatalogRawStage90Reuse(fixture.manifest, invalidPlan, reuseContext(fixture)),
    ).toThrow(/condition coverage is incomplete/);
  } finally {
    fixture.cleanup();
  }
});

test("raw stage90 reuse inspection accepts a copied current plan", () => {
  const fixture = rawCaptureFixture();
  try {
    const copiedPlanPath = path.join(path.dirname(fixture.planPath), "copied-plan.json");
    fs.writeFileSync(copiedPlanPath, fs.readFileSync(fixture.planPath));
    const result = inspectCatalogRawStage90Reuse(fixture.manifest, fixture.plan, {
      ...reuseContext(fixture),
      planPath: copiedPlanPath,
    });
    expect(result.reusableConditionIds).toHaveLength(84);
    expect(result.rejected).toHaveLength(0);
  } finally {
    fixture.cleanup();
  }
});

test("raw stage90 reuse inspection rejects a changed single receipt", () => {
  const fixture = rawCaptureFixture();
  try {
    const first = objectConditions(fixture.manifest)[0];
    if (!first) throw new Error("missing raw fixture row");
    const receiptPath = requiredString(first.chromeReceiptPath, "Chrome receipt path");
    fs.appendFileSync(receiptPath, "changed");
    const result = inspectCatalogRawStage90Reuse(
      fixture.manifest,
      fixture.plan,
      reuseContext(fixture),
    );
    expect(result.reusableConditionIds).toHaveLength(83);
    expect(result.rejected).toHaveLength(1);
    expect(result.rejected[0]?.conditionId).toBe(requiredString(first.conditionId, "condition ID"));
  } finally {
    fixture.cleanup();
  }
});

test("raw stage90 reuse inspection rejects a changed current Chrome identity", () => {
  const fixture = rawCaptureFixture();
  try {
    const result = inspectCatalogRawStage90Reuse(fixture.manifest, fixture.plan, {
      ...reuseContext(fixture),
      chromeExecutableSha256: "0".repeat(64),
    });
    expect(result.reusableConditionIds).toHaveLength(0);
    expect(result.rejected).toHaveLength(84);
    expect(
      result.rejected.every(
        ({ reason }) => reason === "current Chrome executable identity differs",
      ),
    ).toBe(true);
  } finally {
    fixture.cleanup();
  }
});

test("raw stage90 reuse inspection rejects a changed Chrome runtime version", () => {
  const fixture = rawCaptureFixture();
  try {
    const result = inspectCatalogRawStage90Reuse(fixture.manifest, fixture.plan, {
      ...reuseContext(fixture),
      chromeRuntimeVersion: "chrome-test-runtime/2",
    });
    expect(result.reusableConditionIds).toHaveLength(0);
    expect(result.rejected).toHaveLength(84);
    expect(
      result.rejected.every(({ reason }) => reason === "current Chrome runtime version differs"),
    ).toBe(true);
  } finally {
    fixture.cleanup();
  }
});

test.each([
  ["font inventory", { inventorySha256: "3".repeat(64) }],
  ["Python executable", { pythonExecutableSha256: "4".repeat(64) }],
])("raw stage90 reuse inspection rejects changed %s identity", (_label, change) => {
  const fixture = rawCaptureFixture();
  try {
    const context = reuseContext(fixture);
    const fontEnvironment = context.producerContext.fontEnvironment;
    if (!fontEnvironment) throw new Error("missing font environment fixture");
    const result = inspectCatalogRawStage90Reuse(fixture.manifest, fixture.plan, {
      ...context,
      producerContext: {
        ...context.producerContext,
        fontEnvironment: {
          ...fontEnvironment,
          ...change,
        },
      },
    });
    expect(result.reusableConditionIds).toHaveLength(0);
    expect(result.rejected).toHaveLength(84);
  } finally {
    fixture.cleanup();
  }
});

test.each(["captured", "current"] as const)(
  "raw stage90 reuse inspection rejects unavailable %s font environment",
  (side) => {
    const fixture = rawCaptureFixture();
    try {
      const context = reuseContext(fixture);
      if (side === "captured") {
        const producerContext = fixture.manifest.producerContext as JsonObject;
        const fontEnvironment = producerContext.fontEnvironment as unknown as JsonObject;
        fontEnvironment.cacheable = false;
      } else {
        const fontEnvironment = context.producerContext.fontEnvironment as unknown as JsonObject;
        fontEnvironment.cacheable = false;
      }
      const result = inspectCatalogRawStage90Reuse(fixture.manifest, fixture.plan, context);
      expect(result.reusableConditionIds).toHaveLength(0);
      expect(result.rejected).toHaveLength(84);
      expect(result.rejected.every(({ reason }) => reason.includes("font environment"))).toBe(true);
    } finally {
      fixture.cleanup();
    }
  },
);

test("raw stage90 reuse inspection rejects duplicate condition inventory", () => {
  const fixture = rawCaptureFixture();
  try {
    const captures = fixture.manifest.conditions;
    if (!Array.isArray(captures)) throw new Error("missing raw captures");
    const first = captures[0];
    if (!first) throw new Error("missing raw fixture row");
    captures.push(first);
    const result = inspectCatalogRawStage90Reuse(
      fixture.manifest,
      fixture.plan,
      reuseContext(fixture),
    );
    expect(result.reusableConditionIds).toHaveLength(0);
    expect(result.rejected).toHaveLength(84);
    expect(result.rejected.every(({ reason }) => reason.includes("incomplete or duplicated"))).toBe(
      true,
    );
  } finally {
    fixture.cleanup();
  }
});

function operationFixture(
  kind: string,
  selector: string | undefined,
  sourceSha: string,
  png: string,
  index: number,
  temporary: string,
  viewportHeight: number,
): JsonObject {
  const before: JsonValue = { value: index };
  const after: JsonValue = { value: index + 1 };
  const target = { selector: selector ?? "#state", nodeId: 1, visible: true };
  const base: JsonObject = {
    schema: "krr-html84-operation-evidence-v1",
    kind,
    status: "observed",
    sourceSha256Before: sourceSha,
    sourceSha256After: sourceSha,
    preFrame: { generation: 1, scrollY: 0 },
    postFrame: { generation: 2, scrollY: 0 },
    target,
    state: { before, after },
  };
  const pointerSteps = ["move", "down", "up"].map((step) => ({
    kind: step,
    targetNodeId: 1,
    x: 4,
    y: 5,
  }));
  if (["load-and-observe-target", "script-load-observe"].includes(kind))
    return { ...base, delivery: "session-load" };
  if (["input-state-empty", "input-state-disabled-observe"].includes(kind))
    return { ...base, steps: [] };
  if (["input-state-filled-save", "input-state-selected", "input-state-expanded"].includes(kind))
    return { ...base, steps: pointerSteps };
  if (kind === "input-state-focus-visible-tab")
    return { ...base, steps: [{ kind: "key", key: "Tab", targetNodeId: 1 }] };
  if (kind === "navigation-pointer-click")
    return { ...base, steps: pointerSteps, hashBefore: "", hashAfter: "#a", destination: "#a" };
  if (kind === "navigation-all-pointer-clicks") {
    const navigation = navigationLinkFixtures(temporary, png, sourceSha, index, viewportHeight);
    const finalLink = navigation.links.at(-1);
    if (!finalLink) throw new Error("missing all-TOC navigation link fixture");
    return {
      ...base,
      preFrame: { generation: 1, scrollY: 0 },
      postFrame: { generation: finalLink.frame.generation, scrollY: finalLink.scrollY },
      target: { selector: selector ?? "#state", nodeId: navigation.targetNodeId, visible: true },
      state: { before: { tag: "A", text: "目次" }, after: { tag: "A", text: "目次" } },
      steps: navigation.links.flatMap((link) => link.pointerTrace),
      delivery: "session-load",
      links: navigation.links,
    };
  }
  if (kind === "script-real-events") return scriptRealEventsFixture(sourceSha, selector);
  if (kind === "script-await-timer-microtask") return settledTimerFixture(base);
  if (kind === "script-await-mutation-delivery") return settledObserverFixture(base);
  if (kind === "script-await-resource-font") return settledResourceFixture(base);
  if (["load-and-observe-target", "script-load-observe"].includes(kind))
    return { ...base, delivery: "session-load" };
  throw new Error(`missing test operation fixture ${kind}`);
}

function settledTimerFixture(base: JsonObject): JsonObject {
  const state = readExpectedState(
    path.resolve("tests/fixtures/html/aspects/script-async.html"),
    "expected-state",
  );
  return {
    ...base,
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
}

function settledObserverFixture(base: JsonObject): JsonObject {
  const state = readExpectedState(
    path.resolve("tests/fixtures/html/aspects/script-observers.html"),
    "expected-state",
  );
  return {
    ...base,
    state: { before: state, after: state },
    steps: [{ kind: "session-load-settled-observer-state", stateBefore: state, stateAfter: state }],
    delivery: "session-load-settled",
  };
}

function settledResourceFixture(base: JsonObject): JsonObject {
  const state = readExpectedState(
    path.resolve("tests/fixtures/html/aspects/script-resources.html"),
    "expected-state",
  );
  return {
    ...base,
    state: { before: state, after: state },
    steps: [{ kind: "session-load-settled-resource-state", stateBefore: state, stateAfter: state }],
    delivery: "session-load-settled",
  };
}

function operationFinalFrame(evidence: JsonObject): { generation: number; scrollY: number } {
  const value = evidence.postFrame;
  if (typeof value !== "object" || value === null || Array.isArray(value))
    throw new Error("operation fixture is missing its final frame");
  const frame = value as JsonObject;
  if (typeof frame.generation !== "number" || typeof frame.scrollY !== "number")
    throw new Error("operation fixture has an invalid final frame");
  return { generation: frame.generation, scrollY: frame.scrollY };
}

function navigationLinkFixtures(
  temporary: string,
  png: string,
  sourceSha: string,
  index: number,
  viewportHeight: number,
): NavigationFixture {
  const destinations = ["#a", "#b", "#c"];
  const firstTargetNodeId = 10 + index * destinations.length;
  const finalScrollY = 30 + destinations.length - 1;
  const contentHeight = viewportHeight + finalScrollY;
  const links = destinations.map((destination, linkIndex) => {
    const capturePath = path.join(temporary, `navigation-${index}-${linkIndex}.png`);
    fs.copyFileSync(png, capturePath);
    const scrollY = 30 + linkIndex;
    const previousScrollY = linkIndex === 0 ? 0 : 29 + linkIndex;
    const startGeneration = 1 + linkIndex * 3;
    const targetNodeId = firstTargetNodeId + linkIndex;
    const hashBefore = linkIndex === 0 ? "" : (destinations[linkIndex - 1] ?? "");
    return {
      destination,
      hash: destination,
      scrollY,
      capturePath,
      captureSha256: sha256File(capturePath),
      targetVisible: true,
      hashBefore,
      hashAfter: destination,
      frame: { generation: startGeneration + 3, contentHeight, scrollY },
      pointerTrace: pointerTrace(targetNodeId, 20 + linkIndex * 10, {
        startGeneration,
        startScrollY: previousScrollY,
        finalScrollY: scrollY,
      }),
      sourceSha256: sourceSha,
    };
  });
  return { links, targetNodeId: firstTargetNodeId };
}

function pointerTrace(
  targetNodeId: number,
  coordinate: number,
  options: PointerTraceOptions,
): JsonValue[] {
  return ["move", "down", "up"].map((kind, index) => ({
    kind,
    targetNodeId,
    x: coordinate,
    y: coordinate,
    preFrame: {
      generation: options.startGeneration + index,
      scrollY: options.startScrollY,
    },
    postFrame: {
      generation: options.startGeneration + index + 1,
      scrollY: index === 2 ? options.finalScrollY : options.startScrollY,
    },
  }));
}

function scriptRealEventsFixture(sourceSha: string, selector: string | undefined): JsonObject {
  const initial = { result: "", key: "" };
  const shifted = { result: "", key: "Shift" };
  const expected = readExpectedState(
    path.resolve("tests/fixtures/html/aspects/script-events.html"),
  );
  return {
    schema: "krr-html84-operation-evidence-v1",
    kind: "script-real-events",
    status: "observed",
    sourceSha256Before: sourceSha,
    sourceSha256After: sourceSha,
    preFrame: { generation: 1, scrollY: 0 },
    postFrame: { generation: 10, scrollY: 0 },
    target: { selector: selector ?? "#field", nodeId: 2, visible: true },
    state: { before: initial, after: expected },
    steps: scriptRealEventSteps(initial, shifted),
    delivery: "session-load",
    events: scriptRealEventRecords(initial, shifted, expected),
  };
}

function scriptRealEventSteps(initial: JsonValue, shifted: JsonValue): JsonValue[] {
  return [
    ...pointerTrace(2, 20, { startGeneration: 1, startScrollY: 0, finalScrollY: 0 }),
    { kind: "text", targetNodeId: 2, text: "入力", stateBefore: initial, stateAfter: initial },
    { kind: "key_down", targetNodeId: 2, key: "Shift", stateBefore: initial, stateAfter: shifted },
    { kind: "key_up", targetNodeId: 2, key: "Shift", stateBefore: shifted, stateAfter: shifted },
    ...pointerTrace(4, 40, { startGeneration: 7, startScrollY: 0, finalScrollY: 0 }),
  ];
}

function scriptRealEventRecords(
  initial: JsonValue,
  shifted: JsonValue,
  expected: JsonValue,
): JsonValue[] {
  return [
    { type: "focus", targetNodeId: 2, focused: true, stateBefore: initial, stateAfter: initial },
    { type: "input", targetNodeId: 2, stateBefore: initial, stateAfter: initial },
    { type: "keydown", targetNodeId: 2, key: "Shift", stateBefore: initial, stateAfter: shifted },
    { type: "keyup", targetNodeId: 2, key: "Shift", stateBefore: shifted, stateAfter: shifted },
    { type: "click", targetNodeId: 4, stateBefore: shifted, stateAfter: expected },
    { type: "submit", targetNodeId: 3, stateBefore: shifted, stateAfter: expected },
  ];
}

function viewportPng(temporary: string, viewport: Viewport): string {
  const png = path.join(temporary, `${viewport.width}x${viewport.height}.png`);
  if (fs.existsSync(png)) return png;
  const result = Bun.spawnSync([
    "magick",
    "-size",
    `${viewport.width}x${viewport.height}`,
    "xc:white",
    png,
  ]);
  if (result.exitCode !== 0)
    throw new Error(`failed to create ${viewport.width}x${viewport.height} PNG`);
  return png;
}

test("accepts every catalog state and viewport as an independent stage condition", () => {
  const fixture = suiteFixture();
  try {
    expect(() =>
      assertStageScoreSuite(fixture.value as JsonValue, fixture.trustedPlan as JsonValue),
    ).not.toThrow();
  } finally {
    fixture.cleanup();
  }
}, 30000);

test("strict suite rejects missing, unknown, swapped, reused and candidate-sourced raw receipts", () => {
  const fixture = suiteFixture();
  try {
    const first = objectConditions(fixture.value)[0];
    const second = objectConditions(fixture.value)[1];
    const trustedFirst = objectConditions(fixture.trustedPlan)[0];
    const trustedSecond = objectConditions(fixture.trustedPlan)[1];
    if (!first || !second || !trustedFirst || !trustedSecond) throw new Error("missing test tuple");
    const missingReceipt = structuredClone(fixture.value);
    delete (objectConditions(missingReceipt)[0] as JsonObject).rawKrrReportPath;
    expect(() => assertStageScoreSuite(missingReceipt, fixture.trustedPlan as JsonValue)).toThrow(
      /KRR raw report path/,
    );
    const candidateSourced = structuredClone(fixture.value);
    (objectConditions(candidateSourced)[0] as JsonObject).chromeReceiptPath =
      "candidate-derived.json";
    expect(() => assertStageScoreSuite(candidateSourced, fixture.trustedPlan as JsonValue)).toThrow(
      /unknown candidate condition fields/,
    );
    const reused = structuredClone(fixture.trustedPlan);
    (objectConditions(reused)[1] as JsonObject).chromeReceiptSha256 =
      (objectConditions(reused)[0] as JsonObject).chromeReceiptSha256 ?? "";
    expect(() => assertStageScoreSuite(fixture.value as JsonValue, reused)).toThrow(
      /reused Chrome raw receipt SHA/,
    );
    const chromeUnknown = structuredClone(fixture.trustedPlan);
    const chromePath = objectConditions(chromeUnknown)[0]?.chromeReceiptPath as string;
    const originalChromeContents = fs.readFileSync(chromePath, "utf8");
    const originalChromeSha = sha256File(chromePath);
    const chromeReceipt = JSON.parse(originalChromeContents) as JsonObject;
    chromeReceipt.status = "unknown";
    chromeReceipt.unknownReason = "font source not physically bound";
    fs.writeFileSync(chromePath, JSON.stringify(chromeReceipt));
    (objectConditions(chromeUnknown)[0] as JsonObject).chromeReceiptSha256 = sha256File(chromePath);
    expect(() => assertStageScoreSuite(fixture.value as JsonValue, chromeUnknown)).toThrow(
      /identity differs/,
    );
    fs.writeFileSync(chromePath, originalChromeContents);
    (objectConditions(fixture.trustedPlan)[0] as JsonObject).chromeReceiptSha256 =
      originalChromeSha;
    const swappedChrome = structuredClone(fixture.trustedPlan);
    const firstChrome = objectConditions(swappedChrome)[0] as JsonObject;
    const secondChrome = objectConditions(swappedChrome)[1] as JsonObject;
    const firstChromePath = firstChrome.chromeReceiptPath as string;
    const secondChromePath = secondChrome.chromeReceiptPath as string;
    firstChrome.chromeReceiptPath = secondChromePath;
    firstChrome.chromeReceiptSha256 = sha256File(secondChromePath);
    secondChrome.chromeReceiptPath = firstChromePath;
    secondChrome.chromeReceiptSha256 = sha256File(firstChromePath);
    expect(() => assertStageScoreSuite(fixture.value as JsonValue, swappedChrome)).toThrow(
      /identity differs/,
    );
    const reusedNonce = structuredClone(fixture.value);
    const nonceConditions = objectConditions(reusedNonce);
    const firstRawPath = nonceConditions[0]?.rawKrrReportPath as string;
    const secondRawPath = nonceConditions[1]?.rawKrrReportPath as string;
    const firstRaw = JSON.parse(fs.readFileSync(firstRawPath, "utf8")) as JsonObject;
    const secondRaw = JSON.parse(fs.readFileSync(secondRawPath, "utf8")) as JsonObject;
    secondRaw.executionNonce = firstRaw.executionNonce as string;
    fs.writeFileSync(secondRawPath, JSON.stringify(secondRaw));
    (nonceConditions[1] as JsonObject).rawKrrReportSha256 = sha256File(secondRawPath);
    expect(() => assertStageScoreSuite(reusedNonce, fixture.trustedPlan as JsonValue)).toThrow(
      /reused KRR execution nonce/,
    );
    const swapped = structuredClone(fixture.value);
    const rawPath = (objectConditions(swapped)[0] as JsonObject).rawKrrReportPath as string;
    const otherPath = (objectConditions(swapped)[1] as JsonObject).rawKrrReportPath as string;
    (objectConditions(swapped)[0] as JsonObject).rawKrrReportPath = otherPath;
    (objectConditions(swapped)[0] as JsonObject).rawKrrReportSha256 = sha256File(otherPath);
    (objectConditions(swapped)[1] as JsonObject).rawKrrReportPath = rawPath;
    (objectConditions(swapped)[1] as JsonObject).rawKrrReportSha256 = sha256File(rawPath);
    expect(() => assertStageScoreSuite(swapped, fixture.trustedPlan as JsonValue)).toThrow(
      /identity mismatch/,
    );
    const unknown = structuredClone(fixture.value);
    const unknownPath = (objectConditions(unknown)[0] as JsonObject).rawKrrReportPath as string;
    const report = JSON.parse(fs.readFileSync(unknownPath, "utf8")) as JsonObject;
    report.status = "unknown";
    report.unknownReason = "observer API unavailable";
    report.operationEvidence = {
      schema: "krr-html84-operation-evidence-v1",
      kind: report.operationKind as string,
      status: "unknown",
      reason: report.unknownReason as string,
    };
    fs.writeFileSync(unknownPath, JSON.stringify(report));
    (objectConditions(unknown)[0] as JsonObject).rawKrrReportSha256 = sha256File(unknownPath);
    expect(() => assertStageScoreSuite(unknown, fixture.trustedPlan as JsonValue)).toThrow(
      /unknown/,
    );
  } finally {
    fixture.cleanup();
  }
});

test("rejects missing, duplicate, unrecognized, and mismatched condition provenance", () => {
  const fixture = suiteFixture();
  try {
    const conditions = fixture.value.conditions as JsonValue[];
    const first = conditions[0] as JsonObject;
    expect(() =>
      assertStageScoreSuite(
        { ...fixture.value, conditions: conditions.slice(1) } as JsonValue,
        fixture.trustedPlan as JsonValue,
      ),
    ).toThrow("missing suite tuple");
    expect(() =>
      assertStageScoreSuite(
        {
          ...fixture.value,
          conditions: [first, { ...first, conditionId: "different-id" }, ...conditions.slice(1)],
        } as JsonValue,
        fixture.trustedPlan as JsonValue,
      ),
    ).toThrow("duplicate suite tuple");
    expect(() =>
      assertStageScoreSuite(
        {
          ...fixture.value,
          conditions: conditions.map((condition, index) =>
            index === 0 ? { ...(condition as JsonObject), state: "unrecognized-state" } : condition,
          ),
        } as JsonValue,
        fixture.trustedPlan as JsonValue,
      ),
    ).toThrow("unrecognized suite tuple");
    expect(() =>
      assertStageScoreSuite(
        {
          ...fixture.value,
          conditions: conditions.map((condition, index) =>
            index === 0
              ? { ...(condition as JsonObject), sourceSha256: "0".repeat(64) }
              : condition,
          ),
        } as JsonValue,
        fixture.trustedPlan as JsonValue,
      ),
    ).toThrow("condition source SHA differs");
    expect(() =>
      assertStageScoreSuite(
        {
          ...fixture.value,
          conditions: conditions.map((condition, index) =>
            index === 0
              ? { ...(condition as JsonObject), binarySha256: "1".repeat(64) }
              : condition,
          ),
        } as JsonValue,
        fixture.trustedPlan as JsonValue,
      ),
    ).toThrow("condition binary differs");
    expect(() =>
      assertStageScoreSuite(
        {
          ...fixture.value,
          conditions: conditions.map((condition, index) =>
            index === 0
              ? {
                  ...(condition as JsonObject),
                  trusted: (fixture.trustedPlan.conditions as JsonValue[])[0],
                }
              : condition,
          ),
        } as JsonValue,
        fixture.trustedPlan as JsonValue,
      ),
    ).toThrow("candidate suite must not include trusted data");
  } finally {
    fixture.cleanup();
  }
}, 30000);
