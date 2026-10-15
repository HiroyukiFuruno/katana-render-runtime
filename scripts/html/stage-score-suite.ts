import fs from "node:fs";
import path from "node:path";
import type { JsonValue } from "./dynamic-state";
import { assertCatalogComplete, type CatalogCase, loadFeatureCatalog } from "./feature-catalog";
import { sha256File } from "./reference-capture";
import { assertStrictStage90Manifest, type StageViewport } from "./stage-score";

export interface StageScoreSuiteCondition {
  conditionId: string;
  caseId: string;
  state: string;
  viewport: StageViewport;
  sourceSha256: string;
  binarySha256: string;
  binaryPath: string;
  candidate: JsonValue;
}

interface TrustedSuiteCondition extends Omit<StageScoreSuiteCondition, "candidate"> {
  trusted: JsonValue;
}

export interface StageScoreSuiteManifest {
  profile: "stage90-suite";
  threshold: 90;
  acceptance: false;
  catalogPath: string;
  catalogSha256: string;
  conditions: StageScoreSuiteCondition[];
}

export function assertStageScoreSuite(
  value: JsonValue,
  trustedSuitePlan: JsonValue,
  catalogPath = "tests/fixtures/html/feature-catalog.json",
): void {
  const suite = record(value, "stage score suite");
  if (suite.profile !== "stage90-suite" || suite.threshold !== 90 || suite.acceptance !== false)
    throw new Error("invalid stage score suite profile");
  if (path.resolve(text(suite.catalogPath, "catalog path")) !== path.resolve(catalogPath))
    throw new Error("suite catalog path differs");
  if (sha256File(path.resolve(catalogPath)) !== text(suite.catalogSha256, "catalog SHA"))
    throw new Error("suite catalog SHA differs");
  const catalog = loadFeatureCatalog(catalogPath);
  assertCatalogComplete(catalog);
  const conditions = records(suite.conditions, "suite conditions").map(parseCandidateCondition);
  const trustedPlan = record(trustedSuitePlan, "trusted suite plan");
  if (trustedPlan.profile !== "stage90-trusted-plan")
    throw new Error("invalid trusted suite plan profile");
  const trustedConditions = records(trustedPlan.conditions, "trusted plan conditions").map(
    parseTrustedCondition,
  );
  assertConditionInventory(conditions, catalog);
  assertTrustedConditionInventory(trustedConditions, catalog);
  for (const condition of conditions) {
    const trusted = trustedConditions.find((item) => item.conditionId === condition.conditionId);
    if (!trusted) throw new Error(`missing trusted condition: ${condition.conditionId}`);
    assertCondition(condition, trusted, catalog);
  }
}

function parseCandidateCondition(value: { [key: string]: JsonValue }): StageScoreSuiteCondition {
  if (value.trusted !== undefined) throw new Error("candidate suite must not include trusted data");
  return {
    ...parseConditionTuple(value),
    candidate: required(value.candidate, "candidate condition"),
  };
}

function parseTrustedCondition(value: { [key: string]: JsonValue }): TrustedSuiteCondition {
  return {
    ...parseConditionTuple(value),
    trusted: required(value.trusted, "trusted condition"),
  };
}

function parseConditionTuple(value: {
  [key: string]: JsonValue;
}): Omit<StageScoreSuiteCondition, "candidate"> {
  const viewport = record(value.viewport, "condition viewport");
  return {
    conditionId: text(value.conditionId, "condition id"),
    caseId: text(value.caseId, "case id"),
    state: text(value.state, "condition state"),
    viewport: {
      width: positive(viewport.width, "viewport width"),
      height: positive(viewport.height, "viewport height"),
      scale: positive(viewport.scale, "viewport scale"),
    },
    sourceSha256: sha(text(value.sourceSha256, "condition source SHA")),
    binarySha256: sha(text(value.binarySha256, "condition binary SHA")),
    binaryPath: path.resolve(text(value.binaryPath, "condition binary path")),
  };
}

function assertCondition(
  condition: StageScoreSuiteCondition,
  trustedCondition: TrustedSuiteCondition,
  catalog: { scriptCases: CatalogCase[]; visualAspects: CatalogCase[] },
): void {
  const entry = [...catalog.visualAspects, ...catalog.scriptCases].find(
    (item) => item.id === condition.caseId,
  );
  if (!entry) throw new Error(`unrecognized suite case: ${condition.caseId}`);
  const fixture = path.resolve("tests/fixtures/html", entry.fixture);
  if (sha256File(fixture) !== condition.sourceSha256)
    throw new Error(`condition source SHA differs: ${condition.conditionId}`);
  if (!fsFile(condition.binaryPath) || sha256File(condition.binaryPath) !== condition.binarySha256)
    throw new Error(`condition binary differs: ${condition.conditionId}`);
  if (
    trustedCondition.caseId !== condition.caseId ||
    trustedCondition.state !== condition.state ||
    trustedCondition.viewport.width !== condition.viewport.width ||
    trustedCondition.viewport.height !== condition.viewport.height ||
    trustedCondition.viewport.scale !== condition.viewport.scale ||
    trustedCondition.sourceSha256 !== condition.sourceSha256 ||
    trustedCondition.binarySha256 !== condition.binarySha256
  )
    throw new Error(`trusted plan tuple differs: ${condition.conditionId}`);
  const trusted = record(trustedCondition.trusted, "trusted condition");
  const candidate = record(condition.candidate, "candidate condition");
  if (
    trusted.sourceSha256 !== condition.sourceSha256 ||
    trusted.binarySha256 !== condition.binarySha256
  )
    throw new Error(`trusted provenance differs: ${condition.conditionId}`);
  if (
    candidate.sourceSha256 !== condition.sourceSha256 ||
    candidate.binarySha256 !== condition.binarySha256
  )
    throw new Error(`candidate provenance differs: ${condition.conditionId}`);
  if (candidate.profile !== "stage90" || trusted.profile !== "stage90")
    throw new Error(`condition is not an individual stage90 manifest: ${condition.conditionId}`);
  assertManifestTuple(trusted, condition, "trusted");
  assertManifestTuple(candidate, condition, "candidate");
  assertStrictStage90Manifest(condition.candidate, trustedCondition.trusted);
}

function assertTrustedConditionInventory(
  conditions: TrustedSuiteCondition[],
  catalog: { scriptCases: CatalogCase[]; visualAspects: CatalogCase[] },
): void {
  const ids = new Set<string>();
  for (const condition of conditions) {
    if (ids.has(condition.conditionId))
      throw new Error(`duplicate trusted plan condition: ${condition.conditionId}`);
    ids.add(condition.conditionId);
  }
  const required = requiredTuples(catalog);
  const tuples = new Set(
    conditions.map((condition) => tupleKey(condition.caseId, condition.state, condition.viewport)),
  );
  if (tuples.size !== conditions.length) throw new Error("duplicate trusted plan tuple");
  for (const tuple of required)
    if (!tuples.has(tuple)) throw new Error(`missing trusted plan tuple: ${tuple}`);
}

function assertManifestTuple(
  manifest: { [key: string]: JsonValue },
  condition: StageScoreSuiteCondition,
  name: string,
): void {
  const images = records(manifest.images, `${name} images`);
  if (images.length === 0) throw new Error(`${name} images are empty`);
  const performance = record(manifest.performance, `${name} performance`);
  if (performance.conditionId !== condition.conditionId)
    throw new Error(`${name} performance condition differs: ${condition.conditionId}`);
  for (const image of images) {
    if (
      image.caseId !== condition.caseId ||
      image.state !== condition.state ||
      !sameViewport(image.viewport, condition.viewport)
    )
      throw new Error(`${name} image tuple differs: ${condition.conditionId}`);
  }
}

function assertConditionInventory(
  conditions: StageScoreSuiteCondition[],
  catalog: { scriptCases: CatalogCase[]; visualAspects: CatalogCase[] },
): void {
  const ids = new Set<string>();
  for (const condition of conditions) {
    if (ids.has(condition.conditionId))
      throw new Error(`duplicate suite condition: ${condition.conditionId}`);
    ids.add(condition.conditionId);
  }
  const required = requiredTuples(catalog);
  const tuples = new Set<string>();
  for (const condition of conditions) {
    const tuple = tupleKey(condition.caseId, condition.state, condition.viewport);
    if (!required.has(tuple)) throw new Error(`unrecognized suite tuple: ${tuple}`);
    if (tuples.has(tuple)) throw new Error(`duplicate suite tuple: ${tuple}`);
    tuples.add(tuple);
  }
  for (const tuple of required)
    if (!tuples.has(tuple)) throw new Error(`missing suite tuple: ${tuple}`);
}

function requiredTuples(catalog: {
  scriptCases: CatalogCase[];
  visualAspects: CatalogCase[];
}): Set<string> {
  const result = new Set<string>();
  const viewports = [
    { width: 1280, height: 900, scale: 1 },
    { width: 960, height: 720, scale: 1 },
    { width: 620, height: 900, scale: 1 },
  ];
  for (const entry of [...catalog.visualAspects, ...catalog.scriptCases]) {
    const states =
      entry.mode === "script" ? [entry.expectedStateId ?? ""] : (entry.boundaryStates ?? []);
    for (const state of states)
      for (const viewport of viewports) result.add(tupleKey(entry.id, state, viewport));
  }
  return result;
}

function tupleKey(caseId: string, state: string, viewport: StageViewport): string {
  return `${caseId}/${state}/${viewport.width}x${viewport.height}@${viewport.scale}`;
}

function sameViewport(value: JsonValue | undefined, expected: StageViewport): boolean {
  const viewport = record(value, "image viewport");
  return (
    viewport.width === expected.width &&
    viewport.height === expected.height &&
    viewport.scale === expected.scale
  );
}

function fsFile(value: string): boolean {
  return path.isAbsolute(value) && requireFile(value);
}

function requireFile(value: string): boolean {
  try {
    return fs.statSync(value).isFile() && !fs.lstatSync(value).isSymbolicLink();
  } catch {
    return false;
  }
}

function record(value: JsonValue | undefined, name: string): { [key: string]: JsonValue } {
  if (typeof value !== "object" || value === null || Array.isArray(value))
    throw new Error(`invalid ${name}`);
  return value;
}

function records(value: JsonValue | undefined, name: string): Array<{ [key: string]: JsonValue }> {
  if (!Array.isArray(value) || value.some((item) => !isRecord(item)))
    throw new Error(`invalid ${name}`);
  return value as Array<{ [key: string]: JsonValue }>;
}

function isRecord(value: JsonValue): value is { [key: string]: JsonValue } {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function required(value: JsonValue | undefined, name: string): JsonValue {
  if (value === undefined) throw new Error(`missing ${name}`);
  return value;
}

function text(value: JsonValue | undefined, name: string): string {
  if (typeof value !== "string" || value.length === 0) throw new Error(`invalid ${name}`);
  return value;
}

function sha(value: string): string {
  if (!/^[0-9a-f]{64}$/.test(value)) throw new Error("invalid SHA-256");
  return value;
}

function positive(value: JsonValue | undefined, name: string): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value <= 0)
    throw new Error(`invalid ${name}`);
  return value;
}
