import { expect, test } from "bun:test";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import {
  catalogFontAcceptance,
  compareCatalogFontFaces,
  compareChromiumFontInstancesToLayoutRuns,
  compareChromiumFontInstanceToLayoutRuns,
} from "./catalog-font-equivalence";
import type { ChromiumFontInstance } from "./chromium-font-trace";
import { validateFontProvenance } from "./coretext-font-provenance";
import type { JsonValue } from "./dynamic-state";

type JsonObject = { [key: string]: JsonValue };

test("accepts the complete exact-score catalog only with established font identity", () => {
  expect(catalogFontAcceptance(Array(84).fill(90), 84, true, [])).toEqual({
    acceptance: true,
    fontIdentityEquivalence: "established",
  });
  expect(catalogFontAcceptance(Array(84).fill(90), 83, true, [])).toMatchObject({
    acceptance: false,
    fontIdentityEquivalence: "not-established",
  });
  expect(catalogFontAcceptance([89, ...Array(83).fill(90)], 84, true, [])).toMatchObject({
    acceptance: false,
    fontIdentityEquivalence: "not-established",
  });
  expect(
    catalogFontAcceptance(Array(84).fill(90), 84, false, [{ conditionId: "case" }]),
  ).toMatchObject({
    acceptance: false,
    fontIdentityEquivalence: "not-established",
  });
});

test("establishes only matching verified physical font-face sets", () => {
  const fileA = path.resolve("crates/katana-render-runtime/assets/fonts/NotoSans-Regular.ttf");
  const fileB = path.resolve(
    "crates/katana-render-runtime/assets/test-fixtures/font-probes/recursive-latin-crsv-normal.ttf",
  );
  const face = (filePath: string, faceIndex = 0): JsonObject => ({
    scope: "capture",
    family: "Fixture",
    source: "captured-font",
    filePath,
    fileSha256: createHash("sha256").update(fs.readFileSync(filePath)).digest("hex"),
    faceIndex,
    selected: true,
    fileBytesVerified: true,
  });
  const original = face(fileA);
  const sameFileOtherFamily = { ...original, family: "Alias" };
  expect(compareCatalogFontFaces([original, original], [sameFileOtherFamily]).established).toBe(
    true,
  );
  expect(compareCatalogFontFaces([original], [face(fileB)]).established).toBe(false);
  expect(compareCatalogFontFaces([original], [face(fileA, 1)]).established).toBe(false);
  expect(compareCatalogFontFaces([], []).established).toBe(false);
  expect(
    compareCatalogFontFaces([{ ...original, fileSha256: "0".repeat(64) }], [original]),
  ).toMatchObject({ established: false });
  const variableFace = {
    ...original,
    coreTextDescriptor: {
      urlPath: fileA,
      postScriptName: "Fixture-Regular",
      variationAxes: [{ identifier: 1, minimumValue: 100, maximumValue: 900, defaultValue: 400 }],
      variationCoordinates: [{ identifier: 1, value: 400 }],
    },
  } as JsonValue;
  expect(compareCatalogFontFaces([variableFace], [variableFace])).toMatchObject({
    established: false,
    reason: "variable font instance identity is not established",
  });
});

test("accepts layout runs only with finite four-character variation tags", () => {
  const filePath = path.resolve("crates/katana-render-runtime/assets/fonts/NotoSans-Regular.ttf");
  const face = {
    scope: "capture" as const,
    family: "Fixture",
    source: "captured-font",
    filePath,
    fileSha256: createHash("sha256").update(fs.readFileSync(filePath)).digest("hex"),
    faceIndex: 0,
    selected: true as const,
    fileBytesVerified: true as const,
  };
  const run = {
    paintCall: 2,
    treePath: "root/0",
    faceId: "face-0",
    glyphCount: 3,
    layoutVariations: [{ tag: "wght", value: 700 }],
    derivedEffectiveVariations: [{ tag: "wght", value: 700 }],
    derivedAppliedDesignCoordinates: [{ tag: "wght", value: 700 }],
    derivedNormalizedCoords: [0.5],
  };
  const validateRun = (overrides: Record<string, JsonValue>) =>
    validateFontProvenance([{ ...face, layoutRuns: [{ ...run, ...overrides }] }]);
  expect(validateRun({})[0]?.layoutRuns?.[0]?.faceId).toBe("face-0");
  const duplicate = [
    { tag: "wght", value: 700 },
    { tag: "wght", value: 701 },
  ];
  const invalidRuns: Record<string, JsonValue>[] = [
    { layoutVariations: duplicate },
    { derivedEffectiveVariations: duplicate },
    { derivedAppliedDesignCoordinates: duplicate },
    { layoutVariations: [{ tag: "bad", value: 700 }] },
    { derivedAppliedDesignCoordinates: [{ tag: "bad", value: 700 }] },
    { derivedNormalizedCoords: [Number.POSITIVE_INFINITY] },
    { derivedAppliedDesignCoordinates: [{ tag: "wght", value: Number.NaN }] },
  ];
  for (const overrides of invalidRuns) {
    expect(() => validateRun(overrides)).toThrow();
  }
});

test("compares Chromium instances with complete KRR applied design coordinates", () => {
  const filePath = path.resolve("crates/katana-render-runtime/assets/fonts/NotoSans-Regular.ttf");
  const fileSha256 = createHash("sha256").update(fs.readFileSync(filePath)).digest("hex");
  const face = (coordinates: { tag: string; value: number }[] | undefined): JsonObject => {
    const base: JsonObject = {
      scope: "capture",
      family: "Fixture",
      source: "captured-font",
      filePath,
      fileSha256,
      faceIndex: 0,
      selected: true,
      fileBytesVerified: true,
    };
    return coordinates === undefined
      ? base
      : {
          ...base,
          layoutRuns: [
            {
              paintCall: 1,
              treePath: "root/0",
              faceId: "face-0",
              glyphCount: 1,
              layoutVariations: coordinates,
              derivedEffectiveVariations: coordinates,
              derivedAppliedDesignCoordinates: coordinates,
              derivedNormalizedCoords: coordinates.map(() => 0),
            },
          ],
        };
  };
  const variable: ChromiumFontInstance = {
    processId: 1,
    processRole: "renderer",
    physicalFileSHA256: fileSha256,
    physicalFaceTtcIndex: 0,
    postScriptName: "Fixture-Variable",
    fvarTablePresent: true,
    axes: [
      { identifier: 2003265652, minimum: 100, maximum: 900 },
      { identifier: 1869640570, minimum: 8, maximum: 72 },
      { identifier: 2003072104, minimum: 50, maximum: 200 },
      { identifier: 1196572996, minimum: -200, maximum: 1000 },
    ],
    coordinates: [
      { identifier: 2003265652, value: 700 },
      { identifier: 1869640570, value: 14 },
      { identifier: 2003072104, value: 100 },
      { identifier: 1196572996, value: 400 },
    ],
    paintSerials: [1],
    join: { retainedPaintCount: 1, strikeCacheCount: 1, scalerFontCount: 1 },
  };
  const complete = [
    { tag: "wght", value: 700 },
    { tag: "opsz", value: 14 },
    { tag: "wdth", value: 100 },
    { tag: "GRAD", value: 400 },
  ];
  const regularCoordinates = complete.map((item) =>
    item.tag === "wght" ? { ...item, value: 400 } : item,
  );
  const regular = {
    ...variable,
    coordinates: variable.coordinates.map((item) =>
      item.identifier === 2003265652 ? { ...item, value: 400 } : item,
    ),
  };
  const cases = [
    { name: "SFNS-like four-axis instance", instance: variable, runs: complete, established: true },
    {
      name: "static empty instance",
      instance: { ...variable, fvarTablePresent: false, axes: [], coordinates: [] },
      runs: [],
      established: true,
    },
    { name: "axis missing", instance: variable, runs: complete.slice(1), established: false },
    {
      name: "axis extra",
      instance: variable,
      runs: [...complete, { tag: "slnt", value: 0 }],
      established: false,
    },
    {
      name: "axis value differs",
      instance: variable,
      runs: complete.map((item) => (item.tag === "wght" ? { ...item, value: 701 } : item)),
      established: false,
    },
    {
      name: "physical file differs",
      instance: { ...variable, physicalFileSHA256: "0".repeat(64) },
      runs: complete,
      established: false,
    },
    {
      name: "physical face differs",
      instance: { ...variable, physicalFaceTtcIndex: 1 },
      runs: complete,
      established: false,
    },
    { name: "no layout observation", instance: variable, runs: undefined, established: false },
  ];
  for (const fixture of cases) {
    expect(
      compareChromiumFontInstanceToLayoutRuns(fixture.instance, [face(fixture.runs)]),
    ).toMatchObject({
      established: fixture.established,
    });
  }
  const mixedFaces = [face(complete), face(regularCoordinates)];
  expect(compareChromiumFontInstanceToLayoutRuns(variable, mixedFaces)).toMatchObject({
    established: true,
  });
  expect(compareChromiumFontInstancesToLayoutRuns([variable, regular], mixedFaces)).toMatchObject({
    established: true,
  });
  expect(compareChromiumFontInstancesToLayoutRuns([variable], mixedFaces)).toMatchObject({
    established: false,
  });
  expect(
    compareChromiumFontInstancesToLayoutRuns([variable, regular], [face(complete)]),
  ).toMatchObject({ established: false });
});
