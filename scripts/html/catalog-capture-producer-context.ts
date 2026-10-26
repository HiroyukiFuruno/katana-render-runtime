import { execFileSync } from "node:child_process";
import fs from "node:fs";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import { fileSha256 } from "../mermaid/mermaid-pair-manifest";
import {
  canonicalRegularFile,
  type FileRecord,
  parseSourceManifest,
  verifyFileRecord,
} from "../mermaid/native-build-receipt";
import { captureNativeSourceManifest } from "../mermaid/native-source-manifest";
import {
  collectRuntimeBundleToolFingerprint,
  type RuntimeBundleToolFingerprint,
} from "../runtime-bundles/runtime-bundle-tool-fingerprint";

export interface CatalogCaptureProducerContext {
  schema: "krr-html84-capture-producer-context-v1";
  role: "before" | "after";
  sourceManifest: { path: string; sha256: string; digest: string };
  tools: RuntimeBundleToolFingerprint;
  playwrightFiles: FileRecord[];
  os: { release: string; version: string };
  fontEnvironment?: FontEnvironment;
}

export interface FontEnvironment {
  schema: "krr-font-environment-v1";
  cacheable: boolean;
  platform: string;
  inventorySha256: string;
  recordCount: number;
  missReasons: string[];
  pythonVersion: string;
  pythonExecutableSha256?: string;
}

export function captureCatalogProducerContext(
  outputDirectory: string,
  role: "before" | "after",
  repositoryRoot = path.resolve("."),
): CatalogCaptureProducerContext {
  const source = captureNativeSourceManifest(repositoryRoot);
  const sourcePath = path.join(outputDirectory, `catalog-source-manifest-${role}.json`);
  const sourceBytes = Buffer.from(`${JSON.stringify(source)}\n`);
  fs.writeFileSync(sourcePath, sourceBytes, { flag: "wx" });
  const sourceSha256 = fileSha256(sourcePath);
  const playwrightFiles = capturePlaywrightFiles();
  const fontEnvironment = captureFontEnvironment(repositoryRoot);
  return {
    schema: "krr-html84-capture-producer-context-v1",
    role,
    sourceManifest: { path: sourcePath, sha256: sourceSha256, digest: source.digest },
    tools: collectRuntimeBundleToolFingerprint(),
    playwrightFiles,
    os: { release: os.release(), version: os.version() },
    fontEnvironment,
  };
}

export function captureFontEnvironment(repositoryRoot: string): FontEnvironment {
  const output = execFileSync(
    "python3",
    ["scripts/release/native_coverage_result_cli.py", "--repo", repositoryRoot, "font-context"],
    { cwd: repositoryRoot, encoding: "utf8" },
  );
  const value: unknown = JSON.parse(output);
  if (!isFontEnvironment(value)) throw new Error("invalid font environment context");
  return value;
}

export function assertCatalogProducerContext(value: unknown): CatalogCaptureProducerContext {
  if (!isContext(value)) throw new Error("invalid catalog capture producer context");
  canonicalRegularFile(value.sourceManifest.path, "catalog source manifest");
  if (fileSha256(value.sourceManifest.path) !== value.sourceManifest.sha256)
    throw new Error("catalog source manifest SHA differs");
  const parsed = parseSourceManifest(value.sourceManifest.path);
  if (parsed.digest !== value.sourceManifest.digest)
    throw new Error("catalog source manifest identity differs");
  for (const file of value.playwrightFiles) verifyFileRecord(file, "Playwright file");
  return value;
}

export function sameCatalogProducerContext(leftValue: unknown, rightValue: unknown): boolean {
  const left = assertCatalogProducerContext(leftValue);
  const right = assertCatalogProducerContext(rightValue);
  return (
    left.sourceManifest.sha256 === right.sourceManifest.sha256 &&
    left.sourceManifest.digest === right.sourceManifest.digest &&
    JSON.stringify(left.tools) === JSON.stringify(right.tools) &&
    JSON.stringify(left.playwrightFiles) === JSON.stringify(right.playwrightFiles) &&
    JSON.stringify(left.os) === JSON.stringify(right.os) &&
    JSON.stringify(left.fontEnvironment) === JSON.stringify(right.fontEnvironment)
  );
}

function capturePlaywrightFiles(): FileRecord[] {
  const require = createRequire(import.meta.url);
  const roots = ["playwright", "playwright-core"].map((name) =>
    packageRoot(path.dirname(require.resolve(name)), name),
  );
  const files = new Map<string, FileRecord>();
  const directories = new Set<string>();
  for (const root of roots) collectDirectory(fs.realpathSync(root), directories, files);
  return [...files.values()].sort((left, right) => left.path.localeCompare(right.path));
}

function packageRoot(start: string, name: string): string {
  let current = start;
  while (true) {
    if (fs.existsSync(path.join(current, "package.json"))) return fs.realpathSync(current);
    const parent = path.dirname(current);
    if (parent === current) throw new Error(`${name} package root is unavailable`);
    current = parent;
  }
}

function collectDirectory(
  directory: string,
  directories: Set<string>,
  files: Map<string, FileRecord>,
): void {
  if (directories.has(directory)) return;
  directories.add(directory);
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const resolved = fs.realpathSync(path.join(directory, entry.name));
    if (fs.statSync(resolved).isDirectory()) collectDirectory(resolved, directories, files);
    else {
      const record: FileRecord = {
        path: canonicalRegularFile(resolved, "Playwright file"),
        sha256: fileSha256(resolved),
        size: fs.statSync(resolved).size,
      };
      verifyFileRecord(record, "Playwright file");
      files.set(record.path, record);
    }
  }
}

function isContext(value: unknown): value is CatalogCaptureProducerContext {
  if (typeof value !== "object" || value === null) return false;
  const record = value as Record<string, unknown>;
  const source = record.sourceManifest as Record<string, unknown> | null;
  const osInfo = record.os as Record<string, unknown> | null;
  return (
    record.schema === "krr-html84-capture-producer-context-v1" &&
    (record.role === "before" || record.role === "after") &&
    typeof record.sourceManifest === "object" &&
    source !== null &&
    typeof source?.path === "string" &&
    /^[0-9a-f]{64}$/.test(String(source.sha256)) &&
    /^[0-9a-f]{64}$/.test(String(source.digest)) &&
    isTools(record.tools) &&
    Array.isArray(record.playwrightFiles) &&
    record.playwrightFiles.every(isFileRecord) &&
    osInfo !== null &&
    typeof osInfo?.release === "string" &&
    typeof osInfo?.version === "string" &&
    isFontEnvironment(record.fontEnvironment)
  );
}

export function isFontEnvironment(value: unknown): value is FontEnvironment {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const environment = value as Record<string, unknown>;
  return (
    environment.schema === "krr-font-environment-v1" &&
    typeof environment.cacheable === "boolean" &&
    typeof environment.platform === "string" &&
    (environment.cacheable
      ? /^[0-9a-f]{64}$/.test(String(environment.inventorySha256))
      : environment.inventorySha256 === "unavailable" ||
        /^[0-9a-f]{64}$/.test(String(environment.inventorySha256))) &&
    typeof environment.recordCount === "number" &&
    Number.isSafeInteger(environment.recordCount) &&
    environment.recordCount >= 0 &&
    Array.isArray(environment.missReasons) &&
    environment.missReasons.every((reason) => typeof reason === "string") &&
    typeof environment.pythonVersion === "string" &&
    (environment.cacheable
      ? environment.recordCount > 0 &&
        environment.missReasons.length === 0 &&
        /^[0-9a-f]{64}$/.test(String(environment.pythonExecutableSha256))
      : environment.pythonExecutableSha256 === undefined ||
        /^[0-9a-f]{64}$/.test(String(environment.pythonExecutableSha256)))
  );
}

function isFileRecord(value: unknown): value is FileRecord {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const record = value as Record<string, unknown>;
  return (
    typeof record.path === "string" &&
    /^[0-9a-f]{64}$/.test(String(record.sha256)) &&
    typeof record.size === "number" &&
    Number.isSafeInteger(record.size) &&
    record.size >= 0
  );
}

function isTools(value: unknown): value is RuntimeBundleToolFingerprint {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const tools = value as Record<string, unknown>;
  if (
    tools.schema !== "krr-runtime-bundle-tool-fingerprint-v1" ||
    typeof tools.platform !== "string" ||
    typeof tools.arch !== "string" ||
    !Array.isArray(tools.environment)
  )
    return false;
  const identities = tools.tools;
  if (typeof identities !== "object" || identities === null || Array.isArray(identities))
    return false;
  const identityRecord = identities as Record<string, unknown>;
  if (
    !isToolIdentity(identityRecord.bun) ||
    !isToolIdentity(identityRecord.just) ||
    !isToolIdentity(identityRecord.node) ||
    !isToolIdentity(identityRecord.bash) ||
    !isToolIdentity(identityRecord.moon, ["wrapperSha256", "utilsSha256", "packageSha256"])
  )
    return false;
  return tools.environment.every((entry) => {
    if (typeof entry !== "object" || entry === null || Array.isArray(entry)) return false;
    const environment = entry as Record<string, unknown>;
    return (
      typeof environment.name === "string" && /^[0-9a-f]{64}$/.test(String(environment.valueSha256))
    );
  });
}

function isToolIdentity(value: unknown, extraKeys: string[] = []): boolean {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const identity = value as Record<string, unknown>;
  return (
    typeof identity.version === "string" &&
    /^[0-9a-f]{64}$/.test(String(identity.executableSha256)) &&
    extraKeys.every((key) => /^[0-9a-f]{64}$/.test(String(identity[key])))
  );
}
