import { describe, expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  captureDynamicStatePair,
  captureKrrState,
  readKrrDynamicState,
  runDynamicFidelity,
} from "./dynamic-fidelity";
import type { JsonValue } from "./dynamic-state";
import {
  assertFeatureResultReportsComplete,
  type FeatureCaseResult,
  loadFeatureCatalog,
  readExpectedState,
} from "./feature-catalog";
import { fidelityAcceptancePlan, runFidelityAcceptance } from "./fidelity-acceptance-cli";
import { sha256File } from "./reference-capture";

const SOURCE_COMMENT = "\n<!-- html-pair-input-review-marker -->\n";

async function withDynamicFixture(
  action: (fixture: string, directory: string) => void | Promise<void>,
): Promise<void> {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "dynamic-paired-input-"));
  const fixture = path.join(directory, "fixture.html");
  fs.copyFileSync("tests/fixtures/html/aspects/script-dynamic-dom.html", fixture);
  try {
    await action(fixture, directory);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
}

describe("dynamic capture input binding", () => {
  test("pins the same HTML identity for both captures and result evidence", async () => {
    await withDynamicFixture(async (fixture) => {
      const expectedSha = sha256File(fixture);
      const expected = readExpectedState(fixture);
      const captureOrder: string[] = [];
      const pair = await captureDynamicStatePair(
        fixture,
        async () => {
          captureOrder.push("chrome");
          return expected;
        },
        (sourceSha) => {
          captureOrder.push("krr");
          expect(sourceSha).toBe(expectedSha);
          return expected;
        },
      );
      expect(captureOrder).toEqual(["chrome", "krr"]);
      expect(pair).toEqual({
        sourceSha256: expectedSha,
        chromeState: expected,
        krrState: expected,
      });
    });
  });

  for (const mutationPhase of ["chrome", "krr"] as const) {
    test(`rejects a non-painted comment mutation during ${mutationPhase} capture`, async () => {
      await withDynamicFixture(async (fixture) => {
        const originalSha = sha256File(fixture);
        const expected = readExpectedState(fixture);
        let krrCaptures = 0;
        const mutate = (): JsonValue => {
          fs.appendFileSync(fixture, SOURCE_COMMENT);
          expect(sha256File(fixture)).not.toBe(originalSha);
          expect(readExpectedState(fixture)).toEqual(expected);
          return expected;
        };
        await expect(
          captureDynamicStatePair(
            fixture,
            async () => (mutationPhase === "chrome" ? mutate() : expected),
            () => {
              krrCaptures += 1;
              return mutationPhase === "krr" ? mutate() : expected;
            },
          ),
        ).rejects.toThrow("HTML input SHA changed");
        expect(krrCaptures).toBe(mutationPhase === "chrome" ? 0 : 1);
      });
    });
  }

  test("rejects failed Chrome capture without starting KRR", async () => {
    await withDynamicFixture(async (fixture) => {
      let krrStarted = false;
      await expect(
        captureDynamicStatePair(
          fixture,
          async () => {
            throw new Error("Chrome capture failed");
          },
          () => {
            krrStarted = true;
            return null;
          },
        ),
      ).rejects.toThrow("Chrome capture failed");
      expect(krrStarted).toBe(false);
    });
  });

  test("rejects changed pinned input before touching the KRR executable", async () => {
    await withDynamicFixture((fixture, directory) => {
      const pinnedSha = sha256File(fixture);
      fs.appendFileSync(fixture, SOURCE_COMMENT);
      expect(() =>
        captureKrrState(
          path.join(directory, "missing-probe"),
          fixture,
          "null",
          path.join(directory, "unused-report.json"),
          "dynamic-dom",
          { width: 1280, height: 900 },
          pinnedSha,
        ),
      ).toThrow("HTML input SHA changed");
    });
  });

  test("rejects a producer report hash differing from the pinned HTML", async () => {
    await withDynamicFixture((fixture, directory) => {
      const reportPath = path.join(directory, "report.json");
      const capturePath = path.join(directory, "capture.png");
      const sourceSha = sha256File(fixture);
      const state = readExpectedState(fixture);
      const report = {
        executionNonce: "capture-nonce",
        htmlSha256: sourceSha,
        binarySha256: "b".repeat(64),
        state,
        capturePath,
      };
      fs.writeFileSync(capturePath, "unit capture-presence artifact");
      fs.writeFileSync(reportPath, JSON.stringify(report));
      expect(
        readKrrDynamicState(reportPath, report.executionNonce, sourceSha, report.binarySha256),
      ).toEqual(state);
      fs.appendFileSync(fixture, SOURCE_COMMENT);
      fs.writeFileSync(reportPath, JSON.stringify({ ...report, htmlSha256: sha256File(fixture) }));
      expect(() =>
        readKrrDynamicState(reportPath, report.executionNonce, sourceSha, report.binarySha256),
      ).toThrow("Stale dynamic report");
    });
  });
});

describe("dynamic HTML fidelity phases", () => {
  test("accepts only immutable final and interim thresholds before browser capture", async () => {
    await expect(runDynamicFidelity("", "", 90)).rejects.toThrow(/final.*99/);
    await expect(runDynamicFidelity("", "", 99, { phase: "interim" })).rejects.toThrow(
      /interim.*90/,
    );
    await expect(
      runDynamicFidelity("", "", 90, { phase: "unrecognized" } as never),
    ).rejects.toThrow(/unsupported.*phase/);
  });

  test("final feature consumer rejects an interim envelope and accepts the final array", () => {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), "dynamic-fidelity-phase-"));
    try {
      const catalog = loadFeatureCatalog();
      const toResult = (entry: (typeof catalog.visualAspects)[number]): FeatureCaseResult => ({
        caseId: entry.id,
        status: "passed",
        features: Object.fromEntries(entry.features.map((feature) => [feature, "passed"])),
      });
      const visualResults = catalog.visualAspects.map(toResult);
      const scriptResults = catalog.scriptCases.map(toResult);
      const visualPath = path.join(directory, "visual-results.json");
      const finalScriptPath = path.join(directory, "final-script-results.json");
      const interimScriptPath = path.join(directory, "interim-script-results.json");
      fs.writeFileSync(visualPath, JSON.stringify(visualResults));
      fs.writeFileSync(finalScriptPath, JSON.stringify(scriptResults));
      fs.writeFileSync(
        interimScriptPath,
        JSON.stringify({ phase: "interim", threshold: 90, results: scriptResults }),
      );

      expect(() =>
        assertFeatureResultReportsComplete(catalog, visualPath, finalScriptPath),
      ).not.toThrow();
      expect(() =>
        assertFeatureResultReportsComplete(catalog, visualPath, interimScriptPath),
      ).toThrow(/invalid feature results/);
    } finally {
      fs.rmSync(directory, { recursive: true, force: true });
    }
  });
});

describe("fixed fidelity acceptance CLI routes", () => {
  test.each([
    ["input-state-final", "input-state", "tmp/html-fidelity/input-state", 99, undefined],
    [
      "input-state-interim",
      "input-state",
      "tmp/html-fidelity/input-state-interim",
      90,
      { phase: "interim" },
    ],
    ["dynamic-final", "dynamic", "tmp/html-fidelity/dynamic", 99, undefined],
    ["dynamic-interim", "dynamic", "tmp/html-fidelity/dynamic-interim", 90, { phase: "interim" }],
  ] as const)(
    "%s binds its fixed input, output, score and phase",
    (mode, kind, output, threshold, options) => {
      const buildReport = `${output}/test-build.jsonl`;
      const expected = { kind, buildReport, output, threshold, ...(options ? { options } : {}) };
      expect(fidelityAcceptancePlan([mode, buildReport])).toEqual(expected);
    },
  );

  test.each([
    [
      "reports-final",
      "final",
      "tmp/html-fidelity/visual-feature-results.json",
      "tmp/html-fidelity/dynamic/script-feature-results.json",
    ],
    [
      "reports-interim",
      "interim",
      "tmp/html-fidelity/interim/interim-visual-feature-results.json",
      "tmp/html-fidelity/dynamic-interim/interim-script-feature-results.json",
    ],
  ] as const)("%s binds both report paths", (mode, phase, visualReport, scriptReport) => {
    expect(fidelityAcceptancePlan([mode])).toEqual({
      kind: "reports",
      phase,
      visualReport,
      scriptReport,
    });
  });

  test("returned capture plans cannot change the fixed routes", () => {
    const argv = ["dynamic-interim", "tmp/html-fidelity/dynamic-interim/test-build.jsonl"];
    const expected = fidelityAcceptancePlan(argv);
    const changed = fidelityAcceptancePlan(argv);
    if (changed.kind === "reports") throw new Error("expected dynamic capture route");
    changed.output = "alternative-output";
    changed.threshold = 99;
    if (changed.options) Object.assign(changed.options, { phase: "final" });
    expect(fidelityAcceptancePlan(argv)).toEqual(expected);
  });

  test.each([
    { argv: [] },
    { argv: ["unsupported"] },
    { argv: ["input-state-final"] },
    { argv: ["dynamic-interim", "tmp/html-fidelity/dynamic/test-build.jsonl"] },
    { argv: ["dynamic-final", ""] },
    { argv: ["dynamic-final", "tmp/html-fidelity/dynamic/test-build.jsonl", "--min-score", "90"] },
    { argv: ["reports-final", "--phase", "interim"] },
    { argv: ["reports-interim", "alternative-report.json"] },
  ])("rejects unsupported arguments %j before any capture", async ({ argv }) => {
    expect(() => fidelityAcceptancePlan(argv)).toThrow();
    await expect(runFidelityAcceptance(argv)).rejects.toThrow();
  });
});
