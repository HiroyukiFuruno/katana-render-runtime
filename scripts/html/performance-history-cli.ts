import { parseArgs } from "node:util";
import {
  activeReference,
  type PerformanceHistory,
  type PerformanceHistoryRecord,
  readPerformanceHistory,
} from "./performance-history";

function selectedRecord(
  history: PerformanceHistory,
  requestedId: string | undefined,
): PerformanceHistoryRecord {
  if (requestedId === undefined) {
    return activeReference(history);
  }
  const record = history.records.find((entry) => entry.id === requestedId);
  if (record === undefined) throw new Error(`unknown performance history record: ${requestedId}`);
  return record;
}

export type PerformanceHistoryView = {
  schema: "krr-html-performance-history-view-v1";
  selection: "active-reference" | "requested-record";
  historicalResult: true;
  currentHeadAcceptance: "not-established-by-this-historical-record";
  record: PerformanceHistoryRecord;
};

export function performanceHistoryView(
  history: PerformanceHistory,
  requestedId: string | undefined,
): PerformanceHistoryView {
  return {
    schema: "krr-html-performance-history-view-v1",
    selection: requestedId === undefined ? "active-reference" : "requested-record",
    historicalResult: true,
    currentHeadAcceptance: "not-established-by-this-historical-record",
    record: selectedRecord(history, requestedId),
  };
}

export function runPerformanceHistoryCli(args: string[]): number {
  try {
    const parsed = parseArgs({
      args,
      options: { root: { type: "string" }, "record-id": { type: "string" } },
      strict: true,
      allowPositionals: false,
    });
    const root = parsed.values.root ?? process.cwd();
    const history = readPerformanceHistory(root);
    const requestedId = parsed.values["record-id"];
    const view = performanceHistoryView(history, requestedId);
    process.stdout.write(`${JSON.stringify(view, null, 2)}\n`);
    return 0;
  } catch (error) {
    const message =
      error instanceof Error ? error.message : "performance history could not be read";
    process.stderr.write(`${message}\n`);
    return 1;
  }
}

if (import.meta.main) process.exitCode = runPerformanceHistoryCli(process.argv.slice(2));
