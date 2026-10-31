import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { z } from "zod";
import {
  collectRuntimeBundleToolFingerprint,
  FINGERPRINT_SCHEMA,
  type RuntimeBundleToolFingerprint,
} from "./runtime-bundle-tool-fingerprint";

const repositoryRoot = path.resolve(import.meta.dir, "../..");
const secretName = "KRR_TOOL_FINGERPRINT_TEST_SECRET";
const secretValue = "test-only-secret-value-must-not-appear";
const hash = /^[a-f0-9]{64}$/;
const allowedEnvironmentNames = [
  "BUN_CONFIG_NO_CLEAR_TERMINAL",
  "LANG",
  "LC_ALL",
  "LC_CTYPE",
  "NODE_ENV",
  "PATH",
  "SOURCE_DATE_EPOCH",
  "TZ",
  "npm_config_user_agent",
].sort();
type CliResult = { exitCode: number; stdout: string; stderr: string };
type FingerprintRuns = {
  baseline: CliResult;
  repeated: CliResult;
  changedTimezone: CliResult;
  changedSecret: CliResult;
  missingTools: CliResult;
  unsupportedOverride: CliResult;
  baseEnvironment: NodeJS.ProcessEnv;
};

function fingerprintCli(env: NodeJS.ProcessEnv): {
  exitCode: number;
  stdout: string;
  stderr: string;
} {
  const fingerprintScript = path.join(import.meta.dir, "runtime-bundle-tool-fingerprint.ts");
  const result = Bun.spawnSync([process.execPath, fingerprintScript], {
    cwd: repositoryRoot,
    env,
    stdin: "ignore",
    stdout: "pipe",
    stderr: "pipe",
  });
  return {
    exitCode: result.exitCode,
    stdout: new TextDecoder().decode(result.stdout),
    stderr: new TextDecoder().decode(result.stderr),
  };
}

describe("runtime bundle tool fingerprint CLI", () => {
  let runs: FingerprintRuns | undefined;
  let emptyPath = "";

  beforeAll(() => {
    emptyPath = fs.mkdtempSync(path.join(os.tmpdir(), "krr-fingerprint-empty-path-"));
    const baseEnvironment = { ...process.env, TZ: "UTC" };
    runs = {
      baseline: fingerprintCli(baseEnvironment),
      repeated: fingerprintCli(baseEnvironment),
      changedTimezone: fingerprintCli({ ...baseEnvironment, TZ: "Asia/Tokyo" }),
      changedSecret: fingerprintCli({ ...baseEnvironment, [secretName]: secretValue }),
      missingTools: fingerprintCli({ ...baseEnvironment, PATH: emptyPath }),
      unsupportedOverride: fingerprintCli({
        ...baseEnvironment,
        SWC_BINARY_PATH: path.join(emptyPath, "unsupported-swc-binary"),
      }),
      baseEnvironment,
    };
  });

  afterAll(() => {
    if (emptyPath) fs.rmSync(emptyPath, { recursive: true, force: true });
  });

  test("prints deterministic JSON for identical tool and environment inputs", () => {
    const { baseline, repeated } = requireRuns(runs);
    expect(baseline.exitCode).toBe(0);
    expect(repeated.exitCode).toBe(0);
    expect(repeated.stdout).toBe(baseline.stdout);
    expect(baseline.stderr).toBe("");
    expect(baseline.stdout.endsWith("\n")).toBe(true);
  });

  test("binds the real Bun, Just, Node, Moon and installed Moon package bytes", () => {
    const { baseline, baseEnvironment } = requireRuns(runs);
    const fingerprint: unknown = JSON.parse(baseline.stdout);
    const moonPackage = z
      .object({ version: z.string() })
      .parse(
        JSON.parse(
          fs.readFileSync(
            path.join(repositoryRoot, "node_modules/@moonrepo/cli/package.json"),
            "utf8",
          ),
        ),
      );
    expect(fingerprint).toMatchObject({
      schema: FINGERPRINT_SCHEMA,
      platform: process.platform,
      arch: process.arch,
      tools: {
        bash: {
          version: expect.stringContaining("GNU bash"),
          executableSha256: expect.stringMatching(hash),
        },
        bun: { version: Bun.version, executableSha256: expect.stringMatching(hash) },
        just: {
          version: expect.stringMatching(/\S/),
          executableSha256: expect.stringMatching(hash),
        },
        node: {
          version: expect.stringMatching(/^v\d/),
          executableSha256: expect.stringMatching(hash),
        },
        moon: {
          version: `moon ${moonPackage.version}`,
          executableSha256: expect.stringMatching(hash),
          wrapperSha256: expect.stringMatching(hash),
          utilsSha256: expect.stringMatching(hash),
          packageSha256: expect.stringMatching(hash),
        },
      },
    });
    const parsed = fingerprint as RuntimeBundleToolFingerprint;
    expect(Object.keys(parsed).sort()).toEqual([
      "arch",
      "environment",
      "platform",
      "schema",
      "tools",
    ]);
    expect(Object.keys(parsed.tools).sort()).toEqual(["bash", "bun", "just", "moon", "node"]);
    expect(Object.keys(parsed.tools.bash).sort()).toEqual(["executableSha256", "version"]);
    expect(Object.keys(parsed.tools.bun).sort()).toEqual(["executableSha256", "version"]);
    expect(Object.keys(parsed.tools.just).sort()).toEqual(["executableSha256", "version"]);
    expect(Object.keys(parsed.tools.node).sort()).toEqual(["executableSha256", "version"]);
    expect(Object.keys(parsed.tools.moon).sort()).toEqual([
      "executableSha256",
      "packageSha256",
      "utilsSha256",
      "version",
      "wrapperSha256",
    ]);
    expect(baseline.stdout).not.toMatch(/"(?:timestamp|pid)"/i);
    expect(parsed.environment).toEqual(
      allowedEnvironmentNames
        .filter((name) => baseEnvironment[name] !== undefined)
        .map((name) => ({
          name,
          valueSha256: createHash("sha256")
            .update(baseEnvironment[name] ?? "")
            .digest("hex"),
        })),
    );
  });

  test("changes when the timezone environment changes", () => {
    const { baseline, changedTimezone } = requireRuns(runs);
    expect(changedTimezone.exitCode).toBe(0);
    expect(changedTimezone.stdout).not.toBe(baseline.stdout);
    const base: RuntimeBundleToolFingerprint = JSON.parse(baseline.stdout);
    const changed: RuntimeBundleToolFingerprint = JSON.parse(changedTimezone.stdout);
    expect(base.environment.find((entry) => entry.name === "TZ")?.valueSha256).toMatch(hash);
    expect(changed.environment.find((entry) => entry.name === "TZ")?.valueSha256).toMatch(hash);
    expect(changed.environment.find((entry) => entry.name === "TZ")?.valueSha256).not.toBe(
      base.environment.find((entry) => entry.name === "TZ")?.valueSha256,
    );
  });

  test("excludes secret environment values from both output and the binding", () => {
    const { baseline, changedSecret } = requireRuns(runs);
    expect(changedSecret.exitCode).toBe(0);
    expect(changedSecret.stdout).toBe(baseline.stdout);
    expect(changedSecret.stdout).not.toContain(secretValue);
    const fingerprint: RuntimeBundleToolFingerprint = JSON.parse(changedSecret.stdout);
    expect(fingerprint.environment.some((entry) => entry.name === secretName)).toBe(false);
  });

  test("fails closed when an actual tool cannot be resolved", () => {
    const { missingTools } = requireRuns(runs);
    expect(missingTools.exitCode).not.toBe(0);
    expect(missingTools.stdout).toBe("");
    expect(missingTools.stderr).toContain("fingerprint resolution failed");
  });

  test("fails closed when an external executable override is configured", () => {
    const { unsupportedOverride } = requireRuns(runs);
    expect(unsupportedOverride.exitCode).not.toBe(0);
    expect(unsupportedOverride.stdout).toBe("");
    expect(unsupportedOverride.stderr).toContain("fingerprint resolution failed");
  });

  test.each(["LD_PRELOAD", "LD_AUDIT", "LD_LIBRARY_PATH", "DYLD_INSERT_LIBRARIES"])(
    "rejects dynamic loader override %s without exposing its value",
    (name) => {
      const previous = process.env[name];
      const value = "fixture-loader-override";
      process.env[name] = value;
      try {
        expect(() => collectRuntimeBundleToolFingerprint()).toThrow(
          "unsupported dynamic loader override",
        );
      } finally {
        if (previous === undefined) delete process.env[name];
        else process.env[name] = previous;
      }
    },
  );
});

function requireRuns(runs: FingerprintRuns | undefined): FingerprintRuns {
  if (!runs) throw new Error("fingerprint CLI observations are unavailable");
  return runs;
}
