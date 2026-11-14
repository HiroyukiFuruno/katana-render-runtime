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

function activeRecord(history: PerformanceHistory): PerformanceHistoryRecord {
  const active = history.records.find((record) => record.id === history.activeReferenceRecordId);
  if (!active) throw new Error("fixture has no active record");
  return active;
}

function validMemoryPolicy(
  record: PerformanceHistoryRecord,
): NonNullable<PerformanceHistoryRecord["memoryPolicy"]> {
  return {
    version: "html-memory-v2",
    policySha256: "a".repeat(64),
    retentionEvidenceSha256: "b".repeat(64),
    candidateConditions: record.scope.candidateConditions,
    acceptedConditions: record.scope.candidateConditions,
    warmupCycles: 5,
    measuredCycles: 25,
  };
}

function validMemoryPolicyV3(
  record: PerformanceHistoryRecord,
): NonNullable<PerformanceHistoryRecord["memoryPolicy"]> {
  return {
    version: "html-memory-v3",
    policySha256: "c".repeat(64),
    retentionEvidenceSha256: "d".repeat(64),
    candidateConditions: record.scope.candidateConditions,
    acceptedConditions: record.scope.candidateConditions,
    warmupCycles: 5,
    measuredCycles: 25,
  };
}

function addV2Record(history: PerformanceHistory): PerformanceHistoryRecord {
  history.schema = "krr-html-performance-history-v2";
  const record = structuredClone(firstRecord(history));
  record.id = "html-memory-v2-record";
  record.memoryPolicy = validMemoryPolicy(record);
  history.records.push(record);
  return record;
}

function validMemoryPolicyV4(
  _record: PerformanceHistoryRecord,
): NonNullable<PerformanceHistoryRecord["memoryPolicy"]> {
  return {
    version: "html-memory-v4",
    policySha256: "e".repeat(64),
    retentionEvidenceSha256: "f".repeat(64),
    candidateConditions: 84,
    acceptedConditions: 84,
    warmupCycles: 5,
    measuredCycles: 25,
  };
}

function addV3Record(history: PerformanceHistory): PerformanceHistoryRecord {
  history.schema = "krr-html-performance-history-v2";
  const record = structuredClone(firstRecord(history));
  record.id = "html-memory-v3-record";
  record.memoryPolicy = validMemoryPolicyV3(record);
  history.records.push(record);
  return record;
}

function invalidMemoryPolicyChanges(): Array<(record: PerformanceHistoryRecord) => void> {
  return [
    (record) => {
      if (!record.memoryPolicy) throw new Error("fixture has no memory policy");
      record.memoryPolicy.policySha256 = "invalid";
    },
    (record) => {
      if (!record.memoryPolicy) throw new Error("fixture has no memory policy");
      record.memoryPolicy.candidateConditions += 1;
    },
    (record) => {
      if (!record.memoryPolicy) throw new Error("fixture has no memory policy");
      record.memoryPolicy.acceptedConditions -= 1;
    },
    (record) => {
      if (!record.memoryPolicy) throw new Error("fixture has no memory policy");
      Object.assign(record.memoryPolicy, { warmupCycles: 4 });
    },
    (record) => {
      if (!record.memoryPolicy) throw new Error("fixture has no memory policy");
      Object.assign(record.memoryPolicy, { measuredCycles: 24 });
    },
    (record) => {
      record.measuredAcceptance.performanceNonregression = false;
    },
  ];
}

function expectInvalidMemoryPolicyChanges(
  addRecord: (history: PerformanceHistory) => PerformanceHistoryRecord,
): void {
  for (const change of invalidMemoryPolicyChanges()) {
    const history = fixture();
    change(addRecord(history));
    expect(() => parsePerformanceHistory(history)).toThrow();
  }
}

function addV4Record(history: PerformanceHistory): PerformanceHistoryRecord {
  history.schema = "krr-html-performance-history-v2";
  const record = structuredClone(firstRecord(history));
  record.id = "html-memory-v4-record";
  record.memoryPolicy = validMemoryPolicyV4(record);
  history.records.push(record);
  return record;
}

function addV5Record(history: PerformanceHistory): PerformanceHistoryRecord {
  const record = addV4Record(history);
  record.id = "html-memory-v5-record";
  record.memoryPolicy = { ...validMemoryPolicyV4(record), version: "html-memory-v5" };
  return record;
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
      delete activeRecord(history).referenceBuild;
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
    (history) => {
      firstRecord(history).memoryPolicy = validMemoryPolicy(firstRecord(history));
    },
  ];
  for (const change of changes) {
    const history = fixture();
    change(history);
    expect(() => parsePerformanceHistory(history)).toThrow();
  }
  expect(() => parsePerformanceHistory({ ...fixture(), extra: true })).toThrow();
});

test("accepts v2 policy evidence alongside unchanged v1 records", () => {
  const history = fixture();
  const legacy = structuredClone(firstRecord(history));
  const v2Record = addV2Record(history);

  const parsed = parsePerformanceHistory(history);
  expect(parsed.records[0]).toEqual(legacy);
  expect(v2Record.memoryPolicy).toEqual({
    version: "html-memory-v2",
    policySha256: "a".repeat(64),
    retentionEvidenceSha256: "b".repeat(64),
    candidateConditions: v2Record.scope.candidateConditions,
    acceptedConditions: v2Record.scope.candidateConditions,
    warmupCycles: 5,
    measuredCycles: 25,
  });
  expect(activeReference(parsed).id).toBe(history.activeReferenceRecordId);
});

test("accepts v3 policy evidence alongside unchanged v1 and v2 records", () => {
  const history = fixture();
  const legacy = structuredClone(firstRecord(history));
  const v2Record = addV2Record(history);
  const v3Record = addV3Record(history);

  const parsed = parsePerformanceHistory(history);
  expect(parsed.records[0]).toEqual(legacy);
  expect(v2Record.memoryPolicy?.version).toBe("html-memory-v2");
  expect(v3Record.memoryPolicy).toEqual(validMemoryPolicyV3(v3Record));
  expect(activeReference(parsed).id).toBe(history.activeReferenceRecordId);
});

test("accepts v4 evidence alongside unchanged v1, v2, and v3 records", () => {
  const history = fixture();
  const legacy = structuredClone(firstRecord(history));
  const v2Record = addV2Record(history);
  const v3Record = addV3Record(history);
  const v4Record = addV4Record(history);
  const parsed = parsePerformanceHistory(history);
  expect(parsed.records[0]).toEqual(legacy);
  expect(v2Record.memoryPolicy?.version).toBe("html-memory-v2");
  expect(v3Record.memoryPolicy?.version).toBe("html-memory-v3");
  expect(v4Record.memoryPolicy).toEqual(validMemoryPolicyV4(v4Record));
});

test("rejects malformed or incomplete v2 memory policy evidence", () => {
  expectInvalidMemoryPolicyChanges(addV2Record);
});

test("rejects malformed or incomplete v3 memory policy evidence", () => {
  expectInvalidMemoryPolicyChanges(addV3Record);
});

test("rejects malformed or incomplete v4 memory policy evidence", () => {
  expectInvalidMemoryPolicyChanges(addV4Record);
});

test("accepts v5 evidence and rejects incomplete condition coverage", () => {
  const history = fixture();
  const record = addV5Record(history);
  expect(parsePerformanceHistory(history).records.at(-1)).toEqual(record);
  expectInvalidMemoryPolicyChanges(addV5Record);
});

test("rejects a reference that does not meet its recorded acceptance", () => {
  const changes: Array<(history: PerformanceHistory) => void> = [
    (history) => {
      activeRecord(history).measuredAcceptance.verifierExitCode = 1;
    },
    (history) => {
      activeRecord(history).measuredAcceptance.performanceNonregression = false;
    },
    (history) => {
      activeRecord(history).measuredAcceptance.htmlMinimumScore = 0;
    },
    (history) => {
      activeRecord(history).measuredAcceptance.outputFontIdentityEquivalence = "not-established";
    },
    (history) => {
      activeRecord(history).candidatePhaseStatistics.cold_open.maxSampleSeconds = 3;
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
