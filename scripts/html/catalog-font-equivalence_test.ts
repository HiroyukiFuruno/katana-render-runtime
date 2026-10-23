import { expect, test } from "bun:test";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { catalogFontAcceptance, compareCatalogFontFaces } from "./catalog-font-equivalence";
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
  expect(
    validateFontProvenance([
      {
        ...face,
        layoutRuns: [
          {
            paintCall: 2,
            treePath: "root/0",
            faceId: "face-0",
            glyphCount: 3,
            layoutVariations: [{ tag: "wght", value: 700 }],
            derivedEffectiveVariations: [{ tag: "wght", value: 700 }],
            derivedNormalizedCoords: [0.5],
          },
        ],
      },
    ])[0]?.layoutRuns?.[0]?.faceId,
  ).toBe("face-0");
  expect(() =>
    validateFontProvenance([
      {
        ...face,
        layoutRuns: [
          {
            paintCall: 2,
            treePath: "root/0",
            faceId: "face-0",
            glyphCount: 3,
            layoutVariations: [
              { tag: "wght", value: 700 },
              { tag: "wght", value: 701 },
            ],
            derivedEffectiveVariations: [{ tag: "wght", value: 700 }],
            derivedNormalizedCoords: [0.5],
          },
        ],
      },
    ]),
  ).toThrow();
  expect(() =>
    validateFontProvenance([
      {
        ...face,
        layoutRuns: [
          {
            paintCall: 2,
            treePath: "root/0",
            faceId: "face-0",
            glyphCount: 3,
            layoutVariations: [{ tag: "bad", value: 700 }],
            derivedEffectiveVariations: [{ tag: "bad", value: 700 }],
            derivedNormalizedCoords: [0.5],
          },
        ],
      },
    ]),
  ).toThrow();
  expect(() =>
    validateFontProvenance([
      {
        ...face,
        layoutRuns: [
          {
            paintCall: 2,
            treePath: "root/0",
            faceId: "face-0",
            glyphCount: 3,
            layoutVariations: [{ tag: "wght", value: 700 }],
            derivedEffectiveVariations: [{ tag: "wght", value: 700 }],
            derivedNormalizedCoords: [Number.POSITIVE_INFINITY],
          },
        ],
      },
    ]),
  ).toThrow();
});
