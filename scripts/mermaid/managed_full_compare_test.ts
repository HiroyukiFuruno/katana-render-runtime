import { expect, test } from "bun:test";
import fs from "node:fs";
import path from "node:path";
import {
  assertCallerSourceBinding,
  type LanguageResult,
  runLanguageComparisons,
} from "./managed-full-compare";
import {
  assertManagedOutputRoot,
  captureCanonicalFixtureSources,
  parseManagedFullCompareRequest,
  validatePinnedInputs,
} from "./managed-full-compare-input";
import type { LanguageAcceptance } from "./managed-full-compare-language";
import { candidateFixtureArgumentPairs } from "./managed-full-compare-language";
import {
  type ManagedCommandResult,
  parseChildJson,
  requireScoreProcessOutcome,
  runManagedCommand,
} from "./managed-full-compare-process";
import {
  type FullScoreAcceptance,
  requireFullScoreAcceptance,
  validateLanguageScoreEvidence,
} from "./managed-full-compare-results";
import { fileSha256 } from "./mermaid-pair-manifest";
import { digestFileRecords } from "./native-build-receipt";
import type { ReferenceScoreRow } from "./reference_score";
import { ReferenceScorePolicy } from "./reference_score_policy";

function typedLanguageAcceptance(language: "en" | "ja"): LanguageAcceptance {
  const pin = { path: `/tmp/${language}-fixture`, sha256: "0".repeat(64), size: 0 };
  return {
    language,
    fixtureCount: language === "en" ? 59 : 58,
    minimumScore: 100,
    accepted: true,
    failedScores: [],
    fixtureSources: [],
    buildReceipt: pin.path,
    nativeBindings: pin,
    rasterReceipt: pin,
    rasterProcessEvidence: pin,
    comparison: pin,
    bindingConfig: pin,
    nativeFrameBuildProof: pin,
  };
}

test("managed full aggregation requires exactly EN59 and JA58 strict scores at least 99", () => {
  for (const [language, count] of [
    ["en", 59],
    ["ja", 58],
  ] as const) {
    const expected = slugs(count);
    const evidence = validEvidence(expected);
    const result = validateLanguageScoreEvidence(
      language,
      expected,
      evidence.scores,
      evidence.inventory,
      evidence.context,
    );
    expect(result.fixtureCount).toBe(count);
    expect(result.minimumScore).toBe(99);
  }
});

test("language failure is recorded before the next language runs and keeps acceptance false", async () => {
  const calls: string[] = [],
    verifications: string[] = [];
  const results = await runLanguageComparisons(
    async (language): Promise<LanguageAcceptance> => {
      calls.push(language);
      if (language === "en") throw new Error("EN native PNG dimensions differ");
      return typedLanguageAcceptance(language);
    },
    () => {
      verifications.push("verified");
    },
  );
  expect(calls).toEqual(["en", "ja"]);
  expect(verifications).toEqual(["verified"]);
  expect(results[0]).toEqual({
    kind: "failure",
    language: "en",
    error: "EN native PNG dimensions differ",
    accepted: false,
  });
  expect(results[0]).toBeDefined();
  expect(results[0]?.accepted).toBe(false);
});

test("shared state failure after a language error stops the next language", async () => {
  const calls: string[] = [];
  await expect(
    runLanguageComparisons(
      async (language): Promise<LanguageAcceptance> => {
        calls.push(language);
        throw new Error("language-local failure");
      },
      () => {
        throw new Error("source or build state changed");
      },
    ),
  ).rejects.toThrow("source or build state changed");
  expect(calls).toEqual(["en"]);
});

test("shared state failure after a later language error preserves prior results", async () => {
  const calls: string[] = [],
    retained: LanguageResult[] = [];
  await expect(
    runLanguageComparisons(
      async (language): Promise<LanguageAcceptance> => {
        calls.push(language);
        if (language === "ja") throw new Error("JA native comparison failed");
        return typedLanguageAcceptance(language);
      },
      () => {
        throw new Error("source or build state changed");
      },
      (result) => retained.push(result),
    ),
  ).rejects.toThrow("source or build state changed");
  expect(calls).toEqual(["en", "ja"]);
  expect(retained).toHaveLength(2);
  expect(retained[0]).toMatchObject({ language: "en", accepted: true });
  expect(retained[1]).toEqual({
    kind: "failure",
    language: "ja",
    error: "JA native comparison failed",
    accepted: false,
  });
});

test("managed build binds source records and digest, not source-manifest serialization", () => {
  const sourceFiles = [
    { path: "/repo/crates/one.rs", sha256: "a".repeat(64), size: 10 },
    { path: "/repo/crates/two.rs", sha256: "b".repeat(64), size: 20 },
  ];
  const sourceDigest = digestFileRecords(sourceFiles);
  const callerManifestSha256 = "c".repeat(64);
  const summary = {
    sourceManifestSha256: "d".repeat(64),
    sourceDigest,
    sourceFiles,
  };

  expect(summary.sourceManifestSha256).not.toBe(callerManifestSha256);
  expect(() => assertCallerSourceBinding(summary, sourceDigest, sourceFiles)).not.toThrow();

  const [firstSourceFile, secondSourceFile] = sourceFiles;
  if (firstSourceFile === undefined || secondSourceFile === undefined) {
    throw new Error("Test source fixture is incomplete");
  }
  const changedFiles = [{ ...firstSourceFile, sha256: "e".repeat(64) }, secondSourceFile];
  const rejected = [
    { ...summary, sourceDigest: digestFileRecords(changedFiles), sourceFiles: changedFiles },
    { ...summary, sourceDigest: "f".repeat(64) },
    { ...summary, sourceFiles: changedFiles },
  ];
  for (const value of rejected) {
    expect(() => assertCallerSourceBinding(value, sourceDigest, sourceFiles)).toThrow();
  }
});

test("managed native render and raster use the same copied nonempty fixtures", () => {
  const candidateFixtures = path.resolve(
    process.cwd(),
    "tmp/managed-language-test/canonical-nonempty-fixtures",
  );
  const argumentsByProducer = candidateFixtureArgumentPairs(candidateFixtures);

  expect(argumentsByProducer.nativeRender).toEqual(["--fixtures-dir", candidateFixtures]);
  expect(argumentsByProducer.rasterProcess).toEqual(["--fixtures", candidateFixtures]);
  expect(argumentsByProducer.nativeRender[1]).toBe(argumentsByProducer.rasterProcess[1]);
});

test("managed native frame verification and capture follow raster PNG production", () => {
  const source = fs.readFileSync(
      path.join(process.cwd(), "scripts/mermaid/managed-full-compare-language.ts"),
      "utf8",
    ),
    rasterProcess = source.indexOf("-raster-process`"),
    rasterOutputValidation = source.indexOf(
      "if (!fs.existsSync(rasterEvidence) || !fs.existsSync(rasterReceipt))",
    ),
    sidecarValidation = source.indexOf(
      "assertNativeFrameInputs(nativeEvidenceValue, svgDirectory, rasterPairs, slugs)",
    ),
    frameCapture = source.indexOf("const captured = captureNativeFrame(");

  expect(rasterProcess).toBeGreaterThanOrEqual(0);
  expect(rasterOutputValidation).toBeGreaterThan(rasterProcess);
  expect(sidecarValidation).toBeGreaterThan(rasterOutputValidation);
  expect(frameCapture).toBeGreaterThan(rasterOutputValidation);
});

test("managed full acceptance rejects partial inventory or any score below 99", () => {
  const en: FullScoreAcceptance = {
    language: "en",
    fixtureCount: 59,
    minimumScore: 99,
    accepted: true,
  };
  const ja: FullScoreAcceptance = {
    language: "ja",
    fixtureCount: 58,
    minimumScore: 99,
    accepted: true,
  };
  expect(() => requireFullScoreAcceptance([en, ja])).not.toThrow();
  expect(() => requireFullScoreAcceptance([{ ...en, fixtureCount: 55 }, ja])).toThrow();
  expect(() => requireFullScoreAcceptance([en, { ...ja, minimumScore: 98.999 }])).toThrow();
  expect(() => requireFullScoreAcceptance([en, { ...ja, accepted: false }])).toThrow();
  expect(() => requireFullScoreAcceptance([ja, en])).toThrow();
});

test("managed full request accepts only official templates and official pins", () => {
  const request = validRequest();
  expect(() => parseManagedFullCompareRequest(request)).not.toThrow();
  const languageWithOldCandidateBinding = {
    ...request,
    languages: {
      ...request.languages,
      en: {
        ...request.languages.en,
        candidateBinding: { path: "/repo/old-candidate.json", sha256: "b".repeat(64) },
      },
    },
  };
  expect(() => parseManagedFullCompareRequest(languageWithOldCandidateBinding)).toThrow();
});

test("managed full input pins canonical fixture bytes and ignored output roots", () => {
  const repositoryRoot = path.resolve(process.cwd());
  for (const [language, count] of [
    ["en", 59],
    ["ja", 58],
  ] as const) {
    const inventory = captureCanonicalFixtureSources(repositoryRoot, language);
    expect(inventory.slugs).toHaveLength(count);
    expect(inventory.sources).toHaveLength(count + 1);
    expect(inventory.sources.filter((source) => source.empty)).toHaveLength(1);
    expect(
      inventory.sources.every(
        (source) =>
          /^[a-f0-9]{64}$/.test(source.markdownSha256) &&
          /^[a-f0-9]{64}$/.test(source.originalFenceSourceSha256) &&
          /^[a-f0-9]{64}$/.test(source.canonicalSourceSha256),
      ),
    ).toBe(true);
  }

  const tempParent = fs.mkdtempSync(path.join(repositoryRoot, "tmp", "managed-output-validation-"));
  try {
    expect(() =>
      assertManagedOutputRoot(path.join(tempParent, "run"), repositoryRoot),
    ).not.toThrow();
    expect(() =>
      assertManagedOutputRoot(path.join(repositoryRoot, "outside-tmp"), repositoryRoot),
    ).toThrow();
    expect(() =>
      assertManagedOutputRoot(path.join(repositoryRoot, "tmp"), repositoryRoot),
    ).toThrow();
  } finally {
    fs.rmSync(tempParent, { recursive: true, force: true });
  }
});

test("managed probe preflight follows Node X_OK semantics before expensive work", () => {
  const root = path.resolve(process.cwd());
  const parent = fs.mkdtempSync(path.join(root, "tmp", "managed-probe-executable-"));
  try {
    const request = executableProbeRequest(root, parent);
    if (process.platform === "win32") {
      expect(() => validatePinnedInputs(request)).not.toThrow();
    } else {
      expect(() => validatePinnedInputs(request)).toThrow(
        "native frame probe binary is not executable",
      );
    }
    for (const language of ["en", "ja"] as const) {
      const probe = request.languages[language].nativeFrameCapture?.probeBinary;
      if (probe === undefined) throw new Error(`${language} probe fixture is missing`);
      fs.chmodSync(probe.path, 0o755);
    }
    expect(() => validatePinnedInputs(request)).not.toThrow();
  } finally {
    fs.rmSync(parent, { recursive: true, force: true });
  }
});

test("managed full aggregation rejects missing, duplicate, weak, and nonformal score evidence", () => {
  const expected = slugs(59);
  const valid = validEvidence(expected);
  const invalidCases = [
    { scores: valid.scores.slice(1), inventory: valid.inventory, context: valid.context },
    {
      scores: [valid.scores[0], ...valid.scores.slice(1, -1), valid.scores[0]],
      inventory: valid.inventory,
      context: valid.context,
    },
    {
      scores: valid.scores.map((row, index) => (index === 0 ? { ...row, score: 98.999 } : row)),
      inventory: valid.inventory,
      context: valid.context,
    },
    {
      scores: valid.scores.map((row, index) =>
        index === 0 ? { ...row, contentRmseScore: 80 } : row,
      ),
      inventory: valid.inventory,
      context: valid.context,
    },
    {
      scores: valid.scores,
      inventory: { ...valid.inventory, skippedEmpty: [{ slug: "skipped", reason: "empty" }] },
      context: valid.context,
    },
    {
      scores: valid.scores,
      inventory: valid.inventory,
      context: { ...valid.context, formalAccepted: true },
    },
  ];
  for (const value of invalidCases) {
    expect(() =>
      validateLanguageScoreEvidence("en", expected, value.scores, value.inventory, value.context),
    ).toThrow();
  }
  const belowGate = valid.scores.map((row, index) =>
    index === 0
      ? {
          ...row,
          score: 98.999,
          rgbScore: 98.999,
          canvasRmseScore: 98.999,
          canvasMaeScore: 98.999,
          contentRmseScore: 98.999,
          contentMaeScore: 98.999,
          passed: false,
        }
      : row,
  );
  const miss = validateLanguageScoreEvidence(
    "en",
    expected,
    belowGate,
    valid.inventory,
    valid.context,
  );
  expect(miss.accepted).toBe(false);
  expect(miss.minimumScore).toBe(98.999);
  expect(miss.scores[0]?.passed).toBe(false);
});

test("managed full aggregation rejects incomplete inventories and lowered score thresholds", () => {
  const expected = slugs(59);
  const valid = validEvidence(expected);
  expect(() =>
    validateLanguageScoreEvidence(
      "en",
      expected.slice(0, -1),
      valid.scores,
      valid.inventory,
      valid.context,
    ),
  ).toThrow();
  expect(() =>
    validateLanguageScoreEvidence(
      "en",
      expected,
      valid.scores.map((row) => ({ ...row, minScore: 98 })),
      valid.inventory,
      valid.context,
    ),
  ).toThrow();
});

test("managed native semantics bind actual PNG edge support and projected metrics separately from RGB", () => {
  const expected = slugs(59);
  expected[0] = "08-04-c4-component";
  const evidence = validGeometryEvidence(expected);
  const accepted = validateNativeGeometryEvidence(expected, evidence);
  expect(accepted.accepted).toBe(true);
  expect(accepted.scores[0]).toMatchObject({
    score: 99,
    rgbScore: 80,
    scoreSemantics: "actual-png-rgb-channel-edge-support-v1+paint-projected-reference-v1",
    actualPngShape: {
      policy: "rgb-channel-edge-support-v1",
      score: 99,
      officialPngSha256: "a".repeat(64),
      actualPngSha256: "f".repeat(64),
    },
  });

  const oldProjectedOnlyScores = evidence.scores.map((row) => {
    if (row.actualPngShape === undefined) return row;
    const { actualPngShape: _actualPngShape, ...projectedOnly } = row;
    return { ...projectedOnly, scoreSemantics: "paint-projected-reference-v1" };
  });
  const legacyNativeContext = { ...evidence.context } as Record<string, unknown>;
  delete legacyNativeContext.comparisonRole;
  legacyNativeContext.scoreSemantics = "rgb-v1+paint-projected-reference-v1";

  const referenceContext = { ...evidence.context } as Record<string, unknown>;
  delete referenceContext.comparisonRole;
  referenceContext.scoreSemantics = "rgb-v1+paint-projected-reference-v1";
  const referenceScores = evidence.scores.map((row) => {
    if (row.paintProjectedReference === undefined) return row;
    const { actualPngShape: _actualPngShape, ...projected } = row;
    return {
      ...projected,
      score: row.paintProjectedReference.score,
      passed: row.paintProjectedReference.structuralIntegrityAccepted,
      scoreSemantics: "paint-projected-reference-v1",
    };
  });

  const outOfCanvasScores = evidence.scores.map((row, index) =>
    index === 0 && row.actualPngShape !== undefined
      ? {
          ...row,
          actualPngShape: { ...row.actualPngShape, roi: { x: 319, y: 219, width: 2, height: 2 } },
        }
      : row,
  );
  const wrongMetrics = {
    ...evidence.geometry,
    pairs: evidence.geometry.pairs.map((pair) => ({
      ...pair,
      projectedMetrics: { ...pair.projectedMetrics, contentMaeScore: 80, score: 80 },
    })),
  };
  const legacyNeutralEvidence = {
    ...evidence.geometry,
    schema: "krr-mermaid-neutral-geometry-evidence-v1",
    policy: "neutral-svg-geometry-v1",
  };
  const missingNativePins = new Map<string, string>(),
    mismatchedNativePins = nativePngPins(expected);
  mismatchedNativePins.set(expected[0] ?? "", "0".repeat(64));
  const rejectedMutations: Array<{
    name: string;
    scores?: unknown | undefined;
    context?: unknown | undefined;
    geometry?: unknown | undefined;
    pins?: ReadonlyMap<string, string>;
    message?: string;
  }> = [
    { name: "shape ROI outside canvas", scores: outOfCanvasScores },
    { name: "projected-only native row", scores: oldProjectedOnlyScores },
    {
      name: "unmarked legacy native context",
      scores: oldProjectedOnlyScores,
      context: legacyNativeContext,
      message: "comparison role does not match its trusted caller",
    },
    { name: "projected metric mismatch", geometry: wrongMetrics },
    { name: "legacy neutral geometry evidence", geometry: legacyNeutralEvidence },
    { name: "missing native PNG pin", pins: missingNativePins },
    { name: "mismatched native PNG pin", pins: mismatchedNativePins },
  ];
  for (const mutation of rejectedMutations) {
    const reject = () =>
      validateNativeGeometryEvidence(expected, evidence, {
        ...(mutation.scores === undefined ? {} : { scores: mutation.scores }),
        ...(mutation.context === undefined ? {} : { context: mutation.context }),
        ...(mutation.geometry === undefined ? {} : { geometry: mutation.geometry }),
        ...(mutation.pins === undefined ? {} : { pins: mutation.pins }),
      });
    if (mutation.message === undefined) expect(reject, mutation.name).toThrow();
    else expect(reject, mutation.name).toThrow(mutation.message);
  }

  expect(
    validateLanguageScoreEvidence(
      "en",
      expected,
      referenceScores,
      evidence.inventory,
      referenceContext,
      evidence.geometry,
    ).accepted,
  ).toBe(true);

  const belowProjectedThreshold = {
    ...evidence.geometry,
    pairs: evidence.geometry.pairs.map((pair) => ({
      ...pair,
      projectedMetrics: { ...pair.projectedMetrics, contentMaeScore: 98.5, score: 98.5 },
    })),
  };
  const belowProjectedScores = evidence.scores.map((row, index) => {
    if (index !== 0) return row;
    if (
      row.scoreSemantics !== "actual-png-rgb-channel-edge-support-v1+paint-projected-reference-v1"
    ) {
      throw new Error("Actual PNG edge support or projected reference semantics are missing");
    }
    const projected = row.paintProjectedReference;
    if (projected === undefined) throw new Error("Projected reference score is missing");
    return {
      ...row,
      score: 98.5,
      passed: false,
      paintProjectedReference: { ...projected, contentMaeScore: 98.5, score: 98.5 },
      actualPngShape: {
        ...row.actualPngShape,
        score: 99,
        fullCanvasScore: 99,
        foregroundUnionScore: 99,
      },
    };
  });
  const rejected = validateNativeGeometryEvidence(expected, evidence, {
    scores: belowProjectedScores,
    geometry: belowProjectedThreshold,
  });
  expect(rejected.accepted).toBe(false);
  expect(rejected.minimumScore).toBe(98.5);
});

test("managed geometry evidence keeps structural mismatches as failed scores", () => {
  const expected = slugs(59);
  expected[0] = "08-04-c4-component";
  const evidence = validGeometryEvidence(expected, false);
  const result = validateLanguageScoreEvidence(
    "en",
    expected,
    evidence.scores,
    evidence.inventory,
    evidence.context,
    evidence.geometry,
    { role: "native-product", pngSha256BySlug: nativePngPins(expected) },
  );
  expect(result.accepted).toBe(false);
  expect(result.minimumScore).toBe(99);
  expect(result.scores[0]?.passed).toBe(false);
});

test("managed Python route pins the resolved tool and preserves raw logs", () => {
  const root = fs.mkdtempSync(path.join(process.cwd(), "tmp", "managed-python-route-"));
  const outputRoot = path.join(root, "run");
  fs.mkdirSync(outputRoot, { mode: 0o700 });
  try {
    const resolvedPython = Bun.which("python3");
    if (resolvedPython === null) throw new Error("test requires the standard python3 tool");
    const pythonPath = fs.realpathSync(resolvedPython);
    const tool = { kind: "python3" as const, path: pythonPath, sha256: fileSha256(pythonPath) };
    const child = runManagedCommand(
      outputRoot,
      "python-usage",
      [pythonPath, "scripts/mermaid/produce_raster_process_evidence.py"],
      process.cwd(),
      tool,
    );
    expect(child.exitCode).toBe(2);
    expect(fs.readFileSync(child.stderrPath, "utf8")).toContain("usage:");
    expect(child.argv[0]).toBe("python3");
    expect(() => parseChildJson(child, "Python usage child")).toThrow();
    const command = JSON.parse(
      fs.readFileSync(path.join(outputRoot, "logs", "python-usage.command.json"), "utf8"),
    ) as { executable: string; resolvedExecutablePath: string; resolvedExecutableSha256: string };
    expect(command).toMatchObject({
      argv: ["python3", "scripts/mermaid/produce_raster_process_evidence.py"],
      executable: "python3",
      resolvedExecutablePath: pythonPath,
      resolvedExecutableSha256: tool.sha256,
    });
    expectManagedToolRejections(outputRoot, root, tool);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("managed Bun route uses the pinned literal script invocation", () => {
  const root = fs.mkdtempSync(path.join(process.cwd(), "tmp", "managed-bun-route-"));
  const outputRoot = path.join(root, "run");
  fs.mkdirSync(outputRoot, { mode: 0o700 });
  try {
    const resolvedBun = Bun.which("bun");
    if (resolvedBun === null) throw new Error("test requires the standard bun tool");
    const bunPath = fs.realpathSync(resolvedBun);
    const tool = { kind: "bun" as const, path: bunPath, sha256: fileSha256(bunPath) };
    const child = runManagedCommand(
      outputRoot,
      "bun-help",
      [bunPath, "run", "scripts/mermaid/raster-production-session.ts", "--help"],
      process.cwd(),
      tool,
    );
    expect(child.exitCode).toBe(0);
    expect(fs.readFileSync(child.stdoutPath, "utf8")).toContain("--seed-official");
    const command = JSON.parse(
      fs.readFileSync(path.join(outputRoot, "logs", "bun-help.command.json"), "utf8"),
    ) as { argv: string[]; executable: string; resolvedExecutableSha256: string };
    expect(command).toMatchObject({
      argv: ["bun", "run", "scripts/mermaid/raster-production-session.ts", "--help"],
      executable: "bun",
      resolvedExecutableSha256: tool.sha256,
    });
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("score process exit must match complete score evidence", () => {
  const failed: ManagedCommandResult = {
    label: "score",
    argv: ["bun", "run", "scripts/mermaid/reference-compare.ts"],
    cwd: process.cwd(),
    stdoutPath: "stdout",
    stderrPath: "stderr",
    exitPath: "exit",
    exitCode: 1,
    signal: null,
    spawnError: null,
  };
  expect(() => requireScoreProcessOutcome(failed, false)).not.toThrow();
  expect(() => requireScoreProcessOutcome(failed, true)).toThrow();
  expect(() => requireScoreProcessOutcome({ ...failed, exitCode: 2 }, false)).toThrow();
});

function expectManagedToolRejections(
  outputRoot: string,
  cwd: string,
  tool: { kind: "bun" | "python3"; path: string; sha256: string },
): void {
  const args = [tool.path, "scripts/mermaid/produce_raster_process_evidence.py"];
  expect(() =>
    runManagedCommand(outputRoot, "wrong-hash", args, cwd, { ...tool, sha256: "0".repeat(64) }),
  ).toThrow();
  expect(() =>
    runManagedCommand(outputRoot, "wrong-role", args, cwd, { ...tool, kind: "bun" }),
  ).toThrow();
  expect(() =>
    runManagedCommand(outputRoot, "unknown-script", [tool.path, "scripts/unknown.py"], cwd, tool),
  ).toThrow();
  expect(() =>
    runManagedCommand(outputRoot, "wrong-executable", ["/bin/false", ...args.slice(1)], cwd, tool),
  ).toThrow();
}

function validateNativeGeometryEvidence(
  expected: string[],
  evidence: ReturnType<typeof validGeometryEvidence>,
  overrides: {
    scores?: unknown;
    context?: unknown;
    geometry?: unknown;
    pins?: ReadonlyMap<string, string>;
  } = {},
): ReturnType<typeof validateLanguageScoreEvidence> {
  return validateLanguageScoreEvidence(
    "en",
    expected,
    overrides.scores ?? evidence.scores,
    evidence.inventory,
    overrides.context ?? evidence.context,
    overrides.geometry ?? evidence.geometry,
    { role: "native-product", pngSha256BySlug: overrides.pins ?? nativePngPins(expected) },
  );
}

function slugs(count: number): string[] {
  const values = Array.from(
    { length: count },
    (_, index) => `fixture-${String(index + 1).padStart(2, "0")}`,
  );
  values[0] = "20-02-sankey-beta";
  return values;
}

function nativePngPins(expected: string[]): Map<string, string> {
  return new Map(expected.map((slug) => [slug, "f".repeat(64)]));
}

function validEvidence(expected: string[]) {
  const policy = new ReferenceScorePolicy(99);
  return {
    scores: expected.map((slug) => {
      const threshold = policy.thresholdFor(slug);
      return {
        slug,
        score: Math.max(99, threshold.minScore),
        rgbScore: Math.max(99, threshold.minScore),
        scoreSemantics: "rgb-v1" as const,
        canvasRmseScore: Math.max(99, threshold.minScore),
        canvasMaeScore: Math.max(99, threshold.minScore),
        contentRmseScore: Math.max(99, threshold.minScore),
        contentMaeScore: Math.max(99, threshold.minScore),
        minScore: threshold.minScore,
        exceptionReason: threshold.reason,
        passed: true,
      };
    }),
    inventory: {
      contract: "explicit-fixture-directory-and-glob",
      expected,
      scored: expected,
      skippedEmpty: [],
    },
    context: {
      mode: "strictVendorOriginal",
      scope: "formal",
      expectedSlugCount: expected.length,
      emptySlugCount: 0,
      scoreThreshold: 99,
      formalAccepted: false,
      independentGold: false,
      scoreSemantics: "rgb-v1",
    },
  };
}

function validGeometryEvidence(expected: string[], accepted = true) {
  const base = validEvidence(expected),
    slug = expected[0] ?? "08-04-c4-component",
    evidenceSha256 = "b".repeat(64),
    score = 99,
    roi = { x: 10, y: 10, width: 300, height: 200 };
  const scores: ReferenceScoreRow[] = base.scores.map((row, index) =>
    index === 0
      ? {
          ...row,
          score,
          rgbScore: 80,
          scoreSemantics:
            "actual-png-rgb-channel-edge-support-v1+paint-projected-reference-v1" as const,
          canvasRmseScore: 80,
          canvasMaeScore: 80,
          contentRmseScore: 80,
          contentMaeScore: 80,
          passed: accepted,
          actualPngShape: {
            policy: "rgb-channel-edge-support-v1" as const,
            score,
            fullCanvasScore: 99,
            foregroundUnionScore: 99,
            officialPngSha256: "a".repeat(64),
            actualPngSha256: "f".repeat(64),
            width: 320,
            height: 220,
            roi,
          },
          paintProjectedReference: {
            policy: "raw-svg-geometry-candidate-computed-paint-v1" as const,
            score,
            structuralIntegrityAccepted: accepted,
            reasons: accepted ? [] : ["$svg:transform"],
            maxTransformDelta: accepted ? 0 : 0.00002,
            fullCanvasRmseScore: 99,
            fullCanvasMaeScore: 99,
            contentRmseScore: 99,
            contentMaeScore: 99,
            width: 320,
            height: 220,
            evidenceSha256,
            roi,
          },
        }
      : row,
  );
  const record = (role: "official" | "candidate") => ({
    svgPath: `/repo/${role}/${slug}.svg`,
    svgSha256: "a".repeat(64),
    sourcePngPath: `/repo/${role}/${slug}.png`,
    sourcePngSha256: "a".repeat(64),
  });
  const projectedReference = {
    svgPath: `/repo/paint-projected-reference/${slug}-reference.svg`,
    svgSha256: "b".repeat(64),
    pngPath: `/repo/paint-projected-reference/${slug}-reference.png`,
    pngSha256: "c".repeat(64),
    width: 320,
    height: 220,
    graphicsElementCount: 8,
    paintVisibilitySha256: "d".repeat(64),
  };
  const projectedMetrics = {
    fullCanvasRmseScore: 99,
    fullCanvasMaeScore: 99,
    contentRmseScore: 99,
    contentMaeScore: 99,
    score: 99,
    roi,
    width: 320,
    height: 220,
  };
  return {
    scores,
    inventory: base.inventory,
    context: {
      ...base.context,
      comparisonRole: "native-product",
      scoreSemantics: "actual-png-rgb-channel-edge-support-v1+paint-projected-reference-v1",
      paintProjectedReferenceEvidence: {
        path: "paint-projected-reference-evidence.json",
        sha256: evidenceSha256,
      },
    },
    geometry: {
      schema: "krr-mermaid-paint-projected-reference-evidence-v1",
      policy: "raw-svg-geometry-candidate-computed-paint-v1",
      structuralPolicy: "ordered-svg-tree-and-nonpaint-plus-computed-paint-visibility-v2",
      renderer: "pinned-playwright-chromium-svg-capture-v1",
      host: { platform: "darwin", arch: "arm64" },
      browser: {
        version: "Chromium 156",
        executablePath: "/repo/chromium",
        executableSha256: "a".repeat(64),
        viewport: { width: 1520, height: 845, deviceScaleFactor: 1 },
      },
      pairs: [
        {
          slug,
          structuralIntegrity: {
            accepted,
            reasons: accepted ? [] : ["$svg:transform"],
            maxTransformDelta: accepted ? 0 : 0.00002,
            signatureSha256: "c".repeat(64),
          },
          official: record("official"),
          candidate: record("candidate"),
          projectedReference,
          projectedMetrics,
        },
      ],
    },
  };
}

function validRequest() {
  const pin = (name: string) => ({ path: `/repo/${name}`, sha256: "a".repeat(64) });
  const language = (code: string) => ({
    fixtures: `/repo/fixtures/${code}`,
    officialTemplate: pin(`official-${code}.json`),
    officialReceipt: pin(`official-${code}-receipt.json`),
    officialProcessEvidence: pin(`official-${code}-process.json`),
    officialPairDirectory: `/repo/pairs/${code}`,
  });
  return {
    schema: "krr-mermaid-managed-full-run-request-v1",
    repositoryRoot: "/repo",
    outputRoot: "/repo/tmp/managed-run",
    sourceManifest: pin("source.json"),
    protectedRegistry: pin("protected.json"),
    binary: "/repo/target/debug/krr",
    runtime: pin("mermaid.js"),
    tools: {
      cargo: pin("cargo"),
      rustc: pin("rustc"),
      python: pin("python"),
      bun: pin("bun"),
      mermaidJs: pin("mermaid-vendor.js"),
      mermaidZenUmlJs: pin("zenuml-vendor.js"),
    },
    languages: { en: language("en"), ja: language("ja") },
  };
}

function executableProbeRequest(repositoryRoot: string, parent: string) {
  const pin = (name: string) => pinnedFile(parent, name),
    language = (code: "en" | "ja") => executableProbeLanguage(repositoryRoot, parent, code);
  return parseManagedFullCompareRequest({
    schema: "krr-mermaid-managed-full-run-request-v1",
    repositoryRoot,
    outputRoot: path.join(parent, "run"),
    sourceManifest: pin("source.json"),
    protectedRegistry: pin("protected.json"),
    binary: path.join(repositoryRoot, "target/debug/krr"),
    runtime: pin("runtime.js"),
    tools: {
      cargo: pin("tools/cargo"),
      rustc: pin("tools/rustc"),
      python: pin("tools/python"),
      bun: pin("tools/bun"),
      mermaidJs: pin("tools/mermaid.js"),
      mermaidZenUmlJs: pin("tools/zenuml.js"),
    },
    languages: { en: language("en"), ja: language("ja") },
  });
}

function executableProbeLanguage(repositoryRoot: string, parent: string, code: "en" | "ja") {
  const pairDirectory = path.join(parent, `${code}-pairs`);
  fs.mkdirSync(pairDirectory);
  return {
    fixtures: path.join(repositoryRoot, "tests/fixtures/mermaid", code),
    officialTemplate: pinnedFile(parent, `${code}/official-template.json`),
    officialReceipt: pinnedFile(parent, `${code}/official-receipt.json`),
    officialProcessEvidence: pinnedFile(parent, `${code}/official-process.json`),
    officialPairDirectory: pairDirectory,
    nativeFrameCapture: {
      probeBinary: pinnedFile(parent, `${code}/probe`, 0o644),
      config: pinnedFile(parent, `${code}/config.json`),
      fontContext: pinnedFile(parent, `${code}/font-context.json`),
      buildProof: {
        bundleManifest: pinnedFile(parent, `${code}/bundle/bundle.json`),
        artifactPath: "candidate/bin",
        artifactSha256: "b".repeat(64),
      },
    },
  };
}

function pinnedFile(parent: string, name: string, mode = 0o600) {
  const filePath = path.join(parent, name);
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, name, { flag: "wx", mode });
  fs.chmodSync(filePath, mode);
  return { path: filePath, sha256: fileSha256(filePath) };
}
