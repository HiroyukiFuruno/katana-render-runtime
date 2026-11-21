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
  schema: "krr-html84-stage90-rejection-diagnostics-v1";
  conditions: Array<{ conditionId: string; category: FixedReasonCategory }>;
} {
  return {
    schema: "krr-html84-stage90-rejection-diagnostics-v1",
    conditions: finalizedConditions.flatMap((value, index) => {
      const conditionId = canonicalConditionIds[index];
      const row = record(value);
      if (!conditionId || row?.status === "captured") return [];
      return [{ conditionId, category: categoryFor(row ?? {}) }];
    }),
  };
}
