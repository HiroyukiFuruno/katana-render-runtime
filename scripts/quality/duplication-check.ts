import { execFileSync } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import {
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import { gunzipSync, gzipSync, constants as zlibConstants } from "node:zlib";

import {
  type AssetEntry,
  type AssetManifest,
  assetManifestSchema,
  type Chunk,
  type Duplicate,
  type Exception,
  type ExceptionSide,
  type Policy,
  policySchema,
  type Report,
  reportSchema,
} from "./duplication-schema";
import { collectOwnedSourcePaths, OWNED_SOURCE_FORMATS } from "./owned-source-inventory";
export function sha256(value: Uint8Array | string): string {
  return createHash("sha256").update(value).digest("hex");
}

function fail(message: string): never {
  throw new Error(message);
}

function readPolicy(file: string): Policy {
  return policySchema.parse(JSON.parse(readFileSync(file, "utf8")));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function sameStrings(value: string[], expected: string[]): boolean {
  return value.length === expected.length && value.every((item, index) => item === expected[index]);
}

export function verifiedThirdPartyJavascript(
  repo: string,
  sourcePaths: string[],
  manifestPath: string,
  resourcesRoot: string,
  sourceKinds: string[],
): Map<string, string> {
  const expectedManifestPath = `${resourcesRoot}/drawio-resource-manifest.json`;
  if (manifestPath !== expectedManifestPath)
    fail("Draw.io asset manifest path is not the pinned repository manifest");
  const manifest = readAssetManifest(repo, manifestPath, resourcesRoot);
  const absoluteRoot = path.resolve(repo, resourcesRoot);
  const sourceSet = new Set(sourcePaths);
  const excluded = new Map<string, string>();
  for (const entry of manifest.resources) {
    if (
      !sourceKinds.includes(entry.sourceKind) ||
      OWNED_SOURCE_FORMATS[path.extname(entry.logicalPath)] !== "javascript"
    )
      continue;
    const identity = verifyManifestJavascript(absoluteRoot, sourceSet, entry);
    excluded.set(identity.path, identity.sha256);
  }
  return excluded;
}

function readAssetManifest(
  repo: string,
  manifestPath: string,
  resourcesRoot: string,
): AssetManifest {
  const manifest = assetManifestSchema.parse(
    JSON.parse(readFileSync(path.join(repo, manifestPath), "utf8")),
  );
  if (manifest.resourcesRoot !== resourcesRoot)
    fail("invalid Draw.io resource manifest root or schema");
  const { resources } = manifest;
  const logicalPaths = resources.map((entry) => entry.logicalPath);
  if (new Set(logicalPaths).size !== logicalPaths.length)
    fail("duplicate Draw.io resource logicalPath");
  for (const entry of resources) {
    if (sourceKindForPath(entry.logicalPath) !== entry.sourceKind) {
      fail(`Draw.io resource sourceKind does not match logicalPath: ${entry.logicalPath}`);
    }
  }
  return manifest;
}

function verifyManifestJavascript(
  absoluteRoot: string,
  sourceSet: Set<string>,
  entry: AssetEntry,
): { path: string; sha256: string } {
  const absolute = path.resolve(absoluteRoot, entry.logicalPath);
  const relative = path.relative(absoluteRoot, absolute);
  if (relative === ".." || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) {
    fail("Draw.io resource logicalPath escapes resourcesRoot");
  }
  if (!sourceSet.has(absolute))
    fail(`manifested third-party JavaScript is missing from source roots: ${entry.logicalPath}`);
  if (lstatSync(absolute).isSymbolicLink() || !lstatSync(absolute).isFile()) {
    fail(`manifested third-party JavaScript is not a regular file: ${entry.logicalPath}`);
  }
  if (
    statSync(absolute).size !== entry.byteSize ||
    sha256(readFileSync(absolute)) !== entry.sha256
  ) {
    fail(`Draw.io asset manifest hash/size mismatch: ${entry.logicalPath}`);
  }
  return { path: absolute, sha256: entry.sha256 };
}

function sourceKindForPath(logicalPath: string): string {
  const firstSegment = logicalPath.split("/")[0] ?? "";
  if (firstSegment === "assets" || firstSegment === "img") return "drawio-webapp";
  if (firstSegment === "shapes") return "drawio-shapes";
  if (firstSegment === "stencils") return "drawio-stencils";
  return firstSegment;
}

function filterVerifiedAssets(
  sourcePaths: string[],
  excludedAssets: Map<string, string>,
): string[] {
  return sourcePaths.filter((file) => !excludedAssets.has(file));
}

function sourceHashes(paths: string[]): Map<string, string> {
  return new Map(paths.map((file) => [file, sha256(readFileSync(file))]));
}

function safeReportPath(repo: string, file: string): string {
  if (!path.isAbsolute(file)) fail(`engine report path is not absolute: ${file}`);
  const resolved = path.resolve(file);
  const relative = path.relative(repo, resolved);
  if (relative === ".." || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) {
    fail(`engine report path escapes repository: ${file}`);
  }
  return resolved;
}

function cloneKey(side: ExceptionSide): string {
  return `${side.path}:${side.start.line}:${side.start.column}-${side.end.line}:${side.end.column}`;
}

function pairKey(first: ExceptionSide, second: ExceptionSide): string {
  return [cloneKey(first), cloneKey(second)].sort().join(" | ");
}

function sideFromChunk(repo: string, chunk: Chunk, hashes: Map<string, string>): ExceptionSide {
  const absolute = safeReportPath(repo, chunk.name);
  const relative = path.relative(repo, absolute).split(path.sep).join("/");
  const digest = hashes.get(absolute);
  if (digest === undefined)
    fail(`engine reported a file outside the scanned source set: ${chunk.name}`);
  return { path: relative, sha256: digest, start: chunk.startLoc, end: chunk.endLoc };
}

export function validateReport(
  repo: string,
  paths: string[],
  before: Map<string, string>,
  after: Map<string, string>,
  report: unknown,
  policy: Policy,
): number {
  const checked = validateSourceStatistics(paths, before, after, report, policy);
  const hashes = new Map(paths.map((file) => [file, before.get(file) ?? ""]));
  const seen = new Map<string, number>();
  let unresolved = 0;
  for (const duplicate of checked.duplicates) {
    const matched = matchingException(repo, duplicate, hashes, policy.exceptions);
    const key = pairKey(matched.first, matched.second);
    if (matched.approved) seen.set(key, (seen.get(key) ?? 0) + 1);
    else unresolved += 1;
  }
  validateExceptionCoverage(repo, policy.exceptions, hashes, seen);
  return unresolved;
}

function checkedReport(value: unknown, minLines: number, minTokens: number): Report {
  const parsed = reportSchema.safeParse(value);
  if (!parsed.success) {
    const label = parsed.error.issues.some((issue) => issue.path[0] === "duplicates")
      ? "invalid jscpd duplicate record"
      : "invalid jscpd JSON report";
    fail(`${label}: ${parsed.error.message}`);
  }
  for (const duplicate of parsed.data.duplicates) {
    if (duplicate.lines < minLines || duplicate.tokens < minTokens)
      fail("invalid jscpd duplicate record");
  }
  return parsed.data;
}

function validateSourceStatistics(
  paths: string[],
  before: Map<string, string>,
  after: Map<string, string>,
  report: unknown,
  policy: Policy,
): Report {
  const checked = checkedReport(report, policy.minLines, policy.minTokens);
  const expectedByFormat = countAndVerifySources(paths, before, after);
  validateStatistics(
    checked.statistics,
    paths.length,
    checked.duplicates.length,
    expectedByFormat,
    policy,
    false,
  );
  return checked;
}

export function validateInventoryReport(
  paths: string[],
  report: unknown,
  policy: Policy,
  sourceSnapshot?: { paths: string[]; hashes: Map<string, string>; excludedPaths?: string[] },
): void {
  const checked = checkedReport(report, 1, 1);
  const emptySources =
    sourceSnapshot === undefined ? new Set<string>() : verifiedEmptySources(paths, sourceSnapshot);
  const counts = new Map<string, number>();
  for (const file of paths) {
    const format = OWNED_SOURCE_FORMATS[path.extname(file)];
    if (format === undefined) fail(`unsupported source extension: ${file}`);
    if (!emptySources.has(file)) counts.set(format, (counts.get(format) ?? 0) + 1);
  }
  validateStatistics(
    checked.statistics,
    paths.length - emptySources.size,
    checked.duplicates.length,
    counts,
    policy,
    true,
  );
}

function verifiedEmptySources(
  paths: string[],
  sourceSnapshot: { paths: string[]; hashes: Map<string, string>; excludedPaths?: string[] },
): Set<string> {
  const actualPaths = [...sourceSnapshot.paths].sort();
  const hashPaths = [...sourceSnapshot.hashes.keys()].sort();
  const scannedPaths = [...paths].sort();
  const excludedPaths = [...(sourceSnapshot.excludedPaths ?? [])].sort();
  const accountedPaths = [...scannedPaths, ...excludedPaths].sort();
  if (
    new Set(actualPaths).size !== actualPaths.length ||
    new Set(hashPaths).size !== hashPaths.length ||
    new Set(scannedPaths).size !== scannedPaths.length ||
    new Set(excludedPaths).size !== excludedPaths.length ||
    scannedPaths.some((file) => excludedPaths.includes(file)) ||
    actualPaths.length !== hashPaths.length ||
    actualPaths.some((file, index) => file !== hashPaths[index]) ||
    actualPaths.length !== accountedPaths.length ||
    actualPaths.some((file, index) => file !== accountedPaths[index])
  ) {
    fail("source snapshot paths do not exactly match source hashes");
  }
  const scannedPathSet = new Set(scannedPaths);

  const emptySources = new Set<string>();
  for (const file of actualPaths) {
    const bytes = readFileSync(file);
    const digest = sha256(bytes);
    if (sourceSnapshot.hashes.get(file) !== digest)
      fail(`source changed before duplication probe validation: ${file}`);
    if (scannedPathSet.has(file) && statSync(file).size === 0) {
      if (bytes.length !== 0 || digest !== sha256(new Uint8Array()))
        fail(`invalid empty source identity: ${file}`);
      emptySources.add(file);
    }
  }
  return emptySources;
}

function countAndVerifySources(
  paths: string[],
  before: Map<string, string>,
  after: Map<string, string>,
): Map<string, number> {
  const expectedByFormat = new Map<string, number>();
  for (const file of paths) {
    const format = OWNED_SOURCE_FORMATS[path.extname(file)];
    if (format === undefined) fail(`unsupported source extension: ${file}`);
    expectedByFormat.set(format, (expectedByFormat.get(format) ?? 0) + 1);
    if (before.get(file) !== after.get(file))
      fail(`source changed during duplication scan: ${file}`);
  }
  return expectedByFormat;
}

function validateStatistics(
  stats: Report["statistics"],
  expectedSources: number,
  expectedClones: number,
  expectedByFormat: Map<string, number>,
  policy: Policy,
  exactSourceCount: boolean,
): void {
  if (
    exactSourceCount
      ? stats.total.sources !== expectedSources
      : stats.total.sources > expectedSources
  )
    fail("jscpd source count does not match scanned roots");
  if (stats.total.clones !== expectedClones)
    fail("jscpd clone count does not match duplicate records");
  let formatSourceTotal = 0;
  let formatCloneTotal = 0;
  for (const format of policy.formats) {
    const counts = stats.formats[format];
    const expected = expectedByFormat.get(format) ?? 0;
    if (exactSourceCount ? counts.sources !== expected : counts.sources > expected)
      fail(`jscpd ${format} source count does not match scanned roots`);
    formatSourceTotal += counts.sources;
    formatCloneTotal += counts.clones;
  }
  if (formatSourceTotal !== stats.total.sources)
    fail("jscpd total and per-format source counts disagree");
  if (formatCloneTotal !== stats.total.clones)
    fail("jscpd total and per-format clone counts disagree");
}

function matchingException(
  repo: string,
  value: Duplicate,
  hashes: Map<string, string>,
  exceptions: Exception[],
): { first: ExceptionSide; second: ExceptionSide; approved: boolean } {
  const first = sideFromChunk(repo, value.firstFile, hashes);
  const second = sideFromChunk(repo, value.secondFile, hashes);
  const key = pairKey(first, second);
  const identity = [sourceIdentity(first), sourceIdentity(second)].sort().join(" | ");
  const matches = exceptions.filter(
    (exception) =>
      pairKey(exception.first, exception.second) === key &&
      exception.fragmentSha256 === sha256(value.fragment) &&
      [sourceIdentity(exception.first), sourceIdentity(exception.second)].sort().join(" | ") ===
        identity,
  );
  if (matches.length > 1) fail("duplicate policy exceptions match the same clone pair");
  return { first, second, approved: matches.length === 1 };
}

function sourceIdentity(side: ExceptionSide): string {
  return `${cloneKey(side)}:${side.sha256}`;
}

function validateExceptionCoverage(
  repo: string,
  exceptions: Exception[],
  hashes: Map<string, string>,
  seen: Map<string, number>,
): void {
  for (const exception of exceptions) {
    const firstFile = path.resolve(repo, exception.first.path);
    const secondFile = path.resolve(repo, exception.second.path);
    if (!hashes.has(firstFile) || !hashes.has(secondFile))
      fail("exception path is outside current source roots");
    if (
      hashes.get(firstFile) !== exception.first.sha256 ||
      hashes.get(secondFile) !== exception.second.sha256
    ) {
      fail("stale exception source SHA");
    }
    if ((seen.get(pairKey(exception.first, exception.second)) ?? 0) !== 1)
      fail("unmatched or ambiguous duplication exception");
  }
}

export function assertToolSucceeded(status: number | null): void {
  if (status !== 0) fail(`jscpd execution failed with status ${String(status)}`);
}

export function assertNoUnapprovedClones(count: number): void {
  if (!Number.isInteger(count) || count < 0) fail("invalid unapproved clone count");
  if (count > 0) fail(`duplication gate found ${count} unapproved clone pair(s)`);
}

function main(): void {
  const repo = path.resolve(import.meta.dir, "../..");
  const policy = readPolicy(path.join(import.meta.dir, "duplication-policy.json"));
  const allPaths = collectOwnedSourcePaths(repo, policy);
  const excludedAssets = verifiedThirdPartyJavascript(
    repo,
    allPaths,
    policy.assetManifest,
    policy.assetResourcesRoot,
    policy.assetSourceKinds,
  );
  const paths = filterVerifiedAssets(allPaths, excludedAssets);
  const before = sourceHashes(allPaths);
  for (const [file, expectedSha] of excludedAssets) {
    if (before.get(file) !== expectedSha) fail("Draw.io asset changed after manifest verification");
  }
  const output = path.join(repo, "tmp/quality/duplication");
  mkdirSync(output, { recursive: true });
  runAndValidateInventoryProbe(repo, output, policy, excludedAssets, paths, {
    paths: allPaths,
    hashes: before,
    excludedPaths: [...excludedAssets.keys()],
  });
  assertCurrentSourceSet(repo, policy, allPaths, excludedAssets);
  const afterInventory = sourceHashes(allPaths);
  assertStableSources(allPaths, before, afterInventory);
  const finalReportPath = runJscpd(
    repo,
    output,
    policy,
    excludedAssets,
    policy.minLines,
    policy.minTokens,
    "final",
  );
  const report: unknown = readCurrentReport(finalReportPath);
  assertCurrentSourceSet(repo, policy, allPaths, excludedAssets);
  const after = sourceHashes(allPaths);
  assertStableSources(allPaths, before, after);
  const unresolved = validateReport(repo, paths, before, after, report, policy);
  assertNoUnapprovedClones(unresolved);
  console.log(
    `duplication gate passed: ${paths.length} sources, ${unresolved} unapproved clone pairs`,
  );
}

function assertCurrentSourceSet(
  repo: string,
  policy: Policy,
  expectedPaths: string[],
  expectedAssets: Map<string, string>,
): void {
  const currentPaths = collectOwnedSourcePaths(repo, policy);
  if (!sameStrings(currentPaths, expectedPaths)) fail("source set changed during duplication scan");
  const currentAssets = verifiedThirdPartyJavascript(
    repo,
    currentPaths,
    policy.assetManifest,
    policy.assetResourcesRoot,
    policy.assetSourceKinds,
  );
  if (
    currentAssets.size !== expectedAssets.size ||
    [...currentAssets].some(([file, digest]) => expectedAssets.get(file) !== digest)
  ) {
    fail("Draw.io asset manifest boundary changed during duplication scan");
  }
}

function assertStableSources(
  paths: string[],
  before: Map<string, string>,
  after: Map<string, string>,
): void {
  for (const file of paths) {
    if (before.get(file) !== after.get(file))
      fail(`source changed during duplication scan: ${file}`);
  }
}

export function readCurrentReport(reportPath: string): unknown {
  if (!lstatSync(reportPath).isFile()) fail("jscpd did not produce report.json");
  const bytes = readFileSync(reportPath);
  const compressed = reportPath.endsWith(".json.gz");
  if (compressed) {
    const filenameDigest = path.basename(reportPath).match(/^([a-f0-9]{64})\.json\.gz$/)?.[1];
    if (filenameDigest === undefined || sha256(bytes) !== filenameDigest) {
      fail("content-addressed compressed report hash mismatch");
    }
  }
  const reportBytes = compressed ? gunzipSync(bytes) : bytes;
  return JSON.parse(reportBytes.toString("utf8"));
}

export function runJscpd(
  repo: string,
  output: string,
  policy: Policy,
  excludedAssets: Map<string, string>,
  minLines: number,
  minTokens: number,
  role: "probe" | "final" | "latest" = "latest",
): string {
  mkdirSync(output, { recursive: true });
  let engineOutput: string | undefined;
  let config: { directory: string; file: string } | undefined;
  try {
    engineOutput = mkdtempSync(path.join(os.tmpdir(), "krr-jscpd-output-"));
    config = canonicalConfigFile(policy);
    const args = [
      "--from",
      "jscpd==5.4.1",
      "jscpd",
      "--config",
      config.file,
      "--absolute",
      "--format",
      policy.formats.join(","),
      "--mode",
      policy.mode,
      "--min-lines",
      String(minLines),
      "--min-tokens",
      String(minTokens),
      "--threshold",
      "100",
      "--reporters",
      "json",
      "--report-name",
      "report",
      "--output",
      engineOutput,
      "--ignore",
      ignorePatterns(repo, policy, excludedAssets),
      ...policy.roots.map((root) => path.join(repo, root)),
    ];
    try {
      execFileSync("uvx", args, { cwd: repo, stdio: "inherit" });
    } catch (error) {
      assertToolSucceeded(
        isRecord(error) && typeof error.status === "number" ? error.status : null,
      );
    }
    const reportPath = path.join(engineOutput, "report.json");
    if (!lstatSync(reportPath).isFile()) fail("jscpd did not produce report.json");
    const digest = storeJscpdReport(output, readFileSync(reportPath), role);
    return path.join(output, "reports", `${digest}.json.gz`);
  } finally {
    if (config !== undefined) rmSync(config.directory, { recursive: true, force: true });
    if (engineOutput !== undefined) rmSync(engineOutput, { recursive: true, force: true });
  }
}

export function runAndValidateInventoryProbe(
  repo: string,
  output: string,
  policy: Policy,
  excludedAssets: Map<string, string>,
  paths: string[],
  sourceSnapshot?: { paths: string[]; hashes: Map<string, string>; excludedPaths?: string[] },
): string {
  const reportPath = runJscpd(repo, output, policy, excludedAssets, 1, 1, "probe");
  validateInventoryReport(paths, readCurrentReport(reportPath), policy, sourceSnapshot);
  return reportPath;
}

export function storeJscpdReport(
  output: string,
  report: Uint8Array,
  role: "probe" | "final" | "latest",
): string {
  mkdirSync(output, { recursive: true });
  const contentSha256 = sha256(report);
  const compressed = gzipSync(report, { level: zlibConstants.Z_BEST_SPEED });
  const digest = sha256(compressed);
  const reportsDirectory = path.join(output, "reports");
  mkdirSync(reportsDirectory, { recursive: true });
  const archivedReport = path.join(reportsDirectory, `${digest}.json.gz`);
  if (lstatMaybeFile(archivedReport)) {
    const previous = readFileSync(archivedReport);
    let previousReport: Buffer;
    try {
      previousReport = gunzipSync(previous);
    } catch {
      fail(`content-addressed compressed report is invalid: ${archivedReport}`);
    }
    if (
      sha256(previous) !== digest ||
      !previous.equals(compressed) ||
      sha256(previousReport) !== contentSha256
    ) {
      fail(`content-addressed compressed report hash mismatch: ${archivedReport}`);
    }
  } else {
    writeAtomically(archivedReport, compressed);
  }
  const roleRecord = Buffer.from(
    `${JSON.stringify(
      {
        role,
        sha256: digest,
        contentSha256,
        encoding: "gzip",
        report: `reports/${digest}.json.gz`,
      },
      null,
      2,
    )}\n`,
  );
  writeAtomically(path.join(reportsDirectory, `${role}-latest.json`), roleRecord);
  if (role === "final" || role === "latest")
    writeAtomically(path.join(output, "report.json"), report);
  return digest;
}

function lstatMaybeFile(file: string): boolean {
  try {
    return lstatSync(file).isFile();
  } catch (error) {
    if (isRecord(error) && error.code === "ENOENT") return false;
    throw error;
  }
}

function writeAtomically(destination: string, data: Uint8Array): void {
  const temporary = path.join(
    path.dirname(destination),
    `.${path.basename(destination)}.${process.pid}.${randomUUID()}.tmp`,
  );
  try {
    writeFileSync(temporary, data, { flag: "wx" });
    renameSync(temporary, destination);
  } catch (error) {
    try {
      unlinkSync(temporary);
    } catch (cleanupError) {
      if (!isRecord(cleanupError) || cleanupError.code !== "ENOENT") throw cleanupError;
    }
    throw error;
  }
}

function canonicalConfigFile(policy: Policy): { directory: string; file: string } {
  const directory = mkdtempSync(path.join(os.tmpdir(), "krr-jscpd-config-"));
  const file = path.join(directory, "jscpd.json");
  try {
    writeFileSync(file, JSON.stringify({ skipLocal: false, skipIsolated: [] }, null, 2), {
      flag: "wx",
    });
    if (policy.engine !== "jscpd@5.4.1") fail("unsupported jscpd engine pin");
    return { directory, file };
  } catch (error) {
    rmSync(directory, { recursive: true, force: true });
    throw error;
  }
}

function ignorePatterns(repo: string, policy: Policy, excludedAssets: Map<string, string>): string {
  const patterns = ["**/generated/**", "**/vendor/**", "**/target/**", "**/node_modules/**"];
  for (const file of excludedAssets.keys()) {
    const relative = path.relative(repo, file).split(path.sep).join("/");
    if (/[,*?{}[\]]/.test(relative))
      fail(`unsupported glob character in manifest resource path: ${relative}`);
    patterns.push(`**/${relative}`);
  }
  if (!sameStrings(policy.excludes, ["generated", "vendor", "target", "node_modules"])) {
    fail("source exclusion policy changed unexpectedly");
  }
  return patterns.join(",");
}

type CliArguments = { kind: "run" } | { kind: "help" } | { kind: "invalid"; arguments: string[] };

export function resolveCliArguments(args: readonly string[]): CliArguments {
  if (args.length === 0) return { kind: "run" };
  if (args.length === 1 && args[0] === "--help") return { kind: "help" };
  return { kind: "invalid", arguments: [...args] };
}

const usage = "Usage: bun scripts/quality/duplication-check.ts [--help]";

if (import.meta.main) {
  const cli = resolveCliArguments(process.argv.slice(2));
  if (cli.kind === "help") {
    console.log(usage);
  } else if (cli.kind === "invalid") {
    console.error(`Unknown or unsupported arguments: ${cli.arguments.join(" ")}\n${usage}`);
    process.exitCode = 2;
  } else {
    try {
      main();
    } catch (error) {
      console.error(error instanceof Error ? error.message : String(error));
      process.exitCode = 1;
    }
  }
}
