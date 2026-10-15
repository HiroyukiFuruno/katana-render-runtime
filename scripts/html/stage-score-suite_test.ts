import { expect, test } from "bun:test";
import fs from "node:fs";
import path from "node:path";
import type { JsonValue } from "./dynamic-state";
import { type CatalogCase, loadFeatureCatalog } from "./feature-catalog";
import { sha256File } from "./reference-capture";
import { assertStageScoreSuite } from "./stage-score-suite";

type JsonObject = { [key: string]: JsonValue };
type Viewport = { width: number; height: number; scale: number };

const viewports: Viewport[] = [
  { width: 1280, height: 900, scale: 1 },
  { width: 960, height: 720, scale: 1 },
  { width: 620, height: 900, scale: 1 },
];

function suiteFixture(): { value: JsonObject; trustedPlan: JsonObject; cleanup: () => void } {
  const temporary = fs.mkdtempSync(path.resolve("tmp/stage-score-suite-"));
  const sourceRoot = path.resolve("tests/fixtures/html");
  const binary = process.execPath;
  const binarySha = sha256File(binary);
  const catalog = loadFeatureCatalog();
  const conditions: JsonObject[] = [];
  const trustedConditions: JsonObject[] = [];
  let index = 0;
  for (const entry of [...catalog.visualAspects, ...catalog.scriptCases]) {
    const states =
      entry.mode === "script"
        ? [entry.expectedStateId as string]
        : (entry.boundaryStates as string[]);
    for (const state of states) {
      for (const viewport of viewports) {
        const condition = conditionFixture(
          entry,
          state,
          viewport,
          index,
          temporary,
          sourceRoot,
          binarySha,
        );
        conditions.push(condition.candidate);
        trustedConditions.push(condition.trusted);
        index += 1;
      }
    }
  }
  const catalogPath = path.resolve("tests/fixtures/html/feature-catalog.json");
  return {
    value: {
      profile: "stage90-suite",
      threshold: 90,
      acceptance: false,
      catalogPath,
      catalogSha256: sha256File(catalogPath),
      conditions,
    },
    trustedPlan: {
      profile: "stage90-trusted-plan",
      conditions: trustedConditions,
    },
    cleanup: () => fs.rmSync(temporary, { recursive: true, force: true }),
  };
}

function conditionFixture(
  entry: CatalogCase,
  state: string,
  viewport: Viewport,
  index: number,
  temporary: string,
  sourceRoot: string,
  binarySha: string,
): { candidate: JsonObject; trusted: JsonObject } {
  const png = viewportPng(temporary, viewport);
  const sourceSha = sha256File(path.resolve(sourceRoot, entry.fixture));
  const nonce = `suite-${index}`;
  const key = `${entry.id}/${viewport.width}x${viewport.height}@${viewport.scale}/${state}/scroll/0`;
  const evidencePath = path.join(
    temporary,
    `${entry.id}-${viewport.width}x${viewport.height}-${state}.json`,
  );
  const evidence: JsonObject = {
    status: "matched",
    sourceSha256: sourceSha,
    binarySha256: binarySha,
    caseId: entry.id,
    state,
    actualScrollY: 0,
    maxScrollY: 0,
    targetVisible: true,
    nonce,
    candidateSha256: sha256File(png),
    key,
    kind: "scroll",
    viewport,
    requestedY: 0,
    observedHash: "#target",
  };
  fs.writeFileSync(evidencePath, JSON.stringify(evidence));
  const image: JsonObject = {
    key,
    caseId: entry.id,
    state,
    kind: "scroll",
    actualScrollY: 0,
    maxScrollY: 0,
    requestedY: 0,
    viewportHeight: viewport.height,
    viewport,
    sourceSha256: sourceSha,
    binarySha256: binarySha,
    targetVisible: true,
    nonce,
    observedHash: "#target",
    referencePath: png,
    referenceSha256: sha256File(png),
    candidatePath: png,
    candidateSha256: sha256File(png),
    rmseScore: 100,
    maeScore: 100,
    operationEvidencePath: evidencePath,
    operationEvidenceSha256: sha256File(evidencePath),
    operationState: "matched",
    evidenceId: `evidence-${index}`,
  };
  const performance: JsonObject = {
    environment: "test",
    conditionId: `${entry.id}-${viewport.width}x${viewport.height}-${state}`,
    phases: ["cold", "warm", "re-display", "full-Y", "post-operation"].map((name) => ({
      name,
      observedMs: 1,
      baselineMs: 1,
      observedMemoryBytes: 1,
      baselineMemoryBytes: 1,
      measurementPairs: 1,
    })),
  };
  const manifest: JsonObject = {
    profile: "stage90",
    threshold: 90,
    sourceSha256: sourceSha,
    binarySha256: binarySha,
    images: [image],
    tocKeys: [],
    performance,
  };
  const condition = {
    conditionId: `${entry.id}-${viewport.width}x${viewport.height}-${state}`,
    caseId: entry.id,
    state,
    viewport,
    sourceSha256: sourceSha,
    binarySha256: binarySha,
    binaryPath: process.execPath,
    candidate: manifest,
  };
  const { candidate, ...tuple } = condition;
  return {
    candidate: condition,
    trusted: { ...tuple, trusted: manifest },
  };
}

function viewportPng(temporary: string, viewport: Viewport): string {
  const png = path.join(temporary, `${viewport.width}x${viewport.height}.png`);
  if (fs.existsSync(png)) return png;
  const result = Bun.spawnSync([
    "magick",
    "-size",
    `${viewport.width}x${viewport.height}`,
    "xc:white",
    png,
  ]);
  if (result.exitCode !== 0)
    throw new Error(`failed to create ${viewport.width}x${viewport.height} PNG`);
  return png;
}

test("accepts every catalog state and viewport as an independent stage condition", () => {
  const fixture = suiteFixture();
  try {
    expect(() =>
      assertStageScoreSuite(fixture.value as JsonValue, fixture.trustedPlan as JsonValue),
    ).not.toThrow();
  } finally {
    fixture.cleanup();
  }
}, 30000);

test("rejects missing, duplicate, unrecognized, and mismatched condition provenance", () => {
  const fixture = suiteFixture();
  try {
    const conditions = fixture.value.conditions as JsonValue[];
    const first = conditions[0] as JsonObject;
    expect(() =>
      assertStageScoreSuite(
        { ...fixture.value, conditions: conditions.slice(1) } as JsonValue,
        fixture.trustedPlan as JsonValue,
      ),
    ).toThrow("missing suite tuple");
    expect(() =>
      assertStageScoreSuite(
        {
          ...fixture.value,
          conditions: [first, { ...first, conditionId: "different-id" }, ...conditions.slice(1)],
        } as JsonValue,
        fixture.trustedPlan as JsonValue,
      ),
    ).toThrow("duplicate suite tuple");
    expect(() =>
      assertStageScoreSuite(
        {
          ...fixture.value,
          conditions: conditions.map((condition, index) =>
            index === 0 ? { ...(condition as JsonObject), state: "unrecognized-state" } : condition,
          ),
        } as JsonValue,
        fixture.trustedPlan as JsonValue,
      ),
    ).toThrow("unrecognized suite tuple");
    expect(() =>
      assertStageScoreSuite(
        {
          ...fixture.value,
          conditions: conditions.map((condition, index) =>
            index === 0
              ? { ...(condition as JsonObject), sourceSha256: "0".repeat(64) }
              : condition,
          ),
        } as JsonValue,
        fixture.trustedPlan as JsonValue,
      ),
    ).toThrow("condition source SHA differs");
    expect(() =>
      assertStageScoreSuite(
        {
          ...fixture.value,
          conditions: conditions.map((condition, index) =>
            index === 0
              ? { ...(condition as JsonObject), binarySha256: "1".repeat(64) }
              : condition,
          ),
        } as JsonValue,
        fixture.trustedPlan as JsonValue,
      ),
    ).toThrow("condition binary differs");
    expect(() =>
      assertStageScoreSuite(
        {
          ...fixture.value,
          conditions: conditions.map((condition, index) =>
            index === 0
              ? {
                  ...(condition as JsonObject),
                  trusted: (fixture.trustedPlan.conditions as JsonValue[])[0],
                }
              : condition,
          ),
        } as JsonValue,
        fixture.trustedPlan as JsonValue,
      ),
    ).toThrow("candidate suite must not include trusted data");
  } finally {
    fixture.cleanup();
  }
}, 30000);
