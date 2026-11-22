import { OBSERVER_SCALER_REJECTION_CODES } from "./chromium-font-trace";
import type { JsonValue } from "./dynamic-state";

const CAPTURE_EXCEPTION = "capture-exception";
const MISSING_KRR_CAPTURE = "missing-krr-capture";
const RESOURCE_OR_CDP_PROBLEM = "resource-or-cdp-problem";
const REJECTED_OBSERVER = "rejected-observer";
const KRR_CHROME_MISMATCH = "krr-chrome-mismatch";
const GENERIC_VERIFICATION_REJECTION = "generic-verification-rejection";

const OBSERVER_REJECTION_REASONS = new Set([
  "Chromium font observation physical pin differs",
  "Chromium font observation condition binding differs",
  "Chromium font observation process executable differs",
  "Chromium font observation process arguments differ",
  "font observer event has no args object",
  "font observer event schema mismatch",
  "font observer event process binding mismatch",
  "used strike has no joined scaler",
  "used strike has incomplete retained paint",
  "invalid trace contract",
  "trace SHA256 mismatch",
  "no joined retained-paint font instance",
]);

const KRR_CHROME_MISMATCH_REASONS = new Set([
  "CoreText PostScript names differ from the ordered selected-font request",
  "font identity response differs from the strict Chrome receipt",
  "Chrome and KRR state differ after font provenance resolution",
  "matching physical font has no observed layout run",
  "Chromium font instance is incomplete or invalid",
  "applied design coordinates differ",
  "Chrome and KRR font instance sets differ",
  "selected physical font set is empty",
  "variable font instance identity is not established",
  "physical font file and face identities differ",
  "physical font provenance is incomplete or invalid",
]);

type FixedReasonCategory =
  | typeof CAPTURE_EXCEPTION
  | typeof MISSING_KRR_CAPTURE
  | typeof RESOURCE_OR_CDP_PROBLEM
  | typeof REJECTED_OBSERVER
  | typeof KRR_CHROME_MISMATCH
  | typeof GENERIC_VERIFICATION_REJECTION;

type JsonRecord = Record<string, JsonValue>;

const DIAGNOSTIC_MARKERS = [
  ["used strike has no joined scaler", "observer-strike-missing-scaler"],
  ["used strike has incomplete retained paint", "observer-incomplete-retained-paint"],
  ["no joined retained-paint font instance", "observer-no-font-instance"],
  ["font observer event schema mismatch", "observer-event-schema"],
  ["font observer event process binding mismatch", "observer-event-process-binding"],
  ["observer Chrome evidence validation failed:", "observer-evidence-validation"],
  ["page.goto:", "chrome-navigation"],
  ["page.screenshot:", "chrome-screenshot"],
  ["page.evaluate:", "chrome-page-evaluation"],
  ["locator.click:", "chrome-click"],
  ["observer Chrome capture failed:", "observer-capture"],
  ["test result: FAILED.", "krr-native-test-failed"],
  ["Catalog target geometry is missing:", "krr-target-geometry"],
  ["Operation evidence does not prove", "krr-operation-evidence"],
  ["actual font audit is incomplete", "krr-font-audit-incomplete"],
  ["no exact physical face matched the CDP PostScript name", "font-probe-no-physical-face"],
  ["multiple physical file/face-index matches are ambiguous", "font-probe-ambiguous-physical-face"],
  ["font face is memory-only, not physically attributable", "font-probe-memory-only-face"],
  [
    "selected family has no uniquely indexed physical font file",
    "font-probe-unindexed-system-font",
  ],
  ["selected family has ambiguous physical face indices", "font-probe-ambiguous-face-index"],
  ["shared font bytes differ from the physical file", "font-probe-font-bytes-differ"],
  ["fontdb face bytes differ from the physical file", "font-probe-font-bytes-differ"],
  ["font file changed while hashing", "font-probe-font-bytes-differ"],
  ["font source is not a regular file", "font-probe-not-regular-file"],
  ["canonicalize font:", "font-probe-file-unavailable"],
  ["read font:", "font-probe-file-unavailable"],
  ["stat font:", "font-probe-file-unavailable"],
  ["no exact URL-bound CoreText descriptor matched", "font-probe-no-coretext-match"],
  ["multiple URL-bound CoreText physical face matches", "font-probe-ambiguous-coretext-face"],
  ["CoreText URL descriptors expose no collection index", "font-probe-collection-index"],
  ["CoreText URL descriptors cannot prove nonzero face index", "font-probe-collection-index"],
  [
    "CoreText URL descriptors cannot prove a nonzero collection face index",
    "font-probe-collection-index",
  ],
  ["Chrome and KRR state differ after font provenance resolution", "chrome-krr-state-mismatch"],
  ["physical font file and face identities differ", "chrome-krr-face-mismatch"],
  ["Chrome and KRR font instance sets differ", "chrome-krr-instance-mismatch"],
  ["applied design coordinates differ", "chrome-krr-design-coordinate-mismatch"],
] as const;

const FONT_RECONCILIATION_REJECTION_STAGES = [
  "krr-observation-unknown",
  "font-reconciliation-request-binding",
  "font-reconciliation-resource-inventory",
  "font-reconciliation-selected-fonts",
  "font-reconciliation-probe-unresolved",
  "font-reconciliation-probe-result",
  "font-reconciliation-resource-digest",
  "font-reconciliation-provenance",
  "font-reconciliation-krr-receipt",
  "font-reconciliation-chrome-receipt",
  "font-reconciliation-state",
] as const;

export type FontReconciliationRejectionStage =
  (typeof FONT_RECONCILIATION_REJECTION_STAGES)[number];

type FailureDetail =
  | (typeof DIAGNOSTIC_MARKERS)[number][1]
  | (typeof OBSERVER_SCALER_REJECTION_CODES)[number]
  | FontReconciliationRejectionStage;

interface Diagnosis {
  conditionId: string;
  category: FixedReasonCategory;
  detail?: FailureDetail;
}

function record(value: JsonValue | undefined): JsonRecord | undefined {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as JsonRecord)
    : undefined;
}

function text(value: JsonValue | undefined): string | undefined {
  return typeof value === "string" ? value : undefined;
}

function stringItems(value: JsonValue | undefined): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : [];
}

function knownReason(reasons: readonly string[], known: ReadonlySet<string>): boolean {
  return reasons.some((reason) =>
    known.has(reason.startsWith("Error: ") ? reason.slice("Error: ".length) : reason),
  );
}

function hasKrrCapture(row: JsonRecord): boolean {
  return text(row.krrCapturePath) !== undefined && text(row.krrCaptureSha256) !== undefined;
}

function hasResourceOrCdpProblem(row: JsonRecord): boolean {
  const observation = record(row.fontIdentityObservation);
  const resources = record(row.resourceProvenance);
  return (
    observation?.status !== "observed" ||
    stringItems(observation?.unknownReasons).length > 0 ||
    resources?.inventoryComplete !== true
  );
}

function rejectionReasons(row: JsonRecord): string[] {
  const unknownReason = text(row.unknownReason);
  return [
    ...stringItems(row.fontIdentityUnknownReasons),
    ...(unknownReason === undefined ? [] : [unknownReason]),
  ];
}

function failureDetail(row: JsonRecord): FailureDetail | undefined {
  const error = text(row.error);
  const reports = [...rejectionReasons(row), ...(error === undefined ? [] : [error])];
  return (
    OBSERVER_SCALER_REJECTION_CODES.find((token) =>
      reports.some((report) => report.includes(token)),
    ) ??
    DIAGNOSTIC_MARKERS.find(([marker]) => reports.some((report) => report.includes(marker)))?.[1] ??
    FONT_RECONCILIATION_REJECTION_STAGES.find((stage) => row.fontIdentityFailureStage === stage)
  );
}

function categoryFor(row: JsonRecord): FixedReasonCategory {
  if (text(row.error) !== undefined) return CAPTURE_EXCEPTION;
  if (!hasKrrCapture(row)) return MISSING_KRR_CAPTURE;
  if (hasResourceOrCdpProblem(row)) return RESOURCE_OR_CDP_PROBLEM;
  const reasons = rejectionReasons(row);
  if (knownReason(reasons, OBSERVER_REJECTION_REASONS)) return REJECTED_OBSERVER;
  if (knownReason(reasons, KRR_CHROME_MISMATCH_REASONS)) return KRR_CHROME_MISMATCH;
  return GENERIC_VERIFICATION_REJECTION;
}

export function catalogStage90RejectionDiagnostics(
  canonicalConditionIds: readonly string[],
  finalizedConditions: readonly JsonValue[],
): {
  schema: "krr-html84-stage90-rejection-diagnostics-v2";
  conditions: Diagnosis[];
} {
  return {
    schema: "krr-html84-stage90-rejection-diagnostics-v2",
    conditions: finalizedConditions.flatMap((value, index) => {
      const conditionId = canonicalConditionIds[index];
      const row = record(value);
      if (!conditionId || row?.status === "captured") return [];
      const detail = row && failureDetail(row);
      return [{ conditionId, category: categoryFor(row ?? {}), ...(detail && { detail }) }];
    }),
  };
}
