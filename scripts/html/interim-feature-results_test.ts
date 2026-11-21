import { expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { assertFeatureResultReportsComplete, loadFeatureCatalog } from "./feature-catalog";
import { parseSuiteOptions } from "./fidelity-suite";
import { assertInterimFeatureResultReportsComplete } from "./interim-feature-results";

test("integrated suite defaults to final99 and requires explicit immutable interim90", () => {
  expect(parseSuiteOptions(["--binary", "unused"]).minScore).toBe(99);
  expect(() => parseSuiteOptions(["--binary", "unused", "--min-score", "90"])).toThrow(/final.*99/);
  expect(
    parseSuiteOptions(["--binary", "unused", "--phase", "interim", "--min-score", "90"]).phase,
  ).toBe("interim");
  expect(() =>
    parseSuiteOptions(["--binary", "unused", "--phase", "interim", "--min-score", "99"]),
  ).toThrow(/interim.*90/);
});

test("interim consumer preserves full catalog coverage and final refuses interim evidence", () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "interim-release-control-"));
  const catalog = loadFeatureCatalog();
  const result = (entry: (typeof catalog.visualAspects)[number]) => ({
    caseId: entry.id,
    status: "passed",
    features: Object.fromEntries(entry.features.map((feature) => [feature, "passed"])),
  });
  const visual = catalog.visualAspects.map(result);
  const script = catalog.scriptCases.map(result);
  const visualPath = path.join(directory, "visual.json");
  const scriptPath = path.join(directory, "script.json");
  const write = (phase = "interim", threshold = 90, rows = script) => {
    fs.writeFileSync(
      visualPath,
      JSON.stringify({ phase: "interim", threshold: 90, results: visual }),
    );
    fs.writeFileSync(scriptPath, JSON.stringify({ phase, threshold, results: rows }));
  };
  try {
    write();
    expect(() =>
      assertInterimFeatureResultReportsComplete(catalog, visualPath, scriptPath),
    ).not.toThrow();
    expect(() => assertFeatureResultReportsComplete(catalog, visualPath, scriptPath)).toThrow(
      /invalid feature results/,
    );
    write("final", 99);
    expect(() =>
      assertInterimFeatureResultReportsComplete(catalog, visualPath, scriptPath),
    ).toThrow(/phase\/90/);
    write("interim", 99);
    expect(() =>
      assertInterimFeatureResultReportsComplete(catalog, visualPath, scriptPath),
    ).toThrow(/phase\/90/);
    write("interim", 90, script.slice(1));
    expect(() =>
      assertInterimFeatureResultReportsComplete(catalog, visualPath, scriptPath),
    ).toThrow(/unrun/);
    const firstScript = script[0];
    if (!firstScript) throw new Error("catalog has no script case");
    write("interim", 90, [...script, firstScript]);
    expect(() =>
      assertInterimFeatureResultReportsComplete(catalog, visualPath, scriptPath),
    ).toThrow(/duplicate/);
    const failed = script.map((row) => ({ ...row, features: { ...row.features } }));
    const first = failed[0];
    if (!first) throw new Error("catalog has no script case");
    const firstFeature = Object.keys(first.features)[0];
    if (!firstFeature) throw new Error("script case has no feature");
    first.features[firstFeature] = "unverified";
    write("interim", 90, failed);
    expect(() =>
      assertInterimFeatureResultReportsComplete(catalog, visualPath, scriptPath),
    ).toThrow(/not verified|unverified/);
    fs.writeFileSync(scriptPath, JSON.stringify(script));
    expect(() =>
      assertInterimFeatureResultReportsComplete(catalog, visualPath, scriptPath),
    ).toThrow(/phase\/90/);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});
