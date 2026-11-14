import fs from "node:fs";
import path from "node:path";
import type { JsonValue } from "./dynamic-state";

export type CatalogMode = "visual" | "script";

export interface CatalogCase {
  id: string;
  fixture: string;
  mode: CatalogMode;
  features: string[];
  boundaryStates?: string[];
  expectedStateId?: string;
}

export interface FeatureCatalog {
  schemaVersion: number;
  threshold: number;
  visualAspects: CatalogCase[];
  scriptCases: CatalogCase[];
  coveragePolicy: {
    representativeSamplesAreInsufficient: boolean;
    unmappedFixtureIsFailure: boolean;
    unrunCaseIsFailure: boolean;
    unsupportedIsUnverified: boolean;
    wptFullConformanceClaim: boolean;
  };
}

export type FeatureResultStatus = "passed" | "failed" | "unverified";

export interface FeatureCaseResult {
  caseId: string;
  status: FeatureResultStatus;
  features: Record<string, FeatureResultStatus>;
}

export function loadFeatureCatalog(
  catalogPath = "tests/fixtures/html/feature-catalog.json",
): FeatureCatalog {
  const value = JSON.parse(fs.readFileSync(path.resolve(catalogPath), "utf8")) as JsonValue;
  if (!isRecord(value) || value.schemaVersion !== 1 || value.threshold !== 99)
    throw new Error("invalid feature catalog schema or threshold");
  if (!isCatalogCaseArray(value.visualAspects) || !isCatalogCaseArray(value.scriptCases))
    throw new Error("invalid feature catalog case list");
  if (
    !isRecord(value.coveragePolicy) ||
    Object.values(value.coveragePolicy).some((flag) => typeof flag !== "boolean")
  )
    throw new Error("invalid feature catalog coverage policy");
  const policy = value.coveragePolicy;
  for (const flag of [
    "representativeSamplesAreInsufficient",
    "unmappedFixtureIsFailure",
    "unrunCaseIsFailure",
    "unsupportedIsUnverified",
  ]) {
    if (policy[flag] !== true) throw new Error(`missing or weakened coverage policy: ${flag}`);
  }
  if (policy.wptFullConformanceClaim !== false)
    throw new Error("WPT conformance claim is forbidden");
  return {
    schemaVersion: 1,
    threshold: 99,
    visualAspects: parseCases(value.visualAspects, "visual"),
    scriptCases: parseCases(value.scriptCases, "script"),
    coveragePolicy: {
      representativeSamplesAreInsufficient: true,
      unmappedFixtureIsFailure: true,
      unrunCaseIsFailure: true,
      unsupportedIsUnverified: true,
      wptFullConformanceClaim: false,
    },
  };
}

function isRecord(value: JsonValue | undefined): value is { [key: string]: JsonValue } {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isCatalogCaseArray(value: JsonValue | undefined): value is JsonValue[] {
  return (
    Array.isArray(value) &&
    value.every((entry) => {
      if (
        !isRecord(entry) ||
        typeof entry.id !== "string" ||
        typeof entry.fixture !== "string" ||
        (entry.mode !== "visual" && entry.mode !== "script") ||
        !Array.isArray(entry.features) ||
        entry.features.length === 0 ||
        entry.features.some((feature) => typeof feature !== "string")
      )
        return false;
      return entry.mode !== "script" || typeof entry.expectedStateId === "string";
    })
  );
}

function parseCases(entries: JsonValue[], mode: CatalogMode): CatalogCase[] {
  return entries.map((entry) => {
    if (
      !isRecord(entry) ||
      entry.mode !== mode ||
      typeof entry.id !== "string" ||
      typeof entry.fixture !== "string"
    )
      throw new Error("catalog case mode differs");
    const features = stringList(entry.features);
    const boundaryStates =
      entry.boundaryStates === undefined ? [] : stringList(entry.boundaryStates);
    if (entry.id.length === 0 || entry.fixture.length === 0)
      throw new Error("empty catalog case identity");
    return {
      id: entry.id,
      fixture: entry.fixture,
      mode,
      features,
      boundaryStates,
      ...(typeof entry.expectedStateId === "string"
        ? { expectedStateId: entry.expectedStateId }
        : {}),
    };
  });
}

function stringList(value: JsonValue | undefined): string[] {
  if (!Array.isArray(value)) throw new Error("catalog string list required");
  return value.map((item) => {
    if (typeof item !== "string" || item.length === 0) throw new Error("invalid catalog string");
    return item;
  });
}

export function assertCatalogComplete(
  catalog: FeatureCatalog,
  fixtureRoot = "tests/fixtures/html",
): void {
  if (catalog.threshold !== 99) throw new Error("feature catalog threshold must remain 99");
  const cases = [...catalog.visualAspects, ...catalog.scriptCases];
  const ids = new Set<string>();
  for (const entry of cases) {
    if (ids.has(entry.id)) throw new Error(`duplicate feature case: ${entry.id}`);
    ids.add(entry.id);
    if (entry.features.length === 0) throw new Error(`feature case has no features: ${entry.id}`);
    const fixture = path.resolve(fixtureRoot, entry.fixture);
    if (!fs.existsSync(fixture)) throw new Error(`missing feature fixture: ${entry.fixture}`);
    if (entry.mode === "script" && !entry.expectedStateId) {
      throw new Error(`script case has no expected state: ${entry.id}`);
    }
  }
  const expected = new Set(cases.map((entry) => path.resolve(fixtureRoot, entry.fixture)));
  const actual = fs
    .readdirSync(path.resolve(fixtureRoot, "aspects"), { withFileTypes: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith(".html"))
    .map((entry) => path.resolve(fixtureRoot, "aspects", entry.name));
  for (const fixture of actual) {
    if (!expected.has(fixture))
      throw new Error(`unmapped feature fixture: ${path.relative(fixtureRoot, fixture)}`);
  }
  if (catalog.coveragePolicy.wptFullConformanceClaim) {
    throw new Error("full WPT conformance cannot be claimed by this catalog");
  }
}

export function assertFeatureResultsComplete(
  catalog: FeatureCatalog,
  results: FeatureCaseResult[],
): void {
  const cases = [...catalog.visualAspects, ...catalog.scriptCases];
  const byId = new Map(results.map((result) => [result.caseId, result]));
  if (byId.size !== results.length) throw new Error("duplicate feature result");
  for (const entry of cases) {
    const result = byId.get(entry.id);
    if (!result) throw new Error(`unrun feature case: ${entry.id}`);
    if (result.status !== "passed") throw new Error(`feature case is not passed: ${entry.id}`);
    for (const feature of entry.features) {
      if (result.features[feature] !== "passed")
        throw new Error(`feature is not verified: ${entry.id}/${feature}`);
    }
  }
  validateExtraResults(cases, results);
}

export function readFeatureResults(resultsPath: string): FeatureCaseResult[] {
  const value = JSON.parse(fs.readFileSync(path.resolve(resultsPath), "utf8")) as JsonValue;
  if (!Array.isArray(value)) throw new Error(`invalid feature results: ${resultsPath}`);
  return value.map((entry) => {
    if (!isRecord(entry) || typeof entry.caseId !== "string" || entry.caseId.length === 0)
      throw new Error(`invalid feature result case: ${resultsPath}`);
    if (entry.status !== "passed" && entry.status !== "failed" && entry.status !== "unverified")
      throw new Error(`invalid feature result status: ${entry.caseId}`);
    if (!isRecord(entry.features))
      throw new Error(`invalid feature result features: ${entry.caseId}`);
    const features: Record<string, FeatureResultStatus> = {};
    for (const [feature, status] of Object.entries(entry.features)) {
      if (status !== "passed" && status !== "failed" && status !== "unverified")
        throw new Error(`invalid feature result feature status: ${entry.caseId}/${feature}`);
      features[feature] = status;
    }
    return { caseId: entry.caseId, status: entry.status, features };
  });
}

export function assertFeatureResultReportsComplete(
  catalog: FeatureCatalog,
  visualResultsPath: string,
  scriptResultsPath: string,
): void {
  assertFeatureResultsComplete(catalog, [
    ...readFeatureResults(visualResultsPath),
    ...readFeatureResults(scriptResultsPath),
  ]);
}

function validateExtraResults(cases: CatalogCase[], results: FeatureCaseResult[]): void {
  for (const result of results) {
    if (!cases.some((entry) => entry.id === result.caseId))
      throw new Error(`extra feature result: ${result.caseId}`);
    if (!Object.values(result.features).every((status) => status === "passed"))
      throw new Error(`feature result contains unverified status: ${result.caseId}`);
  }
}

export function readExpectedState(fixturePath: string, stateId = "expected-state"): JsonValue {
  const source = fs.readFileSync(path.resolve(fixturePath), "utf8");
  const match = source.match(
    new RegExp(`<script[^>]+id=["']${stateId}["'][^>]*>([\\s\\S]*?)</script>`),
  );
  if (!match?.[1]) throw new Error(`missing expected state ${stateId}: ${fixturePath}`);
  return JSON.parse(match[1]) as JsonValue;
}
