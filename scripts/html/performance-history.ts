import fs from "node:fs";
import path from "node:path";
import { z } from "zod";

const sha256 = z.string().regex(/^[a-f0-9]{64}$/);
const referenceBuild = z.strictObject({
  schema: z.literal("krr-html-performance-reference-build-pins-v1"),
  buildArtifactSchema: z.literal("krr-html-performance-build-artifact-v3"),
  buildArtifactSha256: sha256,
  sourceSnapshotSha256: sha256,
  binarySha256: sha256,
  lockSha256: sha256,
  cargoExecutableSha256: sha256,
  rustcExecutableSha256: sha256,
  contextBeforeSha256: sha256,
});
const count = z.number().int().positive().safe();
const nonnegativeCount = z.number().int().nonnegative().safe();
const memoryPolicy = z.strictObject({
  version: z.literal("html-memory-v2"),
  policySha256: sha256,
  retentionEvidenceSha256: sha256,
  candidateConditions: count,
  acceptedConditions: count,
  warmupCycles: z.literal(5),
  measuredCycles: z.literal(25),
});
const seconds = z.number().finite().nonnegative();
const phase = z
  .strictObject({
    maxSampleSeconds: seconds,
    medianSeconds: seconds,
    p95Seconds: seconds,
    maxPeakRssBytes: count,
  })
  .refine(
    (value) =>
      value.medianSeconds <= value.p95Seconds && value.p95Seconds <= value.maxSampleSeconds,
  );
const phases = z.strictObject({
  cold_open: phase,
  full_y: phase,
  post_operation: phase,
  re_display: phase,
  same_session_resize: phase,
  warm_open: phase,
});
const scope = z.strictObject({
  catalog: z.string().min(1),
  candidateConditions: count,
  pairedConditions: nonnegativeCount,
  candidateOnlyConditions: nonnegativeCount,
  trialsPerConditionPerPhase: count,
  candidateSamplesPerPhase: count,
  candidateAndBaselineSamplesPerPhase: count,
  phaseRecords: count,
  checks: count,
  aggregation: z.string().min(1),
});
const record = z
  .strictObject({
    id: z.string().min(1),
    sourceCommit: z.string().regex(/^[a-f0-9]{40}$/),
    verificationCompletedAtUtc: z.iso.datetime(),
    bundleManifestSha256: sha256,
    referenceBuild: referenceBuild.optional(),
    scope,
    measuredAcceptance: z.strictObject({
      verifierRun: z.string().min(1),
      verifierExitCode: z.number().int(),
      performanceNonregression: z.boolean(),
      htmlMinimumScore: z.number().finite().min(0).max(100),
      htmlScoreThreshold: z.number().finite().min(0).max(100),
      outputFontIdentityEquivalence: z.enum(["established", "not-established"]),
    }),
    candidatePhaseStatistics: phases,
    memoryPolicy: memoryPolicy.optional(),
    evidence: z.strictObject({
      summaryRun: z.string().min(1),
      summarySha256: sha256,
      verifierStdoutSha256: sha256,
    }),
    reuseStatus: z.literal(
      "historical-result; current-source eligibility must be verified separately",
    ),
  })
  .refine(
    ({ scope: value }) =>
      value.candidateConditions === value.pairedConditions + value.candidateOnlyConditions &&
      value.candidateSamplesPerPhase ===
        value.candidateConditions * value.trialsPerConditionPerPhase &&
      value.candidateAndBaselineSamplesPerPhase ===
        (value.candidateConditions + value.pairedConditions) * value.trialsPerConditionPerPhase &&
      value.phaseRecords === value.candidateAndBaselineSamplesPerPhase * 6,
  )
  .superRefine((entry, context) => {
    if (entry.memoryPolicy) {
      if (entry.memoryPolicy.candidateConditions !== entry.scope.candidateConditions) {
        context.addIssue({
          code: "custom",
          path: ["memoryPolicy", "candidateConditions"],
          message: "memory policy condition count differs from scope",
        });
      }
      if (entry.memoryPolicy.acceptedConditions !== entry.memoryPolicy.candidateConditions) {
        context.addIssue({
          code: "custom",
          path: ["memoryPolicy", "acceptedConditions"],
          message: "memory policy must accept every recorded candidate condition",
        });
      }
      if (!entry.measuredAcceptance.performanceNonregression) {
        context.addIssue({
          code: "custom",
          path: ["measuredAcceptance", "performanceNonregression"],
          message: "memory policy evidence requires recorded performance nonregression",
        });
      }
    }
  });

export const performanceHistorySchema = z
  .strictObject({
    schema: z.union([
      z.literal("krr-html-performance-history-v1"),
      z.literal("krr-html-performance-history-v2"),
    ]),
    activeReferenceRecordId: z.string().min(1),
    activeReferenceSelectionNote: z.string().min(1),
    records: z.array(record).min(1),
  })
  .superRefine((history, context) => {
    const ids = new Set(history.records.map((entry) => entry.id));
    if (ids.size !== history.records.length) {
      context.addIssue({ code: "custom", message: "duplicate record id" });
    }
    if (history.schema === "krr-html-performance-history-v1") {
      history.records.forEach((entry, index) => {
        if (entry.memoryPolicy !== undefined) {
          context.addIssue({
            code: "custom",
            path: ["records", index, "memoryPolicy"],
            message: "v1 history cannot contain memory policy metadata",
          });
        }
      });
    }
  });

export type PerformanceHistory = z.infer<typeof performanceHistorySchema>;
export type PerformanceHistoryRecord = PerformanceHistory["records"][number];

function selectReference(history: PerformanceHistory): PerformanceHistoryRecord {
  const active = history.records.find((entry) => entry.id === history.activeReferenceRecordId);
  if (!active) throw new Error("active reference is missing");
  if (!active.referenceBuild) throw new Error("active reference build pins are missing");
  const acceptance = active.measuredAcceptance;
  if (
    acceptance.verifierExitCode !== 0 ||
    !acceptance.performanceNonregression ||
    acceptance.htmlMinimumScore < acceptance.htmlScoreThreshold ||
    acceptance.outputFontIdentityEquivalence !== "established" ||
    Object.values(active.candidatePhaseStatistics).some((entry) => entry.maxSampleSeconds >= 3)
  ) {
    throw new Error("active reference did not pass recorded acceptance");
  }
  return active;
}

/* WHY: 記録の整合性は元Bundleの真正性や現HEADの受入を証明しない。 */
export function parsePerformanceHistory(value: unknown): PerformanceHistory {
  const history = performanceHistorySchema.parse(value);
  selectReference(history);
  return history;
}

export function readPerformanceHistory(root: string): PerformanceHistory {
  const file = path.join(root, "docs/performance/html-performance-history.json");
  const value: unknown = JSON.parse(fs.readFileSync(file, "utf8"));
  return parsePerformanceHistory(value);
}

export function activeReference(history: PerformanceHistory): PerformanceHistoryRecord {
  const validated = performanceHistorySchema.parse(history);
  return selectReference(validated);
}
