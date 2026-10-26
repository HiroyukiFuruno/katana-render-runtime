import { describe, expect, test } from "bun:test";
import { execFileSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  realpathSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import { gunzipSync } from "node:zlib";
import {
  assertNoUnapprovedClones,
  assertToolSucceeded,
  readCurrentReport,
  resolveCliArguments,
  runAndValidateInventoryProbe,
  runJscpd,
  sha256,
  storeJscpdReport,
  validateInventoryReport,
  validateReport,
  verifiedThirdPartyJavascript,
} from "./duplication-check";
import { policySchema, reportSchema } from "./duplication-schema";
import { collectOwnedSourcePaths } from "./owned-source-inventory";

function withDuplicationCliFixture(
  run: (fixture: { repo: string; protectedFiles: string[]; output: string }) => void,
): void {
  const fixture = realpathSync(mkdtempSync(path.join(os.tmpdir(), "krr-dup-cli-")));
  const quality = path.join(fixture, "scripts/quality");
  const output = path.join(fixture, "tmp/quality/duplication");
  const resources = path.join(fixture, resourcesRoot);
  const nodeModules = path.resolve(import.meta.dir, "../../node_modules");
  mkdirSync(quality, { recursive: true });
  mkdirSync(output, { recursive: true });
  mkdirSync(resources, { recursive: true });
  for (const source of [
    "duplication-check.ts",
    "duplication-schema.ts",
    "owned-source-inventory.ts",
  ]) {
    copyFileSync(path.join(import.meta.dir, source), path.join(quality, source));
    expect(sha256(readFileSync(path.join(quality, source)))).toBe(
      sha256(readFileSync(path.join(import.meta.dir, source))),
    );
  }
  symlinkSync(nodeModules, path.join(fixture, "node_modules"), "dir");
  const policy = path.join(quality, "duplication-policy.json");
  const manifest = path.join(resources, "drawio-resource-manifest.json");
  const reportPath = path.join(output, "report.json");
  writeFileSync(policy, JSON.stringify(sourceState().policy));
  writeFileSync(manifest, JSON.stringify({ version: 1, resourcesRoot, resources: [] }));
  writeFileSync(reportPath, JSON.stringify(report()));
  const protectedFiles = [policy, manifest, reportPath];
  try {
    run({ repo: fixture, protectedFiles, output });
  } finally {
    rmSync(fixture, { recursive: true, force: true });
  }
}

function protectedFileSnapshot(files: string[], output: string): string[] {
  const inputHashes = files.map((file) => `${path.basename(file)}:${sha256(readFileSync(file))}`);
  const reports: string[] = [];
  function visit(directory: string): void {
    const entries = readdirSync(directory, { withFileTypes: true }).sort((left, right) =>
      left.name.localeCompare(right.name),
    );
    for (const entry of entries) {
      const absolute = path.join(directory, entry.name);
      const relative = path.relative(output, absolute);
      if (entry.isDirectory()) {
        reports.push(`directory:${relative}`);
        visit(absolute);
      } else reports.push(`${relative}:${sha256(readFileSync(absolute))}`);
    }
  }
  visit(output);
  return [...inputHashes, ...reports];
}

function runDuplicationCli(repo: string, args: string[]): ReturnType<typeof Bun.spawnSync> {
  const script = path.join(repo, "scripts/quality/duplication-check.ts");
  return Bun.spawnSync([process.execPath, script, ...args], { cwd: repo });
}

const repo = "/repo";
const firstPath = `${repo}/crates/first.rs`;
const secondPath = `${repo}/scripts/second.py`;
const firstText = "first source";
const secondText = "second source";
const fragment = "the exact duplicated fragment";
const resourcesRoot =
  "crates/katana-render-runtime/src/markdown/drawio_renderer/js_runtime/resources";
const manifestRelativePath = `${resourcesRoot}/drawio-resource-manifest.json`;
const assetBytes = "third-party upstream shape code";

function report(clones = 0, firstName = firstPath) {
  return {
    duplicates:
      clones === 0
        ? []
        : [
            {
              firstFile: {
                name: firstName,
                start: 1,
                end: 9,
                startLoc: { line: 1, column: 1 },
                endLoc: { line: 9, column: 4 },
              },
              secondFile: {
                name: secondPath,
                start: 3,
                end: 11,
                startLoc: { line: 3, column: 1 },
                endLoc: { line: 11, column: 4 },
              },
              fragment,
              format: "rust",
              lines: 9,
              tokens: 80,
            },
          ],
    statistics: {
      total: { sources: 2, clones },
      formats: {
        rust: { sources: 1, clones },
        typescript: { sources: 0, clones: 0 },
        javascript: { sources: 0, clones: 0 },
        python: { sources: 1, clones: 0 },
      },
    },
  };
}

function sourceState() {
  const paths = [firstPath, secondPath];
  const hashes = new Map([
    [firstPath, sha256(firstText)],
    [secondPath, sha256(secondText)],
  ]);
  const policy: Parameters<typeof validateReport>[5] = {
    version: 1,
    engine: "jscpd@5.4.1",
    minLines: 8,
    minTokens: 75,
    mode: "mild",
    formats: ["rust", "typescript", "javascript", "python"],
    roots: ["crates", "scripts", "tests"],
    excludes: ["generated", "vendor", "target", "node_modules"],
    assetManifest: manifestRelativePath,
    assetResourcesRoot: resourcesRoot,
    assetSourceKinds: ["drawio-shapes", "drawio-webapp"],
    assetBoundaryReason:
      "Exclude only unchanged Draw.io JavaScript assets explicitly owned by the upstream resource manifest; repository-owned helpers and tests remain in scope.",
    exceptions: [],
  };
  return { paths, before: hashes, after: new Map(hashes), policy };
}

function withManifestFixture(
  run: (fixture: { repo: string; asset: string; helper: string; manifest: string }) => void,
): void {
  const repo = mkdtempSync(path.join(os.tmpdir(), "krr-dup-assets-"));
  const asset = path.join(repo, resourcesRoot, "shapes", "external.js");
  const helper = path.join(repo, "crates/katana-render-runtime/src/owned-helper.js");
  const manifest = path.join(repo, manifestRelativePath);
  mkdirSync(path.dirname(asset), { recursive: true });
  mkdirSync(path.dirname(helper), { recursive: true });
  writeFileSync(asset, assetBytes);
  writeFileSync(helper, "repository-owned helper");
  writeFileSync(
    manifest,
    JSON.stringify({
      version: 1,
      resourcesRoot,
      resources: [
        {
          logicalPath: "shapes/external.js",
          sourceKind: "drawio-shapes",
          byteSize: Buffer.byteLength(assetBytes),
          sha256: sha256(assetBytes),
        },
      ],
    }),
  );
  try {
    run({ repo, asset, helper, manifest });
  } finally {
    rmSync(repo, { recursive: true, force: true });
  }
}

function exactException() {
  return {
    first: {
      path: "crates/first.rs",
      sha256: sha256(firstText),
      start: { line: 1, column: 1 },
      end: { line: 9, column: 4 },
    },
    second: {
      path: "scripts/second.py",
      sha256: sha256(secondText),
      start: { line: 3, column: 1 },
      end: { line: 11, column: 4 },
    },
    fragmentSha256: sha256(fragment),
    reason: "intentional shared protocol fixture",
  };
}

describe("duplication report validation", () => {
  test.each([
    ["root", (value: ReturnType<typeof report>) => ({ ...value, skipLocal: true })],
    [
      "statistics",
      (value: ReturnType<typeof report>) => ({
        ...value,
        statistics: { ...value.statistics, unknown: true },
      }),
    ],
    [
      "duplicate",
      (value: ReturnType<typeof report>) => ({
        ...value,
        duplicates: value.duplicates.map((duplicate) => ({ ...duplicate, approved: true })),
      }),
    ],
    [
      "location",
      (value: ReturnType<typeof report>) => ({
        ...value,
        duplicates: value.duplicates.map((duplicate) => ({
          ...duplicate,
          firstFile: {
            ...duplicate.firstFile,
            startLoc: { ...duplicate.firstFile.startLoc, unknown: true },
          },
        })),
      }),
    ],
  ])("rejects unknown report keys at the %s boundary", (_, change) => {
    const state = sourceState();
    expect(() =>
      validateReport(repo, state.paths, state.before, state.after, change(report(1)), state.policy),
    ).toThrow("Unrecognized key");
  });

  test("rejects missing format statistics and inconsistent chunk ranges", () => {
    const state = sourceState();
    const incomplete = report();
    const { python: _python, ...formats } = incomplete.statistics.formats;
    expect(() =>
      validateInventoryReport(
        state.paths,
        { ...incomplete, statistics: { ...incomplete.statistics, formats } },
        state.policy,
      ),
    ).toThrow();
    const malformed = report(1);
    const duplicate = malformed.duplicates[0];
    if (!duplicate) throw new Error("fixture duplicate is missing");
    duplicate.firstFile.endLoc.line += 1;
    expect(() =>
      validateReport(repo, state.paths, state.before, state.after, malformed, state.policy),
    ).toThrow("chunk range");
  });

  test("rejects policy weakening and malformed nested exception identities", () => {
    const { policy } = sourceState();
    expect(() => policySchema.parse({ ...policy, skipLocal: true })).toThrow("Unrecognized key");
    expect(() => policySchema.parse({ ...policy, minLines: 7 })).toThrow();
    const exception = exactException();
    expect(() =>
      policySchema.parse({
        ...policy,
        exceptions: [{ ...exception, first: { ...exception.first, path: "../outside.rs" } }],
      }),
    ).toThrow();
    expect(() =>
      policySchema.parse({ ...policy, exceptions: [{ ...exception, reason: " " }] }),
    ).toThrow();
    expect(() =>
      policySchema.parse({
        ...policy,
        exceptions: [{ ...exception, first: { ...exception.first, sha256: "stale" } }],
      }),
    ).toThrow();
  });

  test("accepts inventory statistics only when every supported source is scanned", () => {
    const state = sourceState();
    expect(() => validateInventoryReport(state.paths, report(), state.policy)).not.toThrow();
    const incomplete = report();
    incomplete.statistics.formats.python.sources = 0;
    expect(() => validateInventoryReport(state.paths, incomplete, state.policy)).toThrow(
      "source count",
    );
  });

  test("excludes only physically empty sources with an exact unchanged source snapshot", () => {
    const fixture = mkdtempSync(path.join(os.tmpdir(), "krr-dup-empty-source-"));
    const empty = path.join(fixture, "empty.js");
    const rust = path.join(fixture, "source.rs");
    const whitespace = path.join(fixture, "whitespace.ts");
    const commentOnly = path.join(fixture, "comment-only.js");
    const unsupported = path.join(fixture, "unsupported.txt");
    try {
      writeFileSync(empty, "");
      writeFileSync(rust, "fn current_source() {}\n");
      const paths = [empty, rust];
      const hashes = new Map(paths.map((file) => [file, sha256(readFileSync(file))]));
      const probe = report();
      probe.statistics.total.sources = 1;
      probe.statistics.formats.rust.sources = 1;
      probe.statistics.formats.python.sources = 0;
      expect(() => validateInventoryReport(paths, probe, sourceState().policy)).toThrow(
        "source count",
      );
      expect(() =>
        validateInventoryReport(paths, probe, sourceState().policy, { paths, hashes }),
      ).not.toThrow();

      const staleHashes = new Map(hashes);
      staleHashes.set(empty, sha256("was not empty"));
      expect(() =>
        validateInventoryReport(paths, probe, sourceState().policy, {
          paths,
          hashes: staleHashes,
        }),
      ).toThrow("source changed");
      expect(() =>
        validateInventoryReport(paths, probe, sourceState().policy, {
          paths: [...paths, path.join(fixture, "omitted.rs")],
          hashes,
        }),
      ).toThrow("exactly match");

      writeFileSync(whitespace, " \n");
      writeFileSync(commentOnly, "// retained source\n");
      const nonemptyPaths = [...paths, whitespace, commentOnly];
      const nonemptyHashes = new Map(
        nonemptyPaths.map((file) => [file, sha256(readFileSync(file))]),
      );
      expect(() =>
        validateInventoryReport(nonemptyPaths, probe, sourceState().policy, {
          paths: nonemptyPaths,
          hashes: nonemptyHashes,
        }),
      ).toThrow("source count");

      writeFileSync(unsupported, "source");
      const unsupportedHashes = new Map([[unsupported, sha256(readFileSync(unsupported))]]);
      expect(() =>
        validateInventoryReport([unsupported], probe, sourceState().policy, {
          paths: [unsupported],
          hashes: unsupportedHashes,
        }),
      ).toThrow("unsupported source extension");

      writeFileSync(empty, "became nonempty");
      expect(() =>
        validateInventoryReport(paths, probe, sourceState().policy, { paths, hashes }),
      ).toThrow("source changed");
    } finally {
      rmSync(fixture, { recursive: true, force: true });
    }
  });

  test("accepts a clean report", () => {
    const state = sourceState();
    expect(
      validateReport(repo, state.paths, state.before, state.after, report(), state.policy),
    ).toBe(0);
  });

  test("rejects an unapproved clone", () => {
    const state = sourceState();
    const unresolved = validateReport(
      repo,
      state.paths,
      state.before,
      state.after,
      report(1),
      state.policy,
    );
    expect(() => assertNoUnapprovedClones(unresolved)).toThrow("unapproved clone");
  });

  test("accepts only the exact approved clone pair", () => {
    const state = sourceState();
    state.policy.exceptions.push(exactException());
    expect(
      validateReport(repo, state.paths, state.before, state.after, report(1), state.policy),
    ).toBe(0);
  });

  test("rejects stale source hashes and unmatched exceptions", () => {
    const stale = sourceState();
    const exception = exactException();
    exception.first.sha256 = "0".repeat(64);
    stale.policy.exceptions.push(exception);
    expect(() =>
      validateReport(repo, stale.paths, stale.before, stale.after, report(1), stale.policy),
    ).toThrow("stale exception");

    const unmatched = sourceState();
    unmatched.policy.exceptions.push(exactException());
    expect(() =>
      validateReport(
        repo,
        unmatched.paths,
        unmatched.before,
        unmatched.after,
        report(),
        unmatched.policy,
      ),
    ).toThrow("unmatched");
  });

  test("rejects source changes and paths escaping the repository", () => {
    const changed = sourceState();
    changed.after.set(firstPath, sha256("changed source"));
    expect(() =>
      validateReport(repo, changed.paths, changed.before, changed.after, report(), changed.policy),
    ).toThrow("changed during");

    const escaped = sourceState();
    expect(() =>
      validateReport(
        repo,
        escaped.paths,
        escaped.before,
        escaped.after,
        report(1, "/outside/escape.rs"),
        escaped.policy,
      ),
    ).toThrow("escapes repository");
  });

  test("rejects incomplete statistics and failed engine status", () => {
    const state = sourceState();
    const incomplete = report();
    incomplete.statistics.total.sources = 1;
    expect(() =>
      validateReport(repo, state.paths, state.before, state.after, incomplete, state.policy),
    ).toThrow("source count");
    expect(() => assertToolSucceeded(1)).toThrow("status 1");
    expect(() => assertToolSucceeded(null)).toThrow("status null");
    expect(() => assertToolSucceeded(0)).not.toThrow();
  });

  test("rejects malformed chunk locations and non-integer statistics", () => {
    const state = sourceState();
    const malformed = report(1);
    const duplicate = malformed.duplicates[0];
    if (duplicate === undefined) throw new Error("fixture duplicate is missing");
    duplicate.firstFile.startLoc.line = -1;
    expect(() =>
      validateReport(repo, state.paths, state.before, state.after, malformed, state.policy),
    ).toThrow("duplicate record");

    const invalidCounts = report();
    invalidCounts.statistics.total.sources = -1;
    expect(() =>
      validateReport(repo, state.paths, state.before, state.after, invalidCounts, state.policy),
    ).toThrow("source count");
  });
});

describe("jscpd configuration isolation", () => {
  test("ambient skipLocal cannot suppress clones in the pinned engine run", () => {
    const fixtureRepo = mkdtempSync(path.join(os.tmpdir(), "krr-dup-config-"));
    const first = path.join(fixtureRepo, "crates/a.rs");
    const second = path.join(fixtureRepo, "crates/b.rs");
    const output = path.join(fixtureRepo, "tmp/quality/duplication");
    const source = `fn repeated(value: usize) -> usize {\n${Array.from(
      { length: 16 },
      (_, index) => `    let item_${index} = value + ${index};`,
    ).join("\n")}\n    value\n}\n`;
    mkdirSync(path.dirname(first), { recursive: true });
    mkdirSync(path.join(fixtureRepo, "scripts"), { recursive: true });
    mkdirSync(path.join(fixtureRepo, "tests"), { recursive: true });
    mkdirSync(output, { recursive: true });
    writeFileSync(path.join(fixtureRepo, ".jscpd.json"), JSON.stringify({ skipLocal: true }));
    writeFileSync(first, source);
    writeFileSync(second, source);
    try {
      const ambientOutput = path.join(fixtureRepo, "tmp/ambient");
      mkdirSync(ambientOutput, { recursive: true });
      execFileSync(
        "uvx",
        [
          "--from",
          "jscpd==5.4.1",
          "jscpd",
          "--absolute",
          "--format",
          "rust",
          "--mode",
          "mild",
          "--min-lines",
          "8",
          "--min-tokens",
          "75",
          "--threshold",
          "100",
          "--reporters",
          "json",
          "--report-name",
          "ambient",
          "--output",
          ambientOutput,
          path.join(fixtureRepo, "crates"),
        ],
        { cwd: fixtureRepo, stdio: "ignore" },
      );
      const ambient = JSON.parse(
        readFileSync(path.join(ambientOutput, "ambient.json"), "utf8"),
      ) as {
        statistics: { total: { clones: number } };
      };
      expect(ambient.statistics.total.clones).toBe(0);

      const { policy } = sourceState();
      runJscpd(fixtureRepo, output, policy, new Map(), 8, 75);
      const isolated = JSON.parse(readFileSync(path.join(output, "report.json"), "utf8")) as {
        statistics: { total: { clones: number } };
      };
      expect(isolated.statistics.total.clones).toBeGreaterThan(0);

      const finalReport = readFileSync(path.join(output, "report.json"));
      runJscpd(fixtureRepo, output, policy, new Map(), 8, 75, "probe");
      expect(readFileSync(path.join(output, "report.json"))).toEqual(finalReport);
      const probeRecord = JSON.parse(
        readFileSync(path.join(output, "reports", "probe-latest.json"), "utf8"),
      ) as { report: string };
      const probePath = path.join(output, probeRecord.report);
      const archivedProbe = readFileSync(probePath);

      runJscpd(fixtureRepo, output, policy, new Map(), 8, 75, "final");
      const finalRecord = JSON.parse(
        readFileSync(path.join(output, "reports", "final-latest.json"), "utf8"),
      ) as { report: string };
      expect(gunzipSync(readFileSync(path.join(output, finalRecord.report)))).toEqual(
        readFileSync(path.join(output, "report.json")),
      );
      expect(readFileSync(probePath)).toEqual(archivedProbe);
    } finally {
      rmSync(fixtureRepo, { recursive: true, force: true });
    }
  });

  test("scans configured tests root and honors its ignored cache directories", () => {
    const fixtureRepo = mkdtempSync(path.join(os.tmpdir(), "krr-dup-tests-root-"));
    const root = "tests/fixtures/quality-cache";
    const source = `export function repeated(value: number): number {\n${Array.from(
      { length: 17 },
      (_, index) => `  const item_${index} = value + ${index};`,
    ).join("\n")}\n  return value;\n}\n`;
    const output = path.join(fixtureRepo, "tmp/quality/duplication");
    mkdirSync(output, { recursive: true });
    for (const directory of ["crates", "scripts", "tests"]) {
      mkdirSync(path.join(fixtureRepo, directory), { recursive: true });
    }
    for (const directory of [".moon", ".probe", ".outputs"]) {
      mkdirSync(path.join(fixtureRepo, root, directory), { recursive: true });
      writeFileSync(path.join(fixtureRepo, root, directory, "ignored.ts"), source);
    }
    mkdirSync(path.join(fixtureRepo, root), { recursive: true });
    writeFileSync(path.join(fixtureRepo, root, "subject.ts"), source);
    writeFileSync(path.join(fixtureRepo, root, "subject.test.ts"), source);
    writeFileSync(
      path.join(fixtureRepo, ".gitignore"),
      [
        "tests/fixtures/quality-cache/.moon/",
        "tests/fixtures/quality-cache/.probe/",
        "tests/fixtures/quality-cache/.outputs/",
      ].join("\n"),
    );
    execFileSync("git", ["init", "--quiet"], { cwd: fixtureRepo });
    const { policy } = sourceState();
    try {
      runJscpd(fixtureRepo, output, policy, new Map(), 8, 75);
      const scanned = JSON.parse(readFileSync(path.join(output, "report.json"), "utf8")) as {
        statistics: {
          total: { sources: number; clones: number };
          formats: { typescript: { sources: number; clones: number } };
        };
      };
      expect(scanned.statistics.total.sources).toBe(2);
      expect(scanned.statistics.total.clones).toBe(1);
      expect(scanned.statistics.formats.typescript.sources).toBe(2);
      expect(scanned.statistics.formats.typescript.clones).toBe(1);
    } finally {
      rmSync(fixtureRepo, { recursive: true, force: true });
    }
  });
});

describe("jscpd report preservation", () => {
  test("archives compressed probe and final reports without replacing raw latest content", () => {
    const output = mkdtempSync(path.join(os.tmpdir(), "krr-dup-report-history-"));
    const latest = path.join(output, "report.json");
    const firstReport = Buffer.from(JSON.stringify(report(1)));
    const finalReport = Buffer.from(JSON.stringify(report(0)));
    writeFileSync(latest, "pre-existing final report");
    try {
      const probeSha = storeJscpdReport(output, firstReport, "probe");
      const archivedProbe = path.join(output, "reports", `${probeSha}.json.gz`);
      const compressedProbe = readFileSync(archivedProbe);
      const contentSha = sha256(firstReport);
      expect(sha256(compressedProbe)).toBe(probeSha);
      expect(probeSha).not.toBe(contentSha);
      expect(gunzipSync(compressedProbe)).toEqual(firstReport);
      expect(readFileSync(latest, "utf8")).toBe("pre-existing final report");
      expect(
        JSON.parse(readFileSync(path.join(output, "reports", "probe-latest.json"), "utf8")),
      ).toEqual({
        role: "probe",
        sha256: probeSha,
        contentSha256: contentSha,
        encoding: "gzip",
        report: `reports/${probeSha}.json.gz`,
      });

      expect(storeJscpdReport(output, firstReport, "probe")).toBe(probeSha);
      expect(readFileSync(archivedProbe)).toEqual(compressedProbe);
      expect(storeJscpdReport(output, firstReport, "final")).toBe(probeSha);
      expect(
        JSON.parse(readFileSync(path.join(output, "reports", "final-latest.json"), "utf8")),
      ).toMatchObject({ role: "final", sha256: probeSha });
      expect(readFileSync(archivedProbe)).toEqual(compressedProbe);
      const finalSha = storeJscpdReport(output, finalReport, "final");
      expect(finalSha).not.toBe(probeSha);
      expect(readFileSync(latest)).toEqual(finalReport);
      expect(gunzipSync(readFileSync(archivedProbe))).toEqual(firstReport);
      expect(gunzipSync(readFileSync(path.join(output, "reports", `${finalSha}.json.gz`)))).toEqual(
        finalReport,
      );
      expect(
        JSON.parse(readFileSync(path.join(output, "reports", "probe-latest.json"), "utf8")).role,
      ).toBe("probe");
      expect(
        JSON.parse(readFileSync(path.join(output, "reports", "final-latest.json"), "utf8")).role,
      ).toBe("final");
    } finally {
      rmSync(output, { recursive: true, force: true });
    }
  });

  test("reuses only intact gzip CAS content and rejects corrupt gzip and legacy JSON remains readable", () => {
    const output = mkdtempSync(path.join(os.tmpdir(), "krr-dup-report-corrupt-"));
    try {
      const bytes = Buffer.from(JSON.stringify(report(1)));
      const digest = storeJscpdReport(output, bytes, "probe");
      const archived = path.join(output, "reports", `${digest}.json.gz`);
      writeFileSync(archived, "corrupt gzip");
      expect(() => storeJscpdReport(output, bytes, "probe")).toThrow(
        "content-addressed compressed report is invalid",
      );
      expect(() => readCurrentReport(archived)).toThrow();

      const legacy = path.join(output, "legacy-report.json");
      writeFileSync(legacy, bytes);
      expect(readCurrentReport(legacy)).toEqual(report(1));
    } finally {
      rmSync(output, { recursive: true, force: true });
    }
  });

  test("keeps short owned sources in the probe and validates the thresholded final report", () => {
    const fixtureRepo = realpathSync(
      mkdtempSync(path.join(os.tmpdir(), "krr-dup-final-threshold-")),
    );
    const output = path.join(fixtureRepo, "tmp/quality/duplication");
    const longSource = (name: string) =>
      [
        "fn repeated(value: usize) -> usize {",
        ...Array.from({ length: 18 }, (_, index) => `    let item_${index} = value + ${index};`),
        "    value",
        "}",
        ...Array.from({ length: 400 }, (_, index) => `// ${name}_unique_${index}`),
      ].join("\n");
    for (const root of ["crates", "scripts", "tests"]) {
      mkdirSync(path.join(fixtureRepo, root), { recursive: true });
    }
    const shortSource = path.join(fixtureRepo, "crates/short.rs");
    writeFileSync(shortSource, "const SHORT: usize = 1;\n");
    for (const name of ["first", "second"]) {
      writeFileSync(path.join(fixtureRepo, `crates/${name}.rs`), longSource(name));
    }
    for (const [relative, statement, comment] of [
      ["scripts/support.ts", "export const value", "//"],
      ["scripts/support.js", "export const value", "//"],
      ["tests/support.py", "value", "#"],
    ]) {
      if (relative === undefined || statement === undefined || comment === undefined) {
        throw new Error("support source row is incomplete");
      }
      const content = [
        ...Array.from({ length: 30 }, (_, index) => `${statement}_${index} = ${index};`),
        ...Array.from({ length: 400 }, (_, index) => `${comment} support_unique_${index}`),
      ].join("\n");
      writeFileSync(path.join(fixtureRepo, relative), content);
    }
    execFileSync("git", ["init", "--quiet"], { cwd: fixtureRepo });
    const { policy } = sourceState();
    try {
      const paths = collectOwnedSourcePaths(fixtureRepo, policy);
      expect(paths).toHaveLength(6);
      expect(paths).toContain(shortSource);
      const hashes = () => new Map(paths.map((file) => [file, sha256(readFileSync(file))]));
      const before = hashes();
      const probePath = runAndValidateInventoryProbe(fixtureRepo, output, policy, new Map(), paths);
      const probeBytes = readFileSync(probePath);
      const probe = reportSchema.parse(JSON.parse(gunzipSync(probeBytes).toString()));
      expect(probe.statistics.total.sources).toBe(6);
      expect(probe.statistics.formats.rust.sources).toBe(3);
      expect(() => validateInventoryReport(paths.slice(0, 2), probe, policy)).toThrow(
        "source count",
      );

      const finalPath = runJscpd(
        fixtureRepo,
        output,
        policy,
        new Map(),
        policy.minLines,
        policy.minTokens,
        "final",
      );
      const final = reportSchema.parse(readCurrentReport(finalPath));
      expect(final.statistics.total.sources).toBe(5);
      expect(final.statistics.formats.rust.sources).toBe(2);
      expect(final.duplicates.length).toBeGreaterThan(0);
      const unresolved = validateReport(fixtureRepo, paths, before, hashes(), final, policy);
      expect(unresolved).toBe(final.duplicates.length);
      expect(() => assertNoUnapprovedClones(unresolved)).toThrow("unapproved clone");
      expect(readFileSync(probePath)).toEqual(probeBytes);
      expect(readFileSync(path.join(output, "report.json"))).toEqual(
        gunzipSync(readFileSync(finalPath)),
      );
      writeFileSync(shortSource, "const SHORT: usize = 2;\n");
      expect(() => validateReport(fixtureRepo, paths, before, hashes(), final, policy)).toThrow(
        "source changed",
      );
    } finally {
      rmSync(fixtureRepo, { recursive: true, force: true });
    }
  });

  test("validates the actual inventory probe instead of an empty latest report", () => {
    const fixtureRepo = mkdtempSync(path.join(os.tmpdir(), "krr-dup-probe-binding-"));
    const first = path.join(fixtureRepo, "crates/first.rs");
    const second = path.join(fixtureRepo, "crates/second.rs");
    const outputRoot = path.join(fixtureRepo, "tmp/quality");
    const fixture = (name: string) => {
      const body = Array.from(
        { length: 400 },
        (_, index) => `    let ${name}_${index} = ${index}usize.wrapping_mul(${index + 7});`,
      ).join("\n");
      return [
        "fn repeated(value: usize) -> usize {",
        "    let result = value + 1;",
        "    result",
        "}",
        "",
        `fn unique_${name}() -> usize {`,
        body,
        `    ${name}_399`,
        "}",
      ].join("\n");
    };
    mkdirSync(path.join(fixtureRepo, "scripts"), { recursive: true });
    mkdirSync(path.join(fixtureRepo, "tests"), { recursive: true });
    mkdirSync(path.dirname(first), { recursive: true });
    mkdirSync(outputRoot, { recursive: true });
    writeFileSync(first, fixture("alpha"));
    writeFileSync(second, fixture("beta"));
    const typescriptSource = [
      "export function repeated(value: number): number {",
      ...Array.from({ length: 14 }, (_, index) => `  const item_${index} = value + ${index};`),
      "  return value;",
      "}",
    ].join("\n");
    const javascriptSource = [
      "export function repeated(value) {",
      ...Array.from({ length: 14 }, (_, index) => `  const item_${index} = value + ${index};`),
      "  return value;",
      "}",
    ].join("\n");
    const pythonSource = [
      "def repeated(value):",
      ...Array.from({ length: 14 }, (_, index) => `    item_${index} = value + ${index}`),
      "    return value",
    ].join("\n");
    const typescriptPaths = ["first.ts", "second.ts"].map((name) =>
      path.join(fixtureRepo, "scripts", name),
    );
    const javascriptPaths = ["first.js", "second.js"].map((name) =>
      path.join(fixtureRepo, "scripts", name),
    );
    const pythonPaths = ["first.py", "second.py"].map((name) =>
      path.join(fixtureRepo, "tests", name),
    );
    for (const file of typescriptPaths) writeFileSync(file, typescriptSource);
    for (const file of javascriptPaths) writeFileSync(file, javascriptSource);
    for (const file of pythonPaths) writeFileSync(file, pythonSource);

    const { policy } = sourceState();
    try {
      const paths = [first, second, ...typescriptPaths, ...javascriptPaths, ...pythonPaths];
      const missingLatestOutput = path.join(outputRoot, "missing-latest");
      const missingLatestProbePath = runAndValidateInventoryProbe(
        fixtureRepo,
        missingLatestOutput,
        policy,
        new Map(),
        paths,
      );
      const missingLatestProbe = JSON.parse(
        gunzipSync(readFileSync(missingLatestProbePath)).toString("utf8"),
      ) as {
        statistics: { total: { clones: number } };
      };
      expect(missingLatestProbe.statistics.total.clones).toBeGreaterThan(0);
      expect(existsSync(path.join(missingLatestOutput, "report.json"))).toBe(false);

      const emptyLatestOutput = path.join(outputRoot, "empty-latest");
      mkdirSync(emptyLatestOutput, { recursive: true });
      const emptyLatestPath = path.join(emptyLatestOutput, "report.json");
      writeFileSync(emptyLatestPath, "");
      const emptyLatestProbePath = runAndValidateInventoryProbe(
        fixtureRepo,
        emptyLatestOutput,
        policy,
        new Map(),
        paths,
      );
      expect(readFileSync(emptyLatestPath, "utf8")).toBe("");
      expect(
        JSON.parse(gunzipSync(readFileSync(emptyLatestProbePath)).toString("utf8")).statistics.total
          .clones,
      ).toBeGreaterThan(0);

      const acceptedLatestOutput = path.join(outputRoot, "accepted-latest");
      const acceptedReportPath = runJscpd(
        fixtureRepo,
        acceptedLatestOutput,
        policy,
        new Map(),
        8,
        75,
        "final",
      );
      const acceptedReport = readCurrentReport(acceptedReportPath) as {
        statistics: { total: { clones: number } };
      };
      expect(acceptedReport.statistics.total.clones).toBeGreaterThan(0);
      const acceptedLatest = readFileSync(path.join(acceptedLatestOutput, "report.json"));
      const actualProbePath = runAndValidateInventoryProbe(
        fixtureRepo,
        acceptedLatestOutput,
        policy,
        new Map(),
        paths,
      );
      const actualProbe = JSON.parse(
        gunzipSync(readFileSync(actualProbePath)).toString("utf8"),
      ) as {
        statistics: { total: { clones: number } };
      };
      expect(actualProbe.statistics.total.clones).toBeGreaterThan(
        acceptedReport.statistics.total.clones,
      );
      expect(readFileSync(path.join(acceptedLatestOutput, "report.json"))).toEqual(acceptedLatest);
      expect(actualProbePath).not.toBe(acceptedReportPath);
    } finally {
      rmSync(fixtureRepo, { recursive: true, force: true });
    }
  });
});

describe("Draw.io asset ownership boundary", () => {
  test("rejects unknown manifest fields, escaping paths and duplicate identities", () => {
    withManifestFixture(({ repo, asset, helper, manifest }) => {
      const original = JSON.parse(readFileSync(manifest, "utf8")) as {
        resources: Array<{
          logicalPath: string;
          sourceKind: string;
          byteSize: number;
          sha256: string;
        }>;
      };
      const entry = original.resources[0];
      if (!entry) throw new Error("fixture manifest entry is missing");
      const verify = () =>
        verifiedThirdPartyJavascript(repo, [asset, helper], manifestRelativePath, resourcesRoot, [
          "drawio-shapes",
          "drawio-webapp",
        ]);
      for (const resources of [
        [{ ...entry, logicalPath: "../outside.js" }],
        [{ ...entry, byteSize: -1 }],
        [{ ...entry, sha256: "unverified" }],
        [{ ...entry, approved: true }],
        [entry, entry],
      ]) {
        writeFileSync(manifest, JSON.stringify({ ...original, resources }));
        expect(verify).toThrow();
      }
      writeFileSync(manifest, JSON.stringify({ ...original, skipVerification: true }));
      expect(verify).toThrow("Unrecognized key");
    });
  });

  test("excludes only hash-pinned third-party JavaScript and keeps owned helpers", () => {
    withManifestFixture(({ repo, asset, helper }) => {
      const excluded = verifiedThirdPartyJavascript(
        repo,
        [asset, helper],
        manifestRelativePath,
        resourcesRoot,
        ["drawio-shapes", "drawio-webapp"],
      );
      expect(excluded.get(asset)).toBe(sha256(assetBytes));
      expect(excluded.has(helper)).toBe(false);
    });
  });

  test("rejects a third-party asset whose physical hash or size differs from the manifest", () => {
    withManifestFixture(({ repo, asset, helper }) => {
      writeFileSync(asset, "changed asset");
      expect(() =>
        verifiedThirdPartyJavascript(repo, [asset, helper], manifestRelativePath, resourcesRoot, [
          "drawio-shapes",
          "drawio-webapp",
        ]),
      ).toThrow("hash/size mismatch");
    });
  });

  test("rejects unknown source kinds and does not omit an unlisted owned JavaScript file", () => {
    withManifestFixture(({ repo, asset, helper, manifest }) => {
      const content = JSON.parse(readFileSync(manifest, "utf8")) as {
        resources: Array<Record<string, unknown>>;
      };
      const entry = content.resources[0];
      if (entry === undefined) throw new Error("fixture manifest entry is missing");
      entry.sourceKind = "unknown-origin";
      writeFileSync(manifest, JSON.stringify(content));
      expect(() =>
        verifiedThirdPartyJavascript(repo, [asset, helper], manifestRelativePath, resourcesRoot, [
          "drawio-shapes",
          "drawio-webapp",
        ]),
      ).toThrow("unknown sourceKind");
      entry.sourceKind = "drawio-shapes";
      writeFileSync(manifest, JSON.stringify(content));
      const excluded = verifiedThirdPartyJavascript(
        repo,
        [asset, helper],
        manifestRelativePath,
        resourcesRoot,
        ["drawio-shapes", "drawio-webapp"],
      );
      expect(excluded.has(helper)).toBe(false);
    });
  });
});

describe("duplication gate CLI arguments", () => {
  test("keeps no arguments mapped to the existing gate", () => {
    expect(resolveCliArguments([])).toEqual({ kind: "run" });
  });

  test("accepts only the exact help argument", () => {
    expect(resolveCliArguments(["--help"])).toEqual({ kind: "help" });
    expect(resolveCliArguments(["--help", "--extra"])).toEqual({
      kind: "invalid",
      arguments: ["--help", "--extra"],
    });
  });

  const unsupportedArgumentCases: Array<[string, string[]]> = [
    ["single unknown option", ["--unknown"]],
    ["help combined with unknown option", ["--help", "--unknown"]],
  ];
  test.each(unsupportedArgumentCases)(
    "rejects unsupported arguments before running the gate: %s",
    (_label, args) => {
      withDuplicationCliFixture(({ repo, protectedFiles, output }) => {
        const before = protectedFileSnapshot(protectedFiles, output);
        const result = runDuplicationCli(repo, args);
        const stdout = new TextDecoder().decode(result.stdout);
        const stderr = new TextDecoder().decode(result.stderr);
        expect(result.exitCode).toBe(2);
        expect(stderr).toContain("Unknown or unsupported arguments:");
        expect(stderr).toContain("Usage: bun scripts/quality/duplication-check.ts [--help]");
        expect(stdout).toBe("");
        expect(stderr).not.toContain("duplication gate passed:");
        expect(protectedFileSnapshot(protectedFiles, output)).toEqual(before);
      });
    },
  );

  test("prints help and exits without changing reports or policy", () => {
    withDuplicationCliFixture(({ repo, protectedFiles, output }) => {
      const before = protectedFileSnapshot(protectedFiles, output);
      const result = runDuplicationCli(repo, ["--help"]);
      const stdout = new TextDecoder().decode(result.stdout);
      const stderr = new TextDecoder().decode(result.stderr);
      expect(result.exitCode).toBe(0);
      expect(stdout).toBe("Usage: bun scripts/quality/duplication-check.ts [--help]\n");
      expect(stderr).toBe("");
      expect(protectedFileSnapshot(protectedFiles, output)).toEqual(before);
    });
  });
});
