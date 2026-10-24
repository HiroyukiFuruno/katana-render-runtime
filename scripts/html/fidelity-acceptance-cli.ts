import fs from "node:fs";
import path from "node:path";
import { z } from "zod";
import {
  catalogFontAcceptance,
  compareCatalogFontFaces,
  compareChromiumFontInstancesToLayoutRuns,
} from "./catalog-font-equivalence";
import { readChromiumFontBinding } from "./chromium-font-binding";
import { runDynamicFidelity, stableJson } from "./dynamic-fidelity";
import { type JsonValue, jsonRecords } from "./dynamic-state";
import { assertFeatureResultReportsComplete, loadFeatureCatalog } from "./feature-catalog";
import { assertInterimFeatureResultReportsComplete } from "./interim-feature-results";
import { sha256File } from "./reference-capture";
import {
  assertCatalogStage90ConditionPlan,
  assertStageScoreSuite,
  createCatalogStage90ConditionPlan,
  validateCatalogRawStage90Captures,
} from "./stage-score-suite";
import { tocBinaryFromBuildReport } from "./toc-fidelity";
import {
  type CatalogStage90ChromeExecutablePin,
  type CatalogStage90ResumeOptions,
  jsonResourceFiles,
  runCatalogStage90Captures,
  runInputStateFidelity,
} from "./visual-state-fidelity";

export { jsonResourceFiles };

const modeSchema = z.enum([
  "input-state-final",
  "input-state-interim",
  "dynamic-final",
  "dynamic-interim",
  "reports-final",
  "reports-interim",
  "catalog-stage90-plan",
  "catalog-stage90-capture",
  "catalog-stage90-validate",
  "catalog-stage90-validate-captures",
]);

type LegacyCaptureMode = Exclude<
  z.infer<typeof modeSchema>,
  | "reports-final"
  | "reports-interim"
  | "catalog-stage90-plan"
  | "catalog-stage90-capture"
  | "catalog-stage90-validate"
  | "catalog-stage90-validate-captures"
>;

interface CapturePlan {
  kind: "input-state" | "dynamic";
  buildReport: string;
  output: string;
  threshold: 99 | 90;
  options?: { phase: "interim" };
}

interface ReportsPlan {
  kind: "reports";
  phase: "final" | "interim";
  visualReport: string;
  scriptReport: string;
}

interface CatalogCapturePlan {
  kind: "catalog-stage90-capture";
  buildReport: string;
  conditionPlan: string;
  output: string;
  threshold: 90;
  acceptance: false;
  resume?: CatalogStage90ResumeOptions;
  chromeExecutable?: CatalogStage90ChromeExecutablePin;
}

interface CatalogPlanWrite {
  kind: "catalog-stage90-plan";
  output: string;
  acceptance: false;
}

interface CatalogValidatePlan {
  kind: "catalog-stage90-validate";
  candidateSuite: string;
  trustedPlan: string;
}

interface CatalogValidateCapturesPlan {
  kind: "catalog-stage90-validate-captures";
  captureManifest: string;
  conditionPlan: string;
}

interface CatalogFontProbeValidation {
  report: JsonValue;
  request: PinnedProbeFile;
  response: PinnedProbeFile;
  requestCaptures: Array<{ [key: string]: JsonValue }>;
  responseCaptures: Array<{ [key: string]: JsonValue }>;
}

interface PinnedProbeFile {
  path: string;
  sha256: string;
  size?: number;
}

export type FidelityAcceptancePlan =
  | CapturePlan
  | ReportsPlan
  | CatalogPlanWrite
  | CatalogCapturePlan
  | CatalogValidatePlan
  | CatalogValidateCapturesPlan;

const capturePlans: Record<LegacyCaptureMode, CapturePlan> = {
  "input-state-final": {
    kind: "input-state",
    buildReport: "tmp/html-fidelity/input-state/test-build.jsonl",
    output: "tmp/html-fidelity/input-state",
    threshold: 99,
  },
  "input-state-interim": {
    kind: "input-state",
    buildReport: "tmp/html-fidelity/input-state-interim/test-build.jsonl",
    output: "tmp/html-fidelity/input-state-interim",
    threshold: 90,
    options: { phase: "interim" },
  },
  "dynamic-final": {
    kind: "dynamic",
    buildReport: "tmp/html-fidelity/dynamic/test-build.jsonl",
    output: "tmp/html-fidelity/dynamic",
    threshold: 99,
  },
  "dynamic-interim": {
    kind: "dynamic",
    buildReport: "tmp/html-fidelity/dynamic-interim/test-build.jsonl",
    output: "tmp/html-fidelity/dynamic-interim",
    threshold: 90,
    options: { phase: "interim" },
  },
};

export function fidelityAcceptancePlan(argv: readonly string[]): FidelityAcceptancePlan {
  const mode = modeSchema.parse(argv[0]);
  const catalogPlan = catalogModePlan(mode, argv);
  if (catalogPlan) return catalogPlan;
  const reportPlan = reportsModePlan(mode, argv);
  if (reportPlan) return reportPlan;
  const plan = capturePlans[mode as LegacyCaptureMode];
  if (!plan || argv.length !== 2 || argv[1] !== plan.buildReport)
    throw new Error("capture acceptance mode requires its fixed build report argument");
  return plan.options ? { ...plan, options: { ...plan.options } } : { ...plan };
}

function catalogModePlan(
  mode: z.infer<typeof modeSchema>,
  argv: readonly string[],
): FidelityAcceptancePlan | undefined {
  if (mode === "catalog-stage90-plan") {
    const output = argv[1];
    if (argv.length !== 2 || !output)
      throw new Error("catalog stage90 plan requires one output path");
    return { kind: mode, output, acceptance: false };
  }
  if (mode === "catalog-stage90-capture" || mode === "catalog-stage90-validate")
    return catalogAcceptancePlan(mode, argv);
  if (mode === "catalog-stage90-validate-captures") {
    if (argv.length !== 3 || argv.slice(1).some((argument) => argument.length === 0))
      throw new Error("catalog capture validation requires a capture manifest and condition plan");
    return {
      kind: mode,
      captureManifest: argv[1] as string,
      conditionPlan: argv[2] as string,
    };
  }
  return undefined;
}

function reportsModePlan(
  mode: z.infer<typeof modeSchema>,
  argv: readonly string[],
): ReportsPlan | undefined {
  if (mode !== "reports-final" && mode !== "reports-interim") return undefined;
  if (argv.length !== 1) throw new Error("report acceptance mode takes no additional arguments");
  return mode === "reports-final"
    ? {
        kind: "reports",
        phase: "final",
        visualReport: "tmp/html-fidelity/visual-feature-results.json",
        scriptReport: "tmp/html-fidelity/dynamic/script-feature-results.json",
      }
    : {
        kind: "reports",
        phase: "interim",
        visualReport: "tmp/html-fidelity/interim/interim-visual-feature-results.json",
        scriptReport: "tmp/html-fidelity/dynamic-interim/interim-script-feature-results.json",
      };
}

function catalogAcceptancePlan(
  mode: "catalog-stage90-capture" | "catalog-stage90-validate",
  argv: readonly string[],
): FidelityAcceptancePlan {
  if (mode === "catalog-stage90-capture") {
    return catalogCapturePlan(argv);
  }
  if (argv.length !== 3 || argv.slice(1).some((argument) => argument.length === 0))
    throw new Error(
      "catalog stage90 validation requires candidate suite and independent trusted plan",
    );
  return {
    kind: "catalog-stage90-validate",
    candidateSuite: argv[1] as string,
    trustedPlan: argv[2] as string,
  };
}

function catalogCapturePlan(argv: readonly string[]): CatalogCapturePlan {
  if (argv.includes("--font-observer")) return catalogObserverCapturePlan(argv);
  if (
    ![4, 6, 8, 10].includes(argv.length) ||
    argv.slice(1).some((argument) => argument.length === 0)
  )
    throw new Error(
      "catalog capture requires build report, exact plan, output, optional resume manifests, and optional Chrome executable path/SHA",
    );
  const chromeOptions = argv.length >= 8;
  if (
    argv.length === 6 &&
    (argv[4] === "--chrome-executable" ||
      argv[5] === "--chrome-executable-sha256" ||
      (path.isAbsolute(argv[4] as string) && /^[a-f0-9]{64}$/.test(argv[5] as string)))
  )
    throw new Error("catalog Chrome executable options require both explicit path/SHA flags");
  if (
    chromeOptions &&
    (argv[argv.length - 4] !== "--chrome-executable" ||
      argv[argv.length - 2] !== "--chrome-executable-sha256")
  )
    throw new Error("catalog Chrome executable options must use the explicit path/SHA flags");
  return {
    kind: "catalog-stage90-capture",
    buildReport: argv[1] as string,
    conditionPlan: argv[2] as string,
    output: argv[3] as string,
    threshold: 90,
    acceptance: false,
    ...(argv.length === 6 || argv.length === 10
      ? {
          resume: {
            observationManifestPath: argv[4] as string,
            finalizedManifestPath: argv[5] as string,
          },
        }
      : {}),
    ...(chromeOptions
      ? {
          chromeExecutable: parseCatalogChromeExecutable(
            argv[argv.length - 3] as string,
            argv[argv.length - 1] as string,
          ),
        }
      : {}),
  };
}

function catalogObserverCapturePlan(argv: readonly string[]): CatalogCapturePlan {
  if (
    argv.at(-1) !== "--font-observer" ||
    argv.filter((value) => value === "--font-observer").length !== 1
  )
    throw new Error("font observer requires one final explicit flag");
  const plan = catalogCapturePlan(argv.slice(0, -1));
  if (!plan.chromeExecutable) throw new Error("font observer requires a pinned executable");
  return { ...plan, chromeExecutable: { ...plan.chromeExecutable, fontObserver: true } };
}

export async function runFidelityAcceptance(argv: readonly string[]): Promise<void> {
  const plan = fidelityAcceptancePlan(argv);
  if (plan.kind === "reports") {
    const catalog = loadFeatureCatalog();
    if (plan.phase === "final")
      assertFeatureResultReportsComplete(catalog, plan.visualReport, plan.scriptReport);
    else assertInterimFeatureResultReportsComplete(catalog, plan.visualReport, plan.scriptReport);
    return;
  }
  if (plan.kind === "catalog-stage90-validate") {
    const candidate = JSON.parse(fs.readFileSync(plan.candidateSuite, "utf8")) as JsonValue;
    const trusted = JSON.parse(fs.readFileSync(plan.trustedPlan, "utf8")) as JsonValue;
    assertStageScoreSuite(candidate, trusted);
    return;
  }
  if (plan.kind === "catalog-stage90-validate-captures") {
    const envelope = validateCatalogChromeCaptures(plan.captureManifest, plan.conditionPlan);
    process.stdout.write(`${JSON.stringify(envelope)}\n`);
    return;
  }
  if (plan.kind === "catalog-stage90-plan") {
    writeCatalogStage90Plan(plan.output);
    return;
  }
  if (plan.kind === "catalog-stage90-capture") {
    const binary = tocBinaryFromBuildReport(plan.buildReport);
    await runCatalogStage90Captures(
      binary,
      plan.conditionPlan,
      plan.output,
      plan.resume,
      plan.chromeExecutable,
    );
    return;
  }
  const binary = tocBinaryFromBuildReport(plan.buildReport);
  const capture = plan.kind === "input-state" ? runInputStateFidelity : runDynamicFidelity;
  if (plan.options) await capture(binary, plan.output, plan.threshold, plan.options);
  else await capture(binary, plan.output, plan.threshold);
}

function parseCatalogChromeExecutable(
  executablePath: string,
  executableSha256: string,
): CatalogStage90ChromeExecutablePin {
  if (!path.isAbsolute(executablePath) || !/^[a-f0-9]{64}$/.test(executableSha256))
    throw new Error("catalog Chrome executable option requires an absolute path and SHA-256 pin");
  return { executablePath, executableSha256 };
}

function writeCatalogStage90Plan(output: string): void {
  const serialized = JSON.stringify(createCatalogStage90ConditionPlan(), null, 2);
  assertCatalogStage90ConditionPlan(JSON.parse(serialized) as JsonValue);
  fs.writeFileSync(output, `${serialized}\n`, { flag: "wx" });
  const written = JSON.parse(fs.readFileSync(output, "utf8")) as JsonValue;
  assertCatalogStage90ConditionPlan(written);
}

if (import.meta.main) await runFidelityAcceptance(process.argv.slice(2));

export function validateCatalogChromeCaptures(
  captureManifestPath: string,
  conditionPlanPath: string,
): JsonValue {
  const { manifest, manifestPath, manifestSha256, plan, planPath, planSha256 } =
    readCatalogChromeInputs(captureManifestPath, conditionPlanPath);
  const captures = jsonRecords(manifest.conditions, "capture conditions");
  const scores = validateCatalogRawStage90Captures(
    manifest,
    JSON.parse(fs.readFileSync(planPath, "utf8")) as JsonValue,
    planPath,
  );
  const fontProbe = validateCatalogFontProbe(manifest);
  const capturesById = new Map(
    captures.map((capture) => [jsonText(capture.conditionId, "condition id"), capture]),
  );
  const validated = scores.map((score) => {
    const capture = capturesById.get(score.conditionId);
    if (!capture) throw new Error(`catalog capture is missing: ${score.conditionId}`);
    return validatedCatalogScoreCondition(capture, score);
  });
  const fontEquivalence = validateCatalogFontEquivalence(
    plan.conditions.map((condition) => condition.conditionId),
    captures,
    fontProbe,
  );
  const acceptance = catalogFontAcceptance(
    scores.map((score) => score.score),
    plan.conditions.length,
    fontEquivalence.established,
    fontEquivalence.rejected,
  );
  return {
    schema: "krr-html84-independent-stage90-validation-v1",
    acceptance: acceptance.acceptance,
    threshold: 90,
    measurement: "not-performed",
    fontIdentityEquivalence: acceptance.fontIdentityEquivalence,
    fontIdentityDiagnostics: fontEquivalence.rejected,
    fontIdentityProbe: fontProbe.report,
    manifestPath,
    manifestSha256,
    planPath,
    planSha256,
    conditions: validated,
  } as unknown as JsonValue;
}

function validateCatalogFontProbe(manifest: {
  [key: string]: JsonValue;
}): CatalogFontProbeValidation {
  const probe = jsonRecord(manifest.fontIdentityProbe ?? null, "font identity probe pins");
  const request = validateProbeFile(probe.request, "font identity request", true);
  const response = validateProbeFile(probe.response, "font identity response", true);
  const binary = validateProbeFile(probe.binary, "font identity probe binary", false);
  const requestBody = jsonRecord(
    JSON.parse(
      fs.readFileSync(jsonText(request.path, "font identity request path"), "utf8"),
    ) as JsonValue,
    "font identity request",
  );
  if (requestBody.schema !== "krr-chrome-font-identity-request-v1")
    throw new Error("font identity request schema is invalid");
  const requestCaptures = jsonRecords(requestBody.captures, "font identity request captures");
  const responseBody = jsonRecord(
    JSON.parse(
      fs.readFileSync(jsonText(response.path, "font identity response path"), "utf8"),
    ) as JsonValue,
    "font identity response",
  );
  if (
    responseBody.schema !== "krr-chrome-font-identity-response-v1" ||
    responseBody.requestSha256 !== jsonText(request.sha256, "font identity request SHA")
  )
    throw new Error("font identity response does not bind its request");
  const report = {
    request,
    response,
    binary,
    inventoryMethod: jsonText(responseBody.inventoryMethod, "font inventory method"),
    inventoryLimit: jsonText(responseBody.inventoryLimit, "font inventory limit"),
  } as unknown as JsonValue;
  return {
    report,
    request,
    response,
    requestCaptures,
    responseCaptures: jsonRecords(responseBody.captures, "font identity response captures"),
  };
}

function validateCatalogFontEquivalence(
  conditionIds: readonly string[],
  captures: readonly { [key: string]: JsonValue }[],
  probe: CatalogFontProbeValidation,
): { established: boolean; rejected: Array<{ conditionId: string; reason: string }> } {
  const rejected: Array<{ conditionId: string; reason: string }> = [];
  if (conditionIds.length !== 84 || new Set(conditionIds).size !== conditionIds.length)
    return rejectAllFontConditions(conditionIds, "condition plan is incomplete or duplicated");
  let indexed: FontIdentityConditionIndex;
  try {
    indexed = indexFontIdentityConditions(captures, probe);
  } catch {
    return rejectAllFontConditions(
      conditionIds,
      "font identity condition records are duplicated or invalid",
    );
  }
  if (
    indexed.captures.size !== conditionIds.length ||
    indexed.requests.size !== conditionIds.length ||
    indexed.responses.size !== conditionIds.length ||
    !sameConditionOrder(captures, conditionIds) ||
    !sameConditionOrder(probe.requestCaptures, conditionIds) ||
    !sameConditionOrder(probe.responseCaptures, conditionIds)
  )
    return rejectAllFontConditions(conditionIds, "font identity condition coverage is incomplete");

  for (const conditionId of conditionIds) {
    const capture = indexed.captures.get(conditionId);
    const requestCapture = indexed.requests.get(conditionId);
    const responseCapture = indexed.responses.get(conditionId);
    if (!capture || !requestCapture || !responseCapture) {
      rejected.push({ conditionId, reason: "font identity condition is missing" });
      continue;
    }
    const reason = validateFontIdentityCondition(
      conditionId,
      capture,
      requestCapture,
      responseCapture,
      probe,
    );
    if (reason !== null) rejected.push({ conditionId, reason });
  }
  return { established: rejected.length === 0, rejected };
}

type FontIdentityConditionIndex = {
  captures: Map<string, { [key: string]: JsonValue }>;
  requests: Map<string, { [key: string]: JsonValue }>;
  responses: Map<string, { [key: string]: JsonValue }>;
};

function rejectAllFontConditions(conditionIds: readonly string[], reason: string) {
  return {
    established: false,
    rejected: conditionIds.map((conditionId) => ({ conditionId, reason })),
  };
}

function indexFontIdentityConditions(
  captures: readonly { [key: string]: JsonValue }[],
  probe: CatalogFontProbeValidation,
): FontIdentityConditionIndex {
  return {
    captures: indexFontIdentityRows(captures, "capture"),
    requests: indexFontIdentityRows(probe.requestCaptures, "request"),
    responses: indexFontIdentityRows(probe.responseCaptures, "response"),
  };
}

function indexFontIdentityRows(
  rows: readonly { [key: string]: JsonValue }[],
  label: string,
): Map<string, { [key: string]: JsonValue }> {
  const result = new Map<string, { [key: string]: JsonValue }>();
  for (const row of rows) {
    const conditionId = jsonText(row.conditionId, `${label} condition id`);
    if (result.has(conditionId)) throw new Error(`${label} conditions are duplicated`);
    result.set(conditionId, row);
  }
  return result;
}

function validateFontIdentityCondition(
  conditionId: string,
  capture: { [key: string]: JsonValue },
  requestCapture: { [key: string]: JsonValue },
  responseCapture: { [key: string]: JsonValue },
  probe: CatalogFontProbeValidation,
): string | null {
  try {
    assertFontIdentityRequestBinding(conditionId, capture, requestCapture, responseCapture, probe);
    const reason = compareFontIdentityReceipts(conditionId, capture, responseCapture);
    return reason;
  } catch (error) {
    return error instanceof Error ? error.message : "font identity is invalid";
  }
}

function readFontReadiness(value: JsonValue | undefined): { status: "loaded" } {
  return z
    .object({ status: z.literal("loaded") })
    .strict()
    .parse(value);
}

function assertFontIdentityRequestBinding(
  conditionId: string,
  capture: { [key: string]: JsonValue },
  requestCapture: { [key: string]: JsonValue },
  responseCapture: { [key: string]: JsonValue },
  probe: CatalogFontProbeValidation,
): void {
  const observationValue = capture.fontIdentityObservation;
  const resourceValue = capture.resourceProvenance;
  if (observationValue === undefined || resourceValue === undefined)
    throw new Error("font observation or resource provenance is missing");
  const observation = jsonRecord(observationValue, "font observation");
  const resources = jsonRecord(resourceValue, "resource provenance");
  readFontReadiness(capture.fontReadiness);
  if (
    capture.status !== "captured" ||
    capture.fontIdentityRequestConditionId !== conditionId ||
    capture.fontIdentityRequestPath !== probe.request.path ||
    capture.fontIdentityRequestSha256 !== probe.request.sha256 ||
    capture.fontIdentityRequestSizeBytes !== probe.request.size ||
    capture.fontIdentityResponsePath !== probe.response.path ||
    capture.fontIdentityResponseSha256 !== probe.response.sha256 ||
    capture.fontIdentityResponseSizeBytes !== probe.response.size ||
    observation.status !== "observed" ||
    resources.inventoryComplete !== true ||
    !hasNoStringReasons(capture.fontIdentityUnknownReasons) ||
    !hasNoStringReasons(observation.unknownReasons) ||
    !sameJson(requestCapture.selectedFonts, observation.selectedFonts) ||
    !sameJson(
      requestCapture.resourceFiles,
      jsonResourceFiles(resources.requests) as unknown as JsonValue,
    ) ||
    jsonRecords(requestCapture.selectedFonts, "requested selected fonts").length !==
      jsonRecords(responseCapture.fontProvenance, "response font provenance").length ||
    responseCapture.status !== "resolved" ||
    !hasNoStringReasons(responseCapture.unknownReasons)
  )
    throw new Error("font identity request/response binding is not established");
}

function compareFontIdentityReceipts(
  conditionId: string,
  capture: { [key: string]: JsonValue },
  responseCapture: { [key: string]: JsonValue },
): string | null {
  const krr = readPinnedJsonRecord(
    jsonText(capture.krrReportPath, "KRR report path"),
    jsonText(capture.krrReportSha256, "KRR report SHA"),
  );
  const chrome = readPinnedJsonRecord(
    jsonText(capture.chromeReceiptPath, "Chrome receipt path"),
    jsonText(capture.chromeReceiptSha256, "Chrome receipt SHA"),
  );
  const captureReadiness = readFontReadiness(capture.fontReadiness);
  const chromeReadiness = readFontReadiness(chrome.fontReadiness);
  if (
    krr.conditionId !== conditionId ||
    chrome.conditionId !== conditionId ||
    chrome.status !== "observed" ||
    !sameJson(captureReadiness, chromeReadiness) ||
    !sameJson(chrome.resourceProvenance, capture.resourceProvenance) ||
    !sameJson(chrome.fontProvenance, responseCapture.fontProvenance)
  )
    throw new Error("font identity response differs from the strict Chrome receipt");
  const comparison =
    chrome.traceBinding === undefined
      ? compareCatalogFontFaces(krr.fontProvenance, chrome.fontProvenance)
      : compareChromiumFontInstancesToLayoutRuns(
          readChromiumFontBinding(chrome.traceBinding, {
            conditionId,
            sourceSha256: jsonText(capture.sourceSha256, "condition source SHA"),
            viewport: z
              .object({ width: z.number(), height: z.number(), scale: z.number() })
              .strict()
              .parse(chrome.viewport),
            browserExecutablePath: jsonText(chrome.browserExecutablePath, "Chrome executable"),
            browserExecutableSha256: jsonText(
              chrome.browserExecutableSha256,
              "Chrome executable SHA",
            ),
            capturePath: jsonText(chrome.capturePath, "Chrome capture"),
            captureSha256: jsonText(chrome.captureSha256, "Chrome capture SHA"),
          }).instances,
          krr.fontProvenance,
        );
  return comparison.established
    ? null
    : (comparison.reason ?? "physical font equivalence is not established");
}

function readPinnedJsonRecord(
  filePath: string,
  expectedSha256: string,
): { [key: string]: JsonValue } {
  if (
    path.resolve(filePath) !== filePath ||
    fs.realpathSync(filePath) !== filePath ||
    fs.lstatSync(filePath).isSymbolicLink() ||
    !fs.statSync(filePath).isFile() ||
    sha256File(filePath) !== expectedSha256
  )
    throw new Error("font receipt physical pin differs");
  return jsonRecord(JSON.parse(fs.readFileSync(filePath, "utf8")) as JsonValue, "font receipt");
}

function sameJson(left: JsonValue | undefined, right: JsonValue | undefined): boolean {
  return left !== undefined && right !== undefined && stableJson(left) === stableJson(right);
}

function hasNoStringReasons(value: JsonValue | undefined): boolean {
  return Array.isArray(value) && value.length === 0;
}

function sameConditionOrder(
  rows: readonly { [key: string]: JsonValue }[],
  expected: readonly string[],
): boolean {
  return (
    rows.length === expected.length &&
    rows.every((row, index) => row.conditionId === expected[index])
  );
}

function validateProbeFile(
  value: JsonValue | undefined,
  name: string,
  withSize: boolean,
): PinnedProbeFile {
  const pin = jsonRecord(value ?? null, name);
  const filePath = jsonText(pin.path, `${name} path`);
  const checksum = jsonText(pin.sha256, `${name} SHA`);
  const resolved = path.resolve(filePath);
  if (
    resolved !== filePath ||
    fs.realpathSync(filePath) !== filePath ||
    !fs.statSync(filePath).isFile()
  )
    throw new Error(`${name} is not a canonical regular file`);
  if (sha256File(filePath) !== checksum) throw new Error(`${name} SHA differs`);
  if (withSize) {
    const size = pin.sizeBytes;
    if (
      typeof size !== "number" ||
      !Number.isSafeInteger(size) ||
      size < 0 ||
      size !== fs.statSync(filePath).size
    )
      throw new Error(`${name} size differs`);
    return { path: filePath, sha256: checksum, size };
  }
  return { path: filePath, sha256: checksum };
}

function readCatalogChromeInputs(captureManifestPath: string, conditionPlanPath: string) {
  const manifestPath = fs.realpathSync(captureManifestPath);
  const planPath = fs.realpathSync(conditionPlanPath);
  const manifestSha256 = sha256File(manifestPath);
  const planSha256 = sha256File(planPath);
  const plan = assertCatalogStage90ConditionPlan(
    JSON.parse(fs.readFileSync(planPath, "utf8")) as JsonValue,
  );
  const manifest = jsonRecord(
    JSON.parse(fs.readFileSync(manifestPath, "utf8")) as JsonValue,
    "capture manifest",
  );
  if (
    manifest.profile !== "catalog-stage90-captures" ||
    manifest.acceptance !== false ||
    manifest.planPath !== planPath ||
    manifest.planSha256 !== planSha256 ||
    manifest.catalogPath !== plan.catalogPath ||
    manifest.catalogSha256 !== plan.catalogSha256
  )
    throw new Error("catalog Chrome capture manifest/plan binding differs");
  return {
    manifest,
    manifestPath,
    manifestSha256,
    plan,
    planPath,
    planSha256,
  };
}

function validatedCatalogScoreCondition(
  capture: { [key: string]: JsonValue },
  score: ReturnType<typeof validateCatalogRawStage90Captures>[number],
): JsonValue {
  if (capture.status !== "captured")
    throw new Error(`catalog capture is not verified: ${score.conditionId}`);
  const capturePath = path.resolve(jsonText(capture.chromeCapturePath, "Chrome capture path"));
  const receiptPath = path.resolve(jsonText(capture.chromeReceiptPath, "Chrome receipt path"));
  if (
    capturePath !== score.chromeCapturePath ||
    jsonText(capture.chromeCaptureSha256, "Chrome capture SHA") !== score.chromeCaptureSha256 ||
    receiptPath !== score.chromeReceiptPath ||
    jsonText(capture.chromeReceiptSha256, "Chrome receipt SHA") !== score.chromeReceiptSha256 ||
    path.resolve(jsonText(capture.krrCapturePath, "KRR capture path")) !== score.krrCapturePath ||
    jsonText(capture.krrCaptureSha256, "KRR capture SHA") !== score.krrCaptureSha256
  )
    throw new Error(`catalog capture score binding differs: ${score.conditionId}`);
  return {
    conditionId: score.conditionId,
    capturePath,
    captureSha256: score.chromeCaptureSha256,
    captureSize: fs.statSync(capturePath).size,
    receiptPath,
    receiptSha256: score.chromeReceiptSha256,
    receiptSize: fs.statSync(receiptPath).size,
    krrCapturePath: score.krrCapturePath,
    krrCaptureSha256: score.krrCaptureSha256,
    krrCaptureSize: fs.statSync(score.krrCapturePath).size,
    score: score.score,
    rmseScore: score.rmseScore,
    maeScore: score.maeScore,
    state: score.state,
  } as unknown as JsonValue;
}

function jsonRecord(value: JsonValue, name: string): { [key: string]: JsonValue } {
  if (typeof value !== "object" || value === null || Array.isArray(value))
    throw new Error(`invalid ${name}`);
  return value;
}

function jsonText(value: JsonValue | undefined, name: string): string {
  if (typeof value !== "string" || value.length === 0) throw new Error(`invalid ${name}`);
  return value;
}
