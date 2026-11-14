import { execFileSync } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

export type JsonValue =
  | string
  | number
  | boolean
  | null
  | JsonValue[]
  | { [key: string]: JsonValue };

export interface MermaidPairInput {
  directory: string;
  attemptDirectory?: string;
  slug: string;
  role: "official" | "candidate";
  sourceSvg: Buffer;
  renderedSvg: Buffer;
  png: Buffer;
  provenance: {
    path: string;
    sha256: string;
    document: JsonValue;
  };
  config: JsonValue;
  runtimeAssets: Array<{ path: string; sha256: string }>;
}

export interface MermaidPairManifest {
  schema: "katana-mermaid-image-pair-v1";
  diagnosticOnly: true;
  accepted: false;
  slug: string;
  role: "official" | "candidate";
  sourceSvgSha256: string;
  renderedSvgSha256: string;
  renderedSvgMatchesSource: boolean;
  pngSha256: string;
  png: { width: number; height: number };
  provenance: MermaidPairInput["provenance"];
  config: JsonValue;
  configSha256: string;
  runtimeAssets: MermaidPairInput["runtimeAssets"];
}

export function publishMermaidPair(input: MermaidPairInput): MermaidPairManifest {
  assertSafeSlug(input.slug);
  fs.mkdirSync(input.directory, { recursive: true });
  const paths = pairPaths(input.directory, input.slug);
  assertPairTargetsSafe(paths);
  validateRuntimeAssets(input.runtimeAssets);
  const widthHeight = decodePng(input.png);
  const manifest: MermaidPairManifest = {
    schema: "katana-mermaid-image-pair-v1",
    diagnosticOnly: true,
    accepted: false,
    slug: input.slug,
    role: input.role,
    sourceSvgSha256: sha256(input.sourceSvg),
    renderedSvgSha256: sha256(input.renderedSvg),
    renderedSvgMatchesSource: input.sourceSvg.equals(input.renderedSvg),
    pngSha256: sha256(input.png),
    png: widthHeight,
    provenance: input.provenance,
    config: input.config,
    configSha256: sha256(Buffer.from(JSON.stringify(input.config))),
    runtimeAssets: input.runtimeAssets,
  };
  const parent = createAttemptWorkspace(input);
  const staged = {
    svg: path.join(parent, `${input.slug}.svg`),
    renderedSvg: path.join(parent, `${input.slug}.rendered.svg`),
    png: path.join(parent, `${input.slug}.png`),
    manifest: path.join(parent, `${input.slug}.pair.json`),
  };
  const backups = new Map<string, string | null>();
  let committed = false;
  try {
    writeStagedPair(staged, input, manifest);
    verifyPngFile(staged.png, widthHeight);
    commitPair(paths, staged, parent, backups);
    verifyMermaidPair(input.directory, input.slug, input.role);
    committed = true;
    return manifest;
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    try {
      rollbackPair(paths, backups, parent);
    } catch (rollbackError) {
      const rollbackDetail =
        rollbackError instanceof Error ? rollbackError.message : String(rollbackError);
      throw new Error(
        `Mermaid pair publication failed; rollback failed and attempt is retained at ${parent}: ${detail}; ${rollbackDetail}`,
      );
    }
    throw new Error(
      `Mermaid pair publication failed; previous pair restored and attempt retained at ${parent}: ${detail}`,
    );
  } finally {
    if (committed) fs.rmSync(parent, { recursive: true, force: true });
  }
}

function createAttemptWorkspace(input: MermaidPairInput): string {
  const attemptDirectory = input.attemptDirectory ?? input.directory;
  if (!fs.existsSync(attemptDirectory) || fs.lstatSync(attemptDirectory).isSymbolicLink()) {
    throw new Error(`Mermaid pair attempt directory is missing or unsafe: ${attemptDirectory}`);
  }
  if (fs.lstatSync(input.directory).isSymbolicLink()) {
    throw new Error("Mermaid pair output and attempt paths must not use symlink aliases");
  }
  const realOutput = fs.realpathSync(input.directory);
  const realAttempt = fs.realpathSync(attemptDirectory);
  const relativeAttempt = path.relative(realOutput, realAttempt);
  if (relativeAttempt.startsWith("..") || path.isAbsolute(relativeAttempt)) {
    throw new Error(`Mermaid pair attempt directory escapes output directory: ${attemptDirectory}`);
  }
  return fs.mkdtempSync(path.join(attemptDirectory, `.publish-${input.slug}-${randomUUID()}-`));
}

function assertPairTargetsSafe(paths: ReturnType<typeof pairPaths>) {
  for (const target of Object.values(paths)) {
    if (fs.existsSync(target) && fs.lstatSync(target).isSymbolicLink()) {
      throw new Error(`Mermaid pair target must not be a symlink: ${target}`);
    }
  }
}

function writeStagedPair(
  staged: ReturnType<typeof pairPaths>,
  input: MermaidPairInput,
  manifest: MermaidPairManifest,
) {
  fs.writeFileSync(staged.svg, input.sourceSvg, { flag: "wx" });
  fs.writeFileSync(staged.renderedSvg, input.renderedSvg, { flag: "wx" });
  fs.writeFileSync(staged.png, input.png, { flag: "wx" });
  fs.writeFileSync(staged.manifest, `${JSON.stringify(manifest, null, 2)}\n`, { flag: "wx" });
}

function commitPair(
  paths: ReturnType<typeof pairPaths>,
  staged: ReturnType<typeof pairPaths>,
  attemptDir: string,
  backups: Map<string, string | null>,
) {
  for (const key of ["svg", "renderedSvg", "png"] as const) {
    backupTarget(paths[key], path.join(attemptDir, `previous-${key}`), backups);
    fs.renameSync(staged[key], paths[key]);
  }
  backupTarget(paths.manifest, path.join(attemptDir, "previous-manifest"), backups);
  fs.renameSync(staged.manifest, paths.manifest);
}

function backupTarget(target: string, backup: string, backups: Map<string, string | null>) {
  if (!fs.existsSync(target)) {
    backups.set(target, null);
    return;
  }
  fs.renameSync(target, backup);
  backups.set(target, backup);
}

function rollbackPair(
  paths: ReturnType<typeof pairPaths>,
  backups: Map<string, string | null>,
  attemptDir: string,
) {
  for (const target of Object.values(paths)) {
    if (backups.has(target) && fs.existsSync(target)) {
      const failed = path.join(attemptDir, `failed-${path.basename(target)}`);
      fs.renameSync(target, failed);
    }
  }
  for (const [target, backup] of backups) {
    if (backup !== null && fs.existsSync(backup)) fs.renameSync(backup, target);
  }
}

export function verifyMermaidPair(
  directory: string,
  slug: string,
  role: MermaidPairManifest["role"],
): MermaidPairManifest {
  assertSafeSlug(slug);
  if (fs.lstatSync(directory).isSymbolicLink()) {
    throw new Error(`Mermaid pair directory must not be a symlink: ${directory}`);
  }
  const paths = pairPaths(directory, slug);
  for (const filePath of Object.values(paths)) {
    if (!fs.existsSync(filePath) || fs.lstatSync(filePath).isSymbolicLink()) {
      throw new Error(`Mermaid pair is incomplete or unsafe for ${slug}: ${filePath}`);
    }
  }
  const manifest = parsePairManifest(fs.readFileSync(paths.manifest, "utf8"));
  if (!manifest.provenance.path.startsWith("embedded:")) {
    throw new Error(`Mermaid pair provenance must be embedded for ${slug}`);
  }
  validateRuntimeAssets(manifest.runtimeAssets);
  const svg = fs.readFileSync(paths.svg);
  const renderedSvg = fs.readFileSync(paths.renderedSvg);
  const png = fs.readFileSync(paths.png);
  if (
    manifest.schema !== "katana-mermaid-image-pair-v1" ||
    manifest.slug !== slug ||
    manifest.role !== role ||
    manifest.diagnosticOnly !== true ||
    manifest.accepted !== false ||
    manifest.sourceSvgSha256 !== sha256(svg) ||
    manifest.renderedSvgSha256 !== sha256(renderedSvg) ||
    manifest.renderedSvgMatchesSource !== svg.equals(renderedSvg) ||
    manifest.pngSha256 !== sha256(png) ||
    manifest.configSha256 !== sha256(Buffer.from(JSON.stringify(manifest.config))) ||
    manifest.provenance.sha256 !== sha256(Buffer.from(JSON.stringify(manifest.provenance.document)))
  ) {
    throw new Error(`Mermaid pair manifest does not match its files for ${slug}`);
  }
  verifyPngFile(paths.png, manifest.png);
  return manifest;
}

export function requireStrictRenderedPair(manifest: MermaidPairManifest) {
  if (!manifest.renderedSvgMatchesSource) {
    throw new Error(`Mermaid strict pair rejected ${manifest.slug}: rendered SVG bytes differ`);
  }
}

export function requireReferenceMode(manifest: MermaidPairManifest, mode: string, scope: string) {
  const config = manifest.config;
  if (
    typeof config !== "object" ||
    config === null ||
    Array.isArray(config) ||
    config.referenceMode !== mode ||
    config.referenceScope !== scope
  ) {
    throw new Error(`Mermaid pair has incompatible reference mode or scope: ${manifest.slug}`);
  }
}

export function sha256(bytes: Buffer): string {
  return createHash("sha256").update(bytes).digest("hex");
}

export function fileSha256(filePath: string): string {
  return sha256(fs.readFileSync(filePath));
}

export function parseJsonValue(serialized: string, label: string): JsonValue {
  const parsed = JSON.parse(serialized) as JsonValue;
  if (!isJsonValue(parsed)) throw new Error(`${label} is not a JSON value`);
  return parsed;
}

function parsePairManifest(serialized: string): MermaidPairManifest {
  const root = jsonRecord(parseJsonValue(serialized, "Mermaid pair manifest"), "pair manifest");
  const png = jsonRecord(requiredJsonValue(root, "png"), "pair PNG metadata");
  const provenance = jsonRecord(requiredJsonValue(root, "provenance"), "pair provenance");
  const role = stringField(root, "role");
  const schema = stringField(root, "schema");
  if (schema !== "katana-mermaid-image-pair-v1")
    throw new Error("Unsupported Mermaid pair manifest schema");
  if (role !== "official" && role !== "candidate") throw new Error("Invalid Mermaid pair role");
  if (booleanField(root, "diagnosticOnly") !== true || booleanField(root, "accepted") !== false) {
    throw new Error("Mermaid pair manifest must remain diagnostic-only");
  }
  const runtimeAssetsValue = requiredJsonValue(root, "runtimeAssets");
  if (!Array.isArray(runtimeAssetsValue)) throw new Error("Mermaid runtime asset list is invalid");
  return {
    schema,
    diagnosticOnly: true,
    accepted: false,
    slug: stringField(root, "slug"),
    role,
    sourceSvgSha256: digestField(root, "sourceSvgSha256"),
    renderedSvgSha256: digestField(root, "renderedSvgSha256"),
    renderedSvgMatchesSource: booleanField(root, "renderedSvgMatchesSource"),
    pngSha256: digestField(root, "pngSha256"),
    png: {
      width: positiveIntegerField(png, "width"),
      height: positiveIntegerField(png, "height"),
    },
    provenance: {
      path: stringField(provenance, "path"),
      sha256: digestField(provenance, "sha256"),
      document: requiredJsonValue(provenance, "document"),
    },
    config: requiredJsonValue(root, "config"),
    configSha256: digestField(root, "configSha256"),
    runtimeAssets: runtimeAssetsValue.map(parseRuntimeAsset),
  };
}

function parseRuntimeAsset(value: JsonValue): { path: string; sha256: string } {
  const asset = jsonRecord(value, "runtime asset");
  return { path: stringField(asset, "path"), sha256: digestField(asset, "sha256") };
}

function jsonRecord(value: JsonValue, label: string): { [key: string]: JsonValue } {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`Invalid ${label}: expected object`);
  }
  return value;
}

function requiredJsonValue(record: { [key: string]: JsonValue }, key: string): JsonValue {
  const value = record[key];
  if (value === undefined) throw new Error(`Mermaid pair manifest is missing ${key}`);
  return value;
}

function stringField(record: { [key: string]: JsonValue }, key: string): string {
  const value = requiredJsonValue(record, key);
  if (typeof value !== "string") throw new Error(`Mermaid pair manifest field ${key} is invalid`);
  return value;
}

function booleanField(record: { [key: string]: JsonValue }, key: string): boolean {
  const value = requiredJsonValue(record, key);
  if (typeof value !== "boolean") throw new Error(`Mermaid pair manifest field ${key} is invalid`);
  return value;
}

function digestField(record: { [key: string]: JsonValue }, key: string): string {
  const value = stringField(record, key);
  if (!/^[a-f0-9]{64}$/.test(value)) throw new Error(`Mermaid pair digest ${key} is invalid`);
  return value;
}

function positiveIntegerField(record: { [key: string]: JsonValue }, key: string): number {
  const value = requiredJsonValue(record, key);
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 1) {
    throw new Error(`Mermaid pair PNG dimension ${key} is invalid`);
  }
  return value;
}

function decodePng(bytes: Buffer): { width: number; height: number } {
  const temp = path.join(fs.mkdtempSync(path.join(os.tmpdir(), ".katana-png-check-")), "image.png");
  try {
    fs.writeFileSync(temp, bytes, { flag: "wx" });
    return identifyPng(temp);
  } finally {
    fs.rmSync(path.dirname(temp), { recursive: true, force: true });
  }
}

function verifyPngFile(filePath: string, expected: { width: number; height: number }) {
  const actual = identifyPng(filePath);
  if (actual.width !== expected.width || actual.height !== expected.height) {
    throw new Error(`Mermaid PNG dimensions changed during publication: ${filePath}`);
  }
}

function identifyPng(filePath: string): { width: number; height: number } {
  const result = execFileSync("magick", ["identify", "-format", "%m %w %h", filePath], {
    encoding: "utf8",
  }).trim();
  const match = /^PNG\s+(\d+)\s+(\d+)$/.exec(result);
  if (match === null) throw new Error(`Mermaid capture is not a decodable PNG: ${filePath}`);
  const width = Number(match[1]);
  const height = Number(match[2]);
  if (width < 1 || height < 1) throw new Error(`Mermaid PNG has invalid dimensions: ${filePath}`);
  return { width, height };
}

function pairPaths(directory: string, slug: string) {
  return {
    svg: path.join(directory, `${slug}.svg`),
    renderedSvg: path.join(directory, `${slug}.rendered.svg`),
    png: path.join(directory, `${slug}.png`),
    manifest: path.join(directory, `${slug}.pair.json`),
  };
}

function validateRuntimeAssets(assets: MermaidPairInput["runtimeAssets"]) {
  for (const asset of assets) {
    if (asset.path.trim() === "" || !/^[a-f0-9]{64}$/.test(asset.sha256)) {
      throw new Error(`Invalid Mermaid runtime asset binding: ${asset.path}`);
    }
    const sourcePath = path.resolve(asset.path);
    if (
      fs.lstatSync(sourcePath).isSymbolicLink() ||
      !fs.statSync(sourcePath).isFile() ||
      fileSha256(sourcePath) !== asset.sha256
    ) {
      throw new Error(`Mermaid runtime asset hash changed: ${sourcePath}`);
    }
  }
}

function assertSafeSlug(slug: string) {
  if (!/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(slug) || slug === "." || slug === "..") {
    throw new Error(`Unsafe Mermaid fixture slug: ${slug}`);
  }
}

function isJsonValue(value: JsonValue): boolean {
  if (
    value === null ||
    typeof value === "string" ||
    typeof value === "boolean" ||
    (typeof value === "number" && Number.isFinite(value))
  ) {
    return true;
  }
  if (Array.isArray(value)) return value.every(isJsonValue);
  if (typeof value !== "object") return false;
  return Object.values(value).every(isJsonValue);
}
