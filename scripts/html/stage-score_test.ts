import { describe, expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import type { JsonValue } from "./dynamic-state";
import { sha256File } from "./reference-capture";
import type { AnonymousDocumentExpectations } from "./stage-score";
import {
  assertAnonymousFidelityResults,
  assertStage90Manifest,
  assertStrictStage90Manifest,
  collectAnonymousFidelityFailures,
  loadCanonicalTrustedExpectations,
} from "./stage-score";

const source = "a".repeat(64);
const binary = "b".repeat(64);
const reference = "c".repeat(64);
const viewport = { width: 1280, height: 100, scale: 1 };
const expected = {
  profile: "stage90",
  threshold: 90,
  sourceSha256: source,
  binarySha256: binary,
  images: [
    {
      key: "case/1280x100@1/initial/scroll/0",
      caseId: "case",
      viewport,
      state: "initial",
      kind: "scroll",
      actualScrollY: 0,
      maxScrollY: 100,
      viewportHeight: 100,
      sourceSha256: source,
      referenceSha256: reference,
    },
    {
      key: "case/1280x100@1/initial/scroll/bottom",
      caseId: "case",
      viewport,
      state: "initial",
      kind: "scroll",
      actualScrollY: 100,
      maxScrollY: 100,
      viewportHeight: 100,
      sourceSha256: source,
      referenceSha256: reference,
    },
  ],
  tocKeys: [],
  performance: {
    environment: "ci",
    conditionId: "fixture",
    phases: [
      {
        name: "cold",
        baselineMs: 100,
        baselineMemoryBytes: 1000,
        observedMs: 100,
        observedMemoryBytes: 1000,
        measurementPairs: 1,
      },
      {
        name: "warm",
        baselineMs: 100,
        baselineMemoryBytes: 1000,
        observedMs: 100,
        observedMemoryBytes: 1000,
        measurementPairs: 1,
      },
      {
        name: "re-display",
        baselineMs: 100,
        baselineMemoryBytes: 1000,
        observedMs: 100,
        observedMemoryBytes: 1000,
        measurementPairs: 1,
      },
      {
        name: "full-Y",
        baselineMs: 100,
        baselineMemoryBytes: 1000,
        observedMs: 100,
        observedMemoryBytes: 1000,
        measurementPairs: 1,
      },
      {
        name: "post-operation",
        baselineMs: 100,
        baselineMemoryBytes: 1000,
        observedMs: 100,
        observedMemoryBytes: 1000,
        measurementPairs: 1,
      },
    ],
  },
};

function stage() {
  return {
    profile: "stage90",
    threshold: 90,
    sourceSha256: source,
    binarySha256: binary,
    images: expected.images.map((image) => ({
      ...image,
      rmseScore: 92,
      maeScore: 91,
      binarySha256: binary,
      targetVisible: true,
      operationState: "matched",
      evidenceId: image.key,
    })),
    performance: {
      environment: "ci",
      conditionId: "fixture",
      phases: expected.performance.phases.map((phase) => ({
        ...phase,
        observedMs: 100,
        observedMemoryBytes: 1000,
        measurementPairs: 1,
      })),
    },
  };
}

describe("stage score contract", () => {
  test("rejects reported improvements that differ from the measured candidate", () => {
    for (const change of [
      { observedMs: 99 },
      { observedMemoryBytes: 999 },
      { measurementPairs: 2 },
    ]) {
      const value = stage();
      value.performance.phases = value.performance.phases.map((phase, index) =>
        index === 0 ? { ...phase, ...change } : phase,
      );
      expect(() => assertStage90Manifest(value, expected)).toThrow("performance");
    }
  });
  test("accepts exact trusted coverage and all required performance phases", () => {
    expect(() => assertStage90Manifest(stage(), expected)).not.toThrow();
  });
  test("rejects threshold, actual-Y, reference, and negative performance mutations", () => {
    const value = stage();
    expect(() => assertStage90Manifest({ ...value, threshold: 99 }, expected)).toThrow("profile");
    expect(() =>
      assertStage90Manifest(
        { ...value, images: value.images.map((image) => ({ ...image, actualScrollY: 50 })) },
        expected,
      ),
    ).toThrow("contract");
    expect(() =>
      assertStage90Manifest(
        {
          ...value,
          images: value.images.map((image) => ({ ...image, referenceSha256: "d".repeat(64) })),
        },
        expected,
      ),
    ).toThrow("provenance");
    expect(() =>
      assertStage90Manifest(
        {
          ...value,
          performance: { ...value.performance, phases: value.performance.phases.slice(0, 4) },
        },
        expected,
      ),
    ).toThrow("phases");
  });

  test("loader binds a registered canonical fixture under the catalog fixture root", () => {
    const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "stage90-registered-source-"));
    const reportPath = path.join(temporary, "scroll.json");
    const sourcePath = path.resolve("tests/fixtures/html/aspects/typography.html");
    const capturePath = path.join(temporary, "scroll.png");
    fs.writeFileSync(capturePath, "fixture capture");
    fs.writeFileSync(
      reportPath,
      JSON.stringify({
        htmlPath: sourcePath,
        htmlSha256: sha256File(sourcePath),
        viewport: { width: 1280, height: 900, scale: 1 },
        maxScrollY: 1,
        scrolls: [{ name: "0", actual: 0, capturePath }],
      }),
    );
    const performancePath = path.join(temporary, "performance.json");
    fs.writeFileSync(performancePath, JSON.stringify({ performance: expected.performance }));
    try {
      const trusted = loadCanonicalTrustedExpectations(
        "tests/fixtures/html/feature-catalog.json",
        [reportPath],
        [],
        binary,
        performancePath,
      );
      expect(trusted.images.map((image) => image.caseId)).toEqual(["typography"]);
    } finally {
      fs.rmSync(temporary, { recursive: true, force: true });
    }
  });

  test("loader rejects a canonical manifest whose fixture is absent from the catalog", () => {
    const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "stage90-loader-"));
    const reportPath = path.join(temporary, "scroll.json");
    const sourcePath = path.resolve("tests/fixtures/html/anonymized-requirements.html");
    const report = {
      htmlPath: sourcePath,
      htmlSha256: sha256File(sourcePath),
      viewport: { width: 1280, height: 900, scale: 1 },
      maxScrollY: 1,
      scrolls: [
        {
          name: "0",
          actual: 0,
          capturePath: "tmp/html-scroll-coverage/full-png-current-1280/scroll/000.png",
        },
      ],
    };
    fs.writeFileSync(reportPath, JSON.stringify(report));
    const performancePath = path.join(temporary, "performance.json");
    fs.writeFileSync(performancePath, JSON.stringify({ performance: {} }));
    try {
      expect(() =>
        loadCanonicalTrustedExpectations(
          "tests/fixtures/html/feature-catalog.json",
          [reportPath],
          [],
          binary,
          performancePath,
        ),
      ).toThrow("absent from catalog");
    } finally {
      fs.rmSync(temporary, { recursive: true, force: true });
    }
  });

  test("loader rejects a canonical source file changed after its manifest was captured", () => {
    const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "stage90-source-change-"));
    const reportPath = path.join(temporary, "scroll.json");
    const sourcePath = path.join(temporary, "source.html");
    fs.writeFileSync(sourcePath, "<html>captured</html>");
    const capturedSha256 = sha256File(sourcePath);
    fs.writeFileSync(sourcePath, "<html>changed</html>");
    fs.writeFileSync(
      reportPath,
      JSON.stringify({
        htmlPath: sourcePath,
        htmlSha256: capturedSha256,
        viewport: { width: 1280, height: 900, scale: 1 },
        maxScrollY: 1,
        scrolls: [{ name: "0", actual: 0, capturePath: "scroll/000.png" }],
      }),
    );
    const performancePath = path.join(temporary, "performance.json");
    fs.writeFileSync(performancePath, JSON.stringify({ performance: {} }));
    try {
      expect(() =>
        loadCanonicalTrustedExpectations(
          "tests/fixtures/html/feature-catalog.json",
          [reportPath],
          [],
          binary,
          performancePath,
        ),
      ).toThrow("canonical source file changed");
    } finally {
      fs.rmSync(temporary, { recursive: true, force: true });
    }
  });

  test("collects every score and scroll failure without stopping at the first image", () => {
    const expectations: AnonymousDocumentExpectations = {
      profile: "stage90-anonymous-document",
      sourceSha256: source,
      binarySha256: binary,
      images: expected.images.map((image) => ({ ...image, kind: "scroll" as const })),
      viewports: [{ viewport, maxScrollY: 100, scrollKeys: ["0", "100"], tocKeys: [] }],
      performance: {
        environment: "ci",
        conditionId: "fixture",
        phases: expected.performance.phases,
      },
    };
    const results = {
      htmlSha256: source,
      binarySha256: binary,
      results: [
        {
          viewport,
          requested: 0,
          chromeScrollY: 0,
          krrScrollY: 1,
          referenceSha256: reference,
          score: { score: 89 },
        },
        {
          viewport,
          requested: 1e9,
          chromeScrollY: 100,
          krrScrollY: 99,
          referenceSha256: reference,
          score: { score: 89 },
        },
      ],
    };
    const failures = collectAnonymousFidelityFailures(results as JsonValue, expectations);
    expect(failures.filter((failure) => failure.kind === "score")).toHaveLength(2);
    expect(failures.filter((failure) => failure.kind === "scroll")).toHaveLength(2);
    expect(() => assertAnonymousFidelityResults(results as JsonValue, expectations)).toThrow(
      /scroll mismatch.*scroll mismatch/s,
    );
  });

  test("strict binder recomputes pixels and requires operation evidence", () => {
    const temporary = fs.mkdtempSync(path.resolve("tmp/stage90-strict-"));
    const png = path.join(temporary, "reference-1280x100.png");
    const pngResult = Bun.spawnSync(["magick", "-size", "1280x100", "xc:white", png]);
    expect(pngResult.exitCode).toBe(0);
    const evidencePath = path.join(temporary, "operation.json");
    const sourceSha = source;
    const evidence = {
      status: "matched",
      sourceSha256: sourceSha,
      binarySha256: binary,
      caseId: "case",
      state: "initial",
      targetId: "target",
      actualScrollY: 0,
      maxScrollY: 0,
      targetVisible: true,
      nonce: "nonce-1",
      candidateSha256: sha256File(png),
      key: "case/1280x100@1/initial/scroll/0",
      kind: "scroll",
      viewport: { width: 1280, height: 100, scale: 1 },
      requestedY: 0,
      observedHash: "#target",
    };
    fs.writeFileSync(evidencePath, JSON.stringify(evidence));
    const image = {
      key: "case/1280x100@1/initial/scroll/0",
      caseId: "case",
      state: "initial",
      kind: "scroll",
      actualScrollY: 0,
      maxScrollY: 0,
      requestedY: 0,
      viewportHeight: 100,
      viewport: { width: 1280, height: 100, scale: 1 },
      sourceSha256: sourceSha,
      binarySha256: binary,
      targetId: "target",
      targetVisible: true,
      nonce: "nonce-1",
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
      evidenceId: "evidence-1",
    };
    const manifest = {
      profile: "stage90",
      threshold: 90,
      sourceSha256: sourceSha,
      binarySha256: binary,
      images: [image],
      performance: strictPerformance(),
    };
    const trusted = {
      profile: "stage90",
      threshold: 90,
      sourceSha256: sourceSha,
      binarySha256: binary,
      images: [image],
      tocKeys: [],
      performance: strictPerformance(),
    };
    try {
      expect(() =>
        assertStrictStage90Manifest(manifest as JsonValue, trusted as JsonValue),
      ).not.toThrow();
      const tinyPng = path.join(temporary, "reference-1x1.png");
      const tinyResult = Bun.spawnSync(["magick", "-size", "1x1", "xc:white", tinyPng]);
      expect(tinyResult.exitCode).toBe(0);
      const tinyImage = {
        ...image,
        referencePath: tinyPng,
        referenceSha256: sha256File(tinyPng),
        candidatePath: tinyPng,
        candidateSha256: sha256File(tinyPng),
      };
      expect(() =>
        assertStrictStage90Manifest(
          { ...manifest, images: [tinyImage] } as JsonValue,
          { ...trusted, images: [tinyImage] } as JsonValue,
        ),
      ).toThrow("dimensions differ from viewport");
      const state = { ready: true, text: "done", items: ["a", "b"] };
      const stateEvidence = {
        ...evidence,
        kind: "state",
        observedState: state,
      };
      fs.writeFileSync(evidencePath, JSON.stringify(stateEvidence));
      const stateImage = {
        ...image,
        kind: "state",
        expectedState: state,
        operationEvidenceSha256: sha256File(evidencePath),
      };
      expect(() =>
        assertStrictStage90Manifest(
          { ...manifest, images: [stateImage] } as JsonValue,
          { ...trusted, images: [stateImage] } as JsonValue,
        ),
      ).not.toThrow();
      fs.writeFileSync(
        evidencePath,
        JSON.stringify({ ...stateEvidence, observedState: { ...state, ready: false } }),
      );
      expect(() =>
        assertStrictStage90Manifest(
          {
            ...manifest,
            images: [{ ...stateImage, operationEvidenceSha256: sha256File(evidencePath) }],
          } as JsonValue,
          { ...trusted, images: [stateImage] } as JsonValue,
        ),
      ).toThrow("dynamic state mismatch");
      expect(() =>
        assertStrictStage90Manifest(
          { ...manifest, images: [] } as JsonValue,
          { ...trusted, images: [] } as JsonValue,
        ),
      ).toThrow("coverage");
      expect(() =>
        assertStrictStage90Manifest(
          JSON.parse(JSON.stringify({ ...manifest, performance: undefined })) as JsonValue,
          JSON.parse(JSON.stringify({ ...trusted, performance: undefined })) as JsonValue,
        ),
      ).toThrow("profile");
      expect(() =>
        assertStrictStage90Manifest(
          {
            ...manifest,
            images: [{ ...image, viewport: { width: 960, height: 100, scale: 1 } }],
          } as JsonValue,
          trusted as JsonValue,
        ),
      ).toThrow("contract");
      const swappedPath = path.resolve(
        "tests/fixtures/drawio/representative/official/basic-11-japanese-labels.png",
      );
      expect(() =>
        assertStrictStage90Manifest(
          {
            ...manifest,
            images: [
              { ...image, candidatePath: swappedPath, candidateSha256: sha256File(swappedPath) },
            ],
          } as JsonValue,
          trusted as JsonValue,
        ),
      ).toThrow("trusted image");
      const linkedDirectory = path.join(temporary, "linked");
      fs.symlinkSync(path.dirname(png), linkedDirectory);
      expect(() =>
        assertStrictStage90Manifest(
          {
            ...manifest,
            images: [{ ...image, candidatePath: path.join(linkedDirectory, path.basename(png)) }],
          } as JsonValue,
          trusted as JsonValue,
        ),
      ).toThrow("symlinked");
      fs.writeFileSync(
        evidencePath,
        JSON.stringify({ ...evidence, viewport: { width: 960, height: 100, scale: 1 } }),
      );
      expect(() =>
        assertStrictStage90Manifest(
          {
            ...manifest,
            images: [{ ...image, operationEvidenceSha256: sha256File(evidencePath) }],
          } as JsonValue,
          trusted as JsonValue,
        ),
      ).toThrow("identity");
      fs.writeFileSync(evidencePath, JSON.stringify({ ...evidence, observedHash: "#other" }));
      expect(() =>
        assertStrictStage90Manifest(
          {
            ...manifest,
            images: [{ ...image, operationEvidenceSha256: sha256File(evidencePath) }],
          } as JsonValue,
          trusted as JsonValue,
        ),
      ).toThrow("identity");
      fs.writeFileSync(evidencePath, JSON.stringify({ ...evidence, targetVisible: false }));
      expect(() =>
        assertStrictStage90Manifest(manifest as JsonValue, trusted as JsonValue),
      ).toThrow("operation evidence changed");
    } finally {
      fs.rmSync(temporary, { recursive: true, force: true });
    }
  });
});

function strictPerformance() {
  return {
    environment: "test",
    conditionId: "strict",
    phases: ["cold", "warm", "re-display", "full-Y", "post-operation"].map((name) => ({
      name,
      observedMs: 1,
      baselineMs: 1,
      observedMemoryBytes: 1,
      baselineMemoryBytes: 1,
      measurementPairs: 1,
    })),
  };
}
