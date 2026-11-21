import fs from "node:fs";
import path from "node:path";
import type { JsonValue } from "./dynamic-state";
import {
  assertCatalogComplete,
  assertFeatureResultsComplete,
  type FeatureCaseResult,
  type FeatureCatalog,
  type FeatureResultStatus,
} from "./feature-catalog";

export function readInterimFeatureResults(resultsPath: string): FeatureCaseResult[] {
  const value = JSON.parse(fs.readFileSync(path.resolve(resultsPath), "utf8")) as JsonValue;
  if (
    typeof value !== "object" ||
    value === null ||
    Array.isArray(value) ||
    value.phase !== "interim" ||
    value.threshold !== 90 ||
    !Array.isArray(value.results)
  )
    throw new Error("interim feature evidence requires its explicit phase/90 envelope");
  return value.results.map((entry) => {
    if (
      typeof entry !== "object" ||
      entry === null ||
      Array.isArray(entry) ||
      typeof entry.caseId !== "string" ||
      entry.caseId.length === 0 ||
      !isStatus(entry.status) ||
      typeof entry.features !== "object" ||
      entry.features === null ||
      Array.isArray(entry.features)
    )
      throw new Error("invalid interim feature result");
    const features: Record<string, FeatureResultStatus> = {};
    for (const [feature, status] of Object.entries(entry.features)) {
      if (!isStatus(status)) throw new Error("invalid interim feature status");
      features[feature] = status;
    }
    return { caseId: entry.caseId, status: entry.status, features };
  });
}

function isStatus(value: JsonValue | undefined): value is FeatureResultStatus {
  return value === "passed" || value === "failed" || value === "unverified";
}

export function assertInterimFeatureResultReportsComplete(
  catalog: FeatureCatalog,
  visualPath: string,
  scriptPath: string,
): void {
  assertCatalogComplete(catalog);
  assertFeatureResultsComplete(catalog, [
    ...readInterimFeatureResults(visualPath),
    ...readInterimFeatureResults(scriptPath),
  ]);
}
