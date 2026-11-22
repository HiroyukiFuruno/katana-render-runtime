import { describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { catalogStage90RejectionDiagnostics } from "./catalog-stage90-rejection-diagnostics";
import { loadFeatureCatalog } from "./feature-catalog";
import { fidelityThresholdForPhase } from "./fidelity-phase";
import { createCatalogStage90ConditionPlan } from "./stage-score-suite";
import {
  catalogConditionIdentityFields,
  chromeFontTextNodeIds,
  copyCatalogStage90Receipt,
  createChromeFontIdentityRequest,
  INPUT_STATE_OPERATIONS,
  runInputStateFidelity,
  VISUAL_STATE_SPECS,
} from "./visual-state-fidelity";

describe("catalog stage90 resume receipts", () => {
  test("copies the prior unknown receipt exclusively and rejects tampered bytes", () => {
    const directory = fs.mkdtempSync(path.resolve("tmp/catalog-stage90-resume-test-"));
    try {
      const sourcePath = path.join(directory, "prior-unknown.json");
      const destinationPath = path.join(directory, "fresh-unknown.json");
      const original = Buffer.from('{"status":"unknown"}\n');
      fs.writeFileSync(sourcePath, original);
      const originalSha256 = createHash("sha256").update(original).digest("hex");
      expect(copyCatalogStage90Receipt(sourcePath, destinationPath, originalSha256)).toBe(
        originalSha256,
      );
      expect(fs.readFileSync(destinationPath)).toEqual(original);

      fs.writeFileSync(sourcePath, Buffer.from('{"status":"tampered"}\n'));
      expect(() =>
        copyCatalogStage90Receipt(
          sourcePath,
          path.join(directory, "tampered-copy.json"),
          originalSha256,
        ),
      ).toThrow(/receipt changed/);
      expect(fs.readFileSync(destinationPath)).toEqual(original);
    } finally {
      fs.rmSync(directory, { recursive: true, force: true });
    }
  });
});

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

describe("catalog stage90 rejection diagnostics", () => {
  const completeRow = {
    status: "unverified",
    krrReportPath: "/private/krr.json",
    krrReportSha256: "a".repeat(64),
    krrCapturePath: "/private/krr.png",
    krrCaptureSha256: "b".repeat(64),
    fontIdentityObservation: { status: "observed", unknownReasons: [] },
    resourceProvenance: { inventoryComplete: true },
  };

  test("emits only canonical unverified condition IDs with fixed categories", () => {
    expect(
      catalogStage90RejectionDiagnostics(
        ["canonical-capture", "canonical-font", "canonical-captured"],
        [
          { ...completeRow, error: "secret path /private/error" },
          {
            ...completeRow,
            unknownReason: "Error: physical font file and face identities differ",
          },
          { ...completeRow, status: "captured" },
        ],
      ),
    ).toEqual({
      schema: "krr-html84-stage90-rejection-diagnostics-v2",
      conditions: [
        { conditionId: "canonical-capture", category: "capture-exception" },
        {
          conditionId: "canonical-font",
          category: "krr-chrome-mismatch",
          detail: "chrome-krr-face-mismatch",
        },
      ],
    });
  });

  test("classifies capture boundaries and redacts arbitrary rejection text", () => {
    const diagnostics = catalogStage90RejectionDiagnostics(
      ["missing-krr", "resource", "observer", "fallback"],
      [
        {
          status: "unverified",
          krrReportPath: "/private/krr.json",
          krrReportSha256: "a".repeat(64),
        },
        {
          ...completeRow,
          fontIdentityObservation: { status: "unknown", unknownReasons: ["sensitive CDP detail"] },
          resourceProvenance: { inventoryComplete: false },
        },
        {
          ...completeRow,
          unknownReason: "Error: trace SHA256 mismatch",
        },
        {
          ...completeRow,
          unknownReason: "malicious /private/path with coordinates 12,34",
        },
      ],
    );
    expect(diagnostics.conditions.map((condition) => condition.category)).toEqual([
      "missing-krr-capture",
      "resource-or-cdp-problem",
      "rejected-observer",
      "generic-verification-rejection",
    ]);
    expect(JSON.stringify(diagnostics)).not.toContain("sensitive CDP detail");
    expect(JSON.stringify(diagnostics)).not.toContain("/private/path");
  });

  test("identifies an observer failure and a font-probe failure using fixed details", () => {
    const diagnostics = catalogStage90RejectionDiagnostics(
      ["visual", "events"],
      [
        {
          error:
            "ObserverBrowserFailure: observer Chrome evidence validation failed: Error: used strike has no joined scaler: observer-scaler-file-uri-needs-normalization\n/private/trace.json",
        },
        {
          ...completeRow,
          fontIdentityUnknownReasons: [
            "private-name: no exact physical face matched the CDP PostScript name",
          ],
        },
      ],
    );
    expect(diagnostics.conditions).toEqual([
      {
        conditionId: "visual",
        category: "capture-exception",
        detail: "observer-scaler-file-uri-needs-normalization",
      },
      {
        conditionId: "events",
        category: "generic-verification-rejection",
        detail: "font-probe-no-physical-face",
      },
    ]);
  });

  test("reports only allowlisted stages of font reconciliation", () => {
    const diagnostics = catalogStage90RejectionDiagnostics(
      ["observed-stage", "untrusted-stage"],
      [
        { ...completeRow, fontIdentityFailureStage: "font-reconciliation-krr-receipt" },
        { ...completeRow, fontIdentityFailureStage: "/private/evidence.json" },
      ],
    );
    expect(diagnostics.conditions.map((condition) => condition.detail)).toEqual([
      "font-reconciliation-krr-receipt",
      undefined,
    ]);
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

  test("serializes planned target selectors for both targeted and untargeted rows", () => {
    const plan = createCatalogStage90ConditionPlan();
    const targeted = plan.conditions.find((condition) => condition.targetSelector);
    const untargeted = plan.conditions.find((condition) => !condition.targetSelector);
    if (!targeted || !untargeted) throw new Error("plan lacks target selector boundary cases");
    const binarySha256 = "b".repeat(64);
    const targetedRoundTrip = JSON.parse(
      JSON.stringify(catalogConditionIdentityFields(targeted, targeted.sourceSha256, binarySha256)),
    ) as Record<string, unknown>;
    const untargetedRoundTrip = JSON.parse(
      JSON.stringify(
        catalogConditionIdentityFields(untargeted, untargeted.sourceSha256, binarySha256),
      ),
    ) as Record<string, unknown>;
    expect(targetedRoundTrip.targetSelector).toBe(targeted.targetSelector);
    expect(untargetedRoundTrip).not.toHaveProperty("targetSelector");
    expect(targetedRoundTrip.operationKind).toBe(targeted.operationKind);
    expect(untargetedRoundTrip.operationKind).toBe(untargeted.operationKind);
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
