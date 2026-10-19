import fs from "node:fs";
import path from "node:path";
import { z } from "zod";
import { runDynamicFidelity } from "./dynamic-fidelity";
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
  type CatalogStage90ResumeOptions,
  runCatalogStage90Captures,
  runInputStateFidelity,
} from "./visual-state-fidelity";

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
  if (![4, 6].includes(argv.length) || argv.slice(1).some((argument) => argument.length === 0))
    throw new Error(
      "catalog capture requires build report, exact plan, output, and optionally both prior observation and finalized manifests",
    );
  return {
    kind: "catalog-stage90-capture",
    buildReport: argv[1] as string,
    conditionPlan: argv[2] as string,
    output: argv[3] as string,
    threshold: 90,
    acceptance: false,
    ...(argv.length === 6
      ? {
          resume: {
            observationManifestPath: argv[4] as string,
            finalizedManifestPath: argv[5] as string,
          },
        }
      : {}),
  };
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
    await runCatalogStage90Captures(binary, plan.conditionPlan, plan.output, plan.resume);
    return;
  }
  const binary = tocBinaryFromBuildReport(plan.buildReport);
  const capture = plan.kind === "input-state" ? runInputStateFidelity : runDynamicFidelity;
  if (plan.options) await capture(binary, plan.output, plan.threshold, plan.options);
  else await capture(binary, plan.output, plan.threshold);
}

function writeCatalogStage90Plan(output: string): void {
  const serialized = JSON.stringify(createCatalogStage90ConditionPlan(), null, 2);
  assertCatalogStage90ConditionPlan(JSON.parse(serialized) as JsonValue);
  fs.writeFileSync(output, `${serialized}\n`, { flag: "wx" });
  const written = JSON.parse(fs.readFileSync(output, "utf8")) as JsonValue;
  assertCatalogStage90ConditionPlan(written);
}

if (import.meta.main) await runFidelityAcceptance(process.argv.slice(2));

function validateCatalogChromeCaptures(
  captureManifestPath: string,
  conditionPlanPath: string,
): JsonValue {
  const { manifest, manifestPath, manifestSha256, planPath, planSha256 } = readCatalogChromeInputs(
    captureManifestPath,
    conditionPlanPath,
  );
  const fontIdentityProbe = validateCatalogFontProbe(manifest);
  const captures = jsonRecords(manifest.conditions, "capture conditions");
  const scores = validateCatalogRawStage90Captures(
    manifest,
    JSON.parse(fs.readFileSync(planPath, "utf8")) as JsonValue,
    planPath,
  );
  const capturesById = new Map(
    captures.map((capture) => [jsonText(capture.conditionId, "condition id"), capture]),
  );
  const validated = scores.map((score) => {
    const capture = capturesById.get(score.conditionId);
    if (!capture) throw new Error(`catalog capture is missing: ${score.conditionId}`);
    return validatedCatalogScoreCondition(capture, score);
  });
  return {
    schema: "krr-html84-independent-stage90-validation-v1",
    acceptance: false,
    threshold: 90,
    measurement: "not-performed",
    fontIdentityEquivalence: "not-established",
    fontIdentityProbe,
    manifestPath,
    manifestSha256,
    planPath,
    planSha256,
    conditions: validated,
  } as unknown as JsonValue;
}

function validateCatalogFontProbe(manifest: { [key: string]: JsonValue }): JsonValue {
  const probe = jsonRecord(manifest.fontIdentityProbe ?? null, "font identity probe pins");
  const request = validateProbeFile(probe.request, "font identity request", true);
  const response = validateProbeFile(probe.response, "font identity response", true);
  const binary = validateProbeFile(probe.binary, "font identity probe binary", false);
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
  return {
    request,
    response,
    binary,
    inventoryMethod: jsonText(responseBody.inventoryMethod, "font inventory method"),
    inventoryLimit: jsonText(responseBody.inventoryLimit, "font inventory limit"),
  } as unknown as JsonValue;
}

function validateProbeFile(
  value: JsonValue | undefined,
  name: string,
  withSize: boolean,
): { [key: string]: JsonValue } {
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
