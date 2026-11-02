import { expect, test } from "bun:test";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  activeReference,
  type PerformanceHistory,
  type PerformanceHistoryRecord,
  parsePerformanceHistory,
  readPerformanceHistory,
} from "./performance-history";
import { performanceHistoryView } from "./performance-history-cli";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

function fixture(): PerformanceHistory {
  return structuredClone(readPerformanceHistory(root));
}

function firstRecord(history: PerformanceHistory): PerformanceHistoryRecord {
  const first = history.records[0];
  if (!first) throw new Error("fixture has no records");
  return first;
}

function runHistoryCli(args: string[]): ReturnType<typeof Bun.spawnSync> {
  return Bun.spawnSync(["bun", path.join(import.meta.dir, "performance-history-cli.ts"), ...args], {
    cwd: root,
    stdout: "pipe",
    stderr: "pipe",
  });
}

function outputText(
  output: ReturnType<typeof Bun.spawnSync>["stdout"] | ReturnType<typeof Bun.spawnSync>["stderr"],
  stream: string,
): string {
  if (output === undefined) throw new Error(`${stream} was not captured`);
  return output.toString();
}

test("selects the recorded historical reference without claiming current-source acceptance", () => {
  const history = fixture();
  const selected = activeReference(history);
  expect(selected.id).toBe(history.activeReferenceRecordId);
  expect(selected.referenceBuild).toMatchObject({
    schema: "krr-html-performance-reference-build-pins-v1",
    buildArtifactSchema: "krr-html-performance-build-artifact-v3",
  });
  expect(selected.reuseStatus).toContain("current-source eligibility must be verified separately");
  expect(selected.scope.candidateSamplesPerPhase).toBe(
    selected.scope.candidateConditions * selected.scope.trialsPerConditionPerPhase,
  );
});

test("rejects malformed or internally inconsistent history", () => {
  const changes: Array<(history: PerformanceHistory) => void> = [
    (history) => {
      history.records.push(structuredClone(firstRecord(history)));
    },
    (history) => {
      history.activeReferenceRecordId = "missing";
    },
    (history) => {
      firstRecord(history).sourceCommit = "not-a-commit";
    },
    (history) => {
      delete firstRecord(history).referenceBuild;
    },
    (history) => {
      const build = firstRecord(history).referenceBuild;
      if (!build) throw new Error("fixture active reference has no build pins");
      build.binarySha256 = "not-a-sha256";
    },
    (history) => {
      const build = firstRecord(history).referenceBuild;
      if (!build) throw new Error("fixture active reference has no build pins");
      Object.assign(build, { schema: "unknown-reference-build-schema" });
    },
    (history) => {
      firstRecord(history).evidence.summarySha256 = "0";
    },
    (history) => {
      firstRecord(history).scope.candidateSamplesPerPhase += 1;
    },
    (history) => {
      firstRecord(history).scope.candidateAndBaselineSamplesPerPhase += 1;
    },
    (history) => {
      firstRecord(history).scope.phaseRecords += 1;
    },
    (history) => {
      firstRecord(history).scope.candidateOnlyConditions += 1;
    },
    (history) => {
      firstRecord(history).candidatePhaseStatistics.cold_open.p95Seconds = -1;
    },
    (history) => {
      firstRecord(history).candidatePhaseStatistics.cold_open.p95Seconds = 2;
    },
    (history) => {
      firstRecord(history).candidatePhaseStatistics.cold_open.maxPeakRssBytes = 1.5;
    },
  ];
  for (const change of changes) {
    const history = fixture();
    change(history);
    expect(() => parsePerformanceHistory(history)).toThrow();
  }
  expect(() => parsePerformanceHistory({ ...fixture(), extra: true })).toThrow();
});

test("rejects a reference that does not meet its recorded acceptance", () => {
  const changes: Array<(history: PerformanceHistory) => void> = [
    (history) => {
      firstRecord(history).measuredAcceptance.verifierExitCode = 1;
    },
    (history) => {
      firstRecord(history).measuredAcceptance.performanceNonregression = false;
    },
    (history) => {
      firstRecord(history).measuredAcceptance.htmlMinimumScore = 0;
    },
    (history) => {
      firstRecord(history).measuredAcceptance.outputFontIdentityEquivalence = "not-established";
    },
    (history) => {
      firstRecord(history).candidatePhaseStatistics.cold_open.maxSampleSeconds = 3;
    },
  ];
  for (const change of changes) {
    const history = fixture();
    change(history);
    expect(() => activeReference(history)).toThrow();
  }
});

test("retains a failed historical record without selecting it", () => {
  const history = fixture();
  const failed = structuredClone(firstRecord(history));
  failed.id = "historical-failure";
  delete failed.referenceBuild;
  failed.measuredAcceptance.verifierExitCode = 1;
  failed.measuredAcceptance.performanceNonregression = false;
  history.records.push(failed);
  expect(activeReference(parsePerformanceHistory(history)).id).toBe(
    history.activeReferenceRecordId,
  );
  history.activeReferenceRecordId = failed.id;
  expect(() => parsePerformanceHistory(history)).toThrow();
});

test("CLI displays one historical record without claiming current HEAD acceptance", () => {
  const history = fixture();
  const expected = activeReference(history);
  const importedView = performanceHistoryView(history, undefined);
  expect(importedView.record).toEqual(expected);
  expect(importedView.currentHeadAcceptance).toBe("not-established-by-this-historical-record");

  const result = runHistoryCli([]);
  expect(result.exitCode).toBe(0);
  const view = JSON.parse(outputText(result.stdout, "stdout")) as {
    schema: string;
    selection: string;
    historicalResult: boolean;
    currentHeadAcceptance: string;
    record: PerformanceHistoryRecord;
  };
  expect(view.schema).toBe("krr-html-performance-history-view-v1");
  expect(view.selection).toBe("active-reference");
  expect(view.historicalResult).toBe(true);
  expect(view.currentHeadAcceptance).toBe("not-established-by-this-historical-record");
  expect(view.record).toEqual(expected);
  expect(view.record.candidatePhaseStatistics).toEqual(expected.candidatePhaseStatistics);
  expect(view.record.evidence).toEqual(expected.evidence);

  const selected = runHistoryCli(["--root", root, "--record-id", expected.id]);
  expect(selected.exitCode).toBe(0);
  const selectedView = JSON.parse(outputText(selected.stdout, "stdout")) as {
    record: PerformanceHistoryRecord;
  };
  expect(selectedView.record.id).toBe(expected.id);
});

test("CLI rejects an unknown history record id", () => {
  const result = runHistoryCli(["--record-id", "unknown-record"]);
  expect(result.exitCode).toBe(1);
  expect(outputText(result.stdout, "stdout")).toBe("");
  expect(outputText(result.stderr, "stderr")).toContain("unknown performance history record");
});
