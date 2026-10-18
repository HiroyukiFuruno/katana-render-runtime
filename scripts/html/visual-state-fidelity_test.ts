import { describe, expect, test } from "bun:test";
import { loadFeatureCatalog } from "./feature-catalog";
import { fidelityThresholdForPhase } from "./fidelity-phase";
import { createCatalogStage90ConditionPlan } from "./stage-score-suite";
import {
  chromeFontTextNodeIds,
  createChromeFontIdentityRequest,
  INPUT_STATE_OPERATIONS,
  runInputStateFidelity,
  VISUAL_STATE_SPECS,
} from "./visual-state-fidelity";

describe("Chrome rendered text inventory", () => {
  test("visits deep text, pseudo content, shadow trees and frame documents once", () => {
    const text = { nodeId: 7, nodeType: 3 };
    const root = {
      nodeId: 1,
      nodeType: 9,
      children: [
        {
          nodeId: 2,
          nodeType: 1,
          children: [
            { nodeId: 3, nodeType: 1, children: [{ nodeId: 4, nodeType: 1, children: [text] }] },
          ],
          shadowRoots: [{ nodeId: 5, nodeType: 11, children: [{ nodeId: 8, nodeType: 3 }] }],
          pseudoElements: [
            { nodeId: 6, nodeType: 1, children: [text, { nodeId: 9, nodeType: 3 }] },
          ],
          contentDocument: { nodeId: 10, nodeType: 9, children: [{ nodeId: 11, nodeType: 3 }] },
          templateContent: { nodeId: 12, nodeType: 11, children: [{ nodeId: 13, nodeType: 3 }] },
        },
      ],
    };
    expect(chromeFontTextNodeIds(root)).toEqual([7, 8, 9, 11, 13]);
    expect(() => chromeFontTextNodeIds({ nodeId: 0, nodeType: 3 })).toThrow(/live DOM identity/);
  });
});

describe("input-state visual operation coverage", () => {
  test("covers every catalog boundary state with an executable operation", () => {
    expect(INPUT_STATE_OPERATIONS).toEqual([
      "empty",
      "filled",
      "selected",
      "expanded",
      "disabled",
      "focus-visible",
    ]);
  });
});

describe("Chrome font identity request", () => {
  const font = {
    familyName: "Fixture Sans",
    postScriptName: "FixtureSans-Regular",
    glyphCount: 7,
    isCustomFont: true,
  };
  const resource = {
    url: "file:///fixture/font.woff2",
    bodyPath: "/tmp/font.woff2",
    bytesSha256: "a".repeat(64),
  };

  test("preserves the exact planned condition order and physical resource references", () => {
    const request = createChromeFontIdentityRequest(
      [
        { conditionId: "condition-a", selectedFonts: [font], resourceFiles: [resource] },
        { conditionId: "condition-b", selectedFonts: [font], resourceFiles: [] },
      ],
      ["condition-a", "condition-b"],
    );
    expect(request.schema).toBe("krr-chrome-font-identity-request-v1");
    expect(request.captures).toEqual([
      { conditionId: "condition-a", selectedFonts: [font], resourceFiles: [resource] },
      { conditionId: "condition-b", selectedFonts: [font], resourceFiles: [] },
    ]);
  });

  test("covers the complete stage90 plan in its canonical order", () => {
    const plan = createCatalogStage90ConditionPlan();
    const conditionIds = plan.conditions.map((condition) => condition.conditionId);
    const request = createChromeFontIdentityRequest(
      conditionIds.map((conditionId) => ({
        conditionId,
        selectedFonts: [font],
        resourceFiles: [],
      })),
      conditionIds,
    );
    expect(request.captures).toHaveLength(84);
    expect(request.captures.map((capture) => capture.conditionId)).toEqual(conditionIds);
  });

  test.each([
    ["duplicate conditions", ["condition-a", "condition-a"]],
    ["out-of-order conditions", ["condition-b", "condition-a"]],
    ["missing conditions", ["condition-a"]],
  ])("rejects %s", (_label, expectedIds) => {
    expect(() =>
      createChromeFontIdentityRequest(
        [
          { conditionId: "condition-a", selectedFonts: [font], resourceFiles: [] },
          { conditionId: "condition-b", selectedFonts: [font], resourceFiles: [] },
        ],
        expectedIds,
      ),
    ).toThrow(/condition coverage/);
  });

  test.each([
    ["zero glyphs", { ...font, glyphCount: 0 }],
    ["non-integer glyphs", { ...font, glyphCount: 1.5 }],
    ["missing PostScript name", { ...font, postScriptName: "" }],
  ])("rejects a selected font with %s", (_label, selectedFont) => {
    expect(() =>
      createChromeFontIdentityRequest(
        [{ conditionId: "condition-a", selectedFonts: [selectedFont], resourceFiles: [] }],
        ["condition-a"],
      ),
    ).toThrow(/invalid evidence/);
  });

  test.each([
    ["relative body path", { ...resource, bodyPath: "font.woff2" }],
    ["invalid digest", { ...resource, bytesSha256: "not-a-sha" }],
    ["empty URL", { ...resource, url: "" }],
  ])("rejects a resource with %s", (_label, resourceFile) => {
    expect(() =>
      createChromeFontIdentityRequest(
        [{ conditionId: "condition-a", selectedFonts: [font], resourceFiles: [resourceFile] }],
        ["condition-a"],
      ),
    ).toThrow(/invalid evidence/);
  });
});

describe("catalog visual state inventory", () => {
  test("defines each of the 23 visual catalog states exactly once", () => {
    const catalog = loadFeatureCatalog();
    const required = catalog.visualAspects.flatMap((entry) =>
      (entry.boundaryStates ?? []).map((state) => `${entry.id}/${state}`),
    );
    const actual = VISUAL_STATE_SPECS.map((spec) => `${spec.caseId}/${spec.state}`);
    expect(actual).toHaveLength(23);
    expect(new Set(actual).size).toBe(23);
    expect([...actual].sort()).toEqual([...required].sort());
    expect(VISUAL_STATE_SPECS.every((spec) => spec.targetSelector && spec.expression)).toBe(true);
    expect(
      VISUAL_STATE_SPECS.some(
        (spec) => spec.caseId === "navigation" && spec.state === "all-toc-links",
      ),
    ).toBe(true);
  });
});

describe("input-state visual fidelity phases", () => {
  test("keeps final at 99 and accepts only the explicit interim threshold of 90", () => {
    expect(fidelityThresholdForPhase("final", 99)).toBe(99);
    expect(fidelityThresholdForPhase("interim", 90)).toBe(90);
    expect(() => fidelityThresholdForPhase("final", 90)).toThrow(/final.*99/);
    expect(() => fidelityThresholdForPhase("interim", 99)).toThrow(/interim.*90/);
    expect(() => fidelityThresholdForPhase("interim", 95)).toThrow(/interim.*90/);
    expect(() => fidelityThresholdForPhase("unrecognized" as never, 90)).toThrow(
      /unsupported.*phase/,
    );
  });

  test("rejects an unsupported input-state phase before starting captures", async () => {
    await expect(
      runInputStateFidelity("", "", 90, { phase: "unrecognized" } as never),
    ).rejects.toThrow(/unsupported.*phase/);
  });
});
