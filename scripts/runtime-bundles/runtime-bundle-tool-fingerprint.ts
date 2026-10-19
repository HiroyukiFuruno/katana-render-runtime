import { type ExecFileSyncOptionsWithStringEncoding, execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";

export const FINGERPRINT_SCHEMA = "krr-runtime-bundle-tool-fingerprint-v1" as const;
const REPOSITORY_ROOT = path.resolve(import.meta.dir, "../..");
const VERSION_PROBE_OPTIONS: ExecFileSyncOptionsWithStringEncoding = {
  cwd: REPOSITORY_ROOT,
  encoding: "utf8",
  env: process.env,
  stdio: ["ignore", "pipe", "pipe"],
};
const SAFE_ENV_NAMES = new Set([
  "PATH",
  "npm_config_user_agent",
  "NODE_ENV",
  "BUN_CONFIG_NO_CLEAR_TERMINAL",
  "SOURCE_DATE_EPOCH",
  "TZ",
  "LANG",
  "LC_ALL",
  "LC_CTYPE",
]);
const UNSUPPORTED_EXTERNAL_OVERRIDES = [
  "NODE_OPTIONS",
  "NODE_PATH",
  "BUN_OPTIONS",
  "BUN_ENV",
  "BASH_ENV",
  "ENV",
  "SWC_BINARY_PATH",
  "BIOME_BINARY",
  "BIOME_CONFIG_PATH",
  "ROLLUP_NATIVE",
] as const;
const SECRET_NAME = /(?:AUTH|TOKEN|PASSWORD|SECRET|CREDENTIAL|PRIVATE|REGISTRY|KEY)/i;

type ToolIdentity = {
  version: string;
  executableSha256: string;
};
type MoonIdentity = ToolIdentity & {
  wrapperSha256: string;
  utilsSha256: string;
  packageSha256: string;
};
export type RuntimeBundleToolFingerprint = {
  schema: typeof FINGERPRINT_SCHEMA;
  platform: string;
  arch: string;
  tools: {
    bun: ToolIdentity;
    just: ToolIdentity;
    node: ToolIdentity;
    bash: ToolIdentity;
    moon: MoonIdentity;
  };
  environment: { name: string; valueSha256: string }[];
};

function sha256(bytes: Uint8Array | string): string {
  return createHash("sha256").update(bytes).digest("hex");
}

function executablePath(name: string): string {
  const pathValue = process.env.PATH;
  if (!pathValue) throw new Error("PATH is unavailable");
  const found = Bun.which(name, { PATH: pathValue, cwd: REPOSITORY_ROOT });
  if (!found) throw new Error(`${name} executable could not be resolved from PATH`);
  return fs.realpathSync(found);
}

function version(output: string, name: string): string {
  const normalized = output.trim();
  if (!normalized) throw new Error(`${name} version probe returned no output`);
  return normalized;
}

function resolveMoon(): {
  executable: string;
  wrapper: string;
  utils: string;
  packageJson: string;
} {
  const require = createRequire(import.meta.url);
  const utils = require.resolve("@moonrepo/cli/utils");
  const packageJson = require.resolve("@moonrepo/cli/package.json");
  const wrapper = path.join(path.dirname(packageJson), "moon.js");
  const literalWrapper = path.join(REPOSITORY_ROOT, "node_modules", "@moonrepo", "cli", "moon.js");
  if (fs.realpathSync(wrapper) !== fs.realpathSync(literalWrapper))
    throw new Error("Moon package wrapper resolution differs");
  const bunMoonExe = (require("@moonrepo/cli/utils") as { findMoonExe?: unknown }).findMoonExe;
  if (typeof bunMoonExe !== "function") throw new Error("Moon resolver is unavailable");
  const bunResolved = (bunMoonExe as () => unknown)();
  if (typeof bunResolved !== "string") throw new Error("Moon resolver returned an invalid path");

  const node = executablePath("node");
  // Moon の shebang は PATH 上の Node で動くため、同じ環境で公式 resolver の解決先を照合する。
  const MOON_NODE_PROBE =
    "const { findMoonExe } = require('@moonrepo/cli/utils'); process.stdout.write(JSON.stringify({ execPath: process.execPath, moonExe: findMoonExe(), utilsPath: require.resolve('@moonrepo/cli/utils'), packagePath: require.resolve('@moonrepo/cli/package.json') }));";
  const result = execFileSync("node", ["-e", MOON_NODE_PROBE], VERSION_PROBE_OPTIONS).trim();
  const parsed: unknown = JSON.parse(result);
  if (
    typeof parsed !== "object" ||
    parsed === null ||
    !("execPath" in parsed) ||
    typeof parsed.execPath !== "string" ||
    !("moonExe" in parsed) ||
    typeof parsed.moonExe !== "string" ||
    !("utilsPath" in parsed) ||
    typeof parsed.utilsPath !== "string" ||
    !("packagePath" in parsed) ||
    typeof parsed.packagePath !== "string"
  )
    throw new Error("Node Moon resolver probe returned an invalid record");
  if (
    fs.realpathSync(parsed.execPath) !== node ||
    fs.realpathSync(parsed.moonExe) !== fs.realpathSync(bunResolved) ||
    fs.realpathSync(parsed.utilsPath) !== fs.realpathSync(utils) ||
    fs.realpathSync(parsed.packagePath) !== fs.realpathSync(packageJson)
  )
    throw new Error("Bun and Node Moon resolver contexts disagree");
  return { executable: fs.realpathSync(bunResolved), wrapper, utils, packageJson };
}

function environmentFingerprint(): RuntimeBundleToolFingerprint["environment"] {
  return [...SAFE_ENV_NAMES]
    .filter((name) => !SECRET_NAME.test(name) && process.env[name] !== undefined)
    .sort()
    .map((name) => ({ name, valueSha256: sha256(process.env[name] ?? "") }));
}

function verifySupportedEnvironment(): void {
  const unsupported = UNSUPPORTED_EXTERNAL_OVERRIDES.filter(
    (name) => (process.env[name] ?? "").length > 0,
  );
  if (unsupported.length > 0)
    throw new Error(`unsupported external override: ${unsupported.join(",")}`);
}

export function collectRuntimeBundleToolFingerprint(): RuntimeBundleToolFingerprint {
  verifySupportedEnvironment();
  const just = executablePath("just");
  const node = executablePath("node");
  const bash = executablePath("bash");
  const moon = resolveMoon();
  const moonPackage: unknown = JSON.parse(fs.readFileSync(moon.packageJson, "utf8"));
  if (
    typeof moonPackage !== "object" ||
    moonPackage === null ||
    !("version" in moonPackage) ||
    typeof moonPackage.version !== "string"
  )
    throw new Error("Moon package version is unavailable");
  const moonVersion = version(
    execFileSync(
      "node",
      ["node_modules/@moonrepo/cli/moon.js", "--version"],
      VERSION_PROBE_OPTIONS,
    ),
    "moon",
  );
  if (!moonVersion.includes(moonPackage.version))
    throw new Error("Moon package and executable versions disagree");
  return {
    schema: FINGERPRINT_SCHEMA,
    platform: process.platform,
    arch: process.arch,
    tools: {
      bun: {
        version: Bun.version,
        executableSha256: sha256(fs.readFileSync(fs.realpathSync(process.execPath))),
      },
      just: {
        version: version(execFileSync("just", ["--version"], VERSION_PROBE_OPTIONS), "just"),
        executableSha256: sha256(fs.readFileSync(just)),
      },
      node: {
        version: version(execFileSync("node", ["--version"], VERSION_PROBE_OPTIONS), "node"),
        executableSha256: sha256(fs.readFileSync(node)),
      },
      bash: {
        version: version(execFileSync("bash", ["--version"], VERSION_PROBE_OPTIONS), "bash"),
        executableSha256: sha256(fs.readFileSync(bash)),
      },
      moon: {
        version: moonVersion,
        executableSha256: sha256(fs.readFileSync(moon.executable)),
        wrapperSha256: sha256(fs.readFileSync(moon.wrapper)),
        utilsSha256: sha256(fs.readFileSync(moon.utils)),
        packageSha256: sha256(fs.readFileSync(moon.packageJson)),
      },
    },
    environment: environmentFingerprint(),
  };
}

function runtimeBundleToolFingerprintJson(): string {
  return `${JSON.stringify(collectRuntimeBundleToolFingerprint())}\n`;
}

if (import.meta.main) {
  try {
    process.stdout.write(runtimeBundleToolFingerprintJson());
  } catch {
    process.stderr.write("runtime bundle tool fingerprint resolution failed\n");
    process.exitCode = 1;
  }
}
