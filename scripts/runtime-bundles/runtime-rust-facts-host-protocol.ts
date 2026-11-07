import { randomUUID } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";
import { z } from "zod";
import { sha256 } from "../mermaid/mermaid-pair-manifest";
import { canonicalRegularFile, type FileRecord } from "../mermaid/native-build-receipt";

export const hostFileRecordSchema = z.strictObject({
  path: z.string().min(1),
  sha256: z.string().regex(/^[a-f0-9]{64}$/u),
  size: z.number().int().nonnegative(),
});
export const hostBinaryRecordSchema = hostFileRecordSchema.extend({
  mode: z.number().int().nonnegative(),
});
export const hostBuildOutputRecordSchema = hostFileRecordSchema.extend({
  device: z.string().regex(/^\d+$/u),
  inode: z.string().regex(/^\d+$/u),
  mode: z.number().int().nonnegative(),
  mtimeNs: z.string().regex(/^\d+$/u),
  ctimeNs: z.string().regex(/^\d+$/u),
});
const hostToolSchema = z.strictObject({
  launcher: hostFileRecordSchema,
  executable: hostFileRecordSchema,
});
const hostJustToolSchema = hostToolSchema.extend({ invocationPath: z.string().optional() });
export const hostInputsSchema = z.strictObject({
  files: z.array(hostFileRecordSchema).min(1),
  tools: z.strictObject({
    just: hostJustToolSchema,
    cargo: hostToolSchema,
    rustc: hostToolSchema,
    python3: hostToolSchema,
    bun: hostToolSchema,
    rtk: hostToolSchema.nullable(),
  }),
  environmentSha256: z.string().regex(/^[a-f0-9]{64}$/u),
});
export const hostProcessSchema = z.strictObject({
  argv: z.array(z.string()).min(1),
  cwd: z.string(),
  exitCode: z.number().int().nullable(),
  signal: z.union([z.string(), z.number().int().positive()]).nullable(),
  error: z.string().nullable(),
  stdout: hostFileRecordSchema,
  stderr: hostFileRecordSchema,
});
export type RustFactsHostMode = "mathjax" | "native-width" | "classic" | "macho-noso";

export function rustFactsHostBinaryPath(root: string): string {
  return (
    path.join(root, "target/debug/mathjax_consumer_facts") +
    (process.platform === "win32" ? ".exe" : "")
  );
}

export function rustFactsHostArtifactPath(
  root: string,
  receipt: string,
  attempt: string,
  digest: string,
): string {
  if (
    !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/u.test(attempt) ||
    !/^[a-f0-9]{64}$/u.test(digest) ||
    !path.isAbsolute(root) ||
    fs.realpathSync(root) !== root ||
    path.resolve(receipt) !== receipt ||
    !receipt.startsWith(`${path.join(root, "tmp/quality/rust-facts-host")}${path.sep}`)
  )
    throw new Error("unsafe Rust facts host artifact identity");
  const executable = rustFactsHostArtifactName(digest);
  return path.join(path.dirname(receipt), `attempt-${attempt}`, executable);
}

export function rustFactsHostArtifactName(digest: string, platform = process.platform): string {
  if (!/^[a-f0-9]{64}$/u.test(digest)) throw new Error("invalid Rust facts host artifact digest");
  return `mathjax_consumer_facts-${digest}${platform === "win32" ? ".exe" : ""}`;
}

export function hostFileRecord(file: string): FileRecord {
  const label = `Rust facts host input ${file}`;
  canonicalRegularFile(file, label);
  const before = fs.statSync(file, { bigint: true });
  const bytes = fs.readFileSync(file);
  const after = fs.statSync(canonicalRegularFile(file, label), { bigint: true });
  if (
    before.dev !== after.dev ||
    before.ino !== after.ino ||
    before.size !== after.size ||
    before.mtimeNs !== after.mtimeNs ||
    before.ctimeNs !== after.ctimeNs ||
    BigInt(bytes.length) !== before.size
  )
    throw new Error("Rust facts host input changed during read");
  return { path: file, sha256: sha256(bytes), size: bytes.length };
}

function hostBuildOutputSnapshot(file: string) {
  const canonical = canonicalRegularFile(file, "Rust facts host build output");
  const before = fs.statSync(canonical, { bigint: true });
  const bytes = fs.readFileSync(canonical);
  const after = fs.statSync(canonicalRegularFile(file, "Rust facts host build output"), {
    bigint: true,
  });
  if (
    before.dev !== after.dev ||
    before.ino !== after.ino ||
    before.mode !== after.mode ||
    before.size !== after.size ||
    before.mtimeNs !== after.mtimeNs ||
    before.ctimeNs !== after.ctimeNs ||
    BigInt(bytes.length) !== before.size
  )
    throw new Error("Rust facts host build output changed during capture");
  return {
    bytes,
    record: {
      path: file,
      sha256: sha256(bytes),
      size: bytes.length,
      device: before.dev.toString(),
      inode: before.ino.toString(),
      mode: Number(before.mode & 0o777n),
      mtimeNs: before.mtimeNs.toString(),
      ctimeNs: before.ctimeNs.toString(),
    },
  };
}

export function hostBuildOutputRecord(file: string): z.infer<typeof hostBuildOutputRecordSchema> {
  return hostBuildOutputSnapshot(file).record;
}

export function assertHostPhysicalRecord(expected: FileRecord): void {
  const actual = hostFileRecord(expected.path);
  if (
    !isDeepStrictEqual(actual, {
      path: expected.path,
      sha256: expected.sha256,
      size: expected.size,
    })
  )
    throw new Error(`Rust facts host physical proof changed: ${expected.path}`);
}

export function hostAttemptDirectory(root: string, receipt: string): string {
  if (
    !path.isAbsolute(root) ||
    fs.realpathSync(root) !== root ||
    path.resolve(receipt) !== receipt ||
    !receipt.startsWith(`${path.join(root, "tmp/quality/rust-facts-host")}${path.sep}`)
  )
    throw new Error("unsafe Rust facts host receipt destination");
  const parent = path.dirname(receipt);
  let current = parent;
  while (!fs.existsSync(current)) current = path.dirname(current);
  if (fs.realpathSync(current) !== current) throw new Error("Rust facts host destination symlink");
  fs.mkdirSync(parent, { recursive: true });
  if (fs.realpathSync(parent) !== parent) throw new Error("Rust facts host destination changed");
  return fs.mkdtempSync(path.join(parent, "attempt-"));
}

export function createRustFactsHostAttemptDirectory(
  root: string,
  receipt: string,
  attempt: string,
): string {
  const directory = rustFactsHostArtifactPath(root, receipt, attempt, "0".repeat(64));
  const attemptDirectory = path.dirname(directory);
  const parent = path.dirname(attemptDirectory);
  let current = parent;
  while (!fs.existsSync(current)) current = path.dirname(current);
  if (fs.realpathSync(current) !== current) throw new Error("Rust facts host destination symlink");
  fs.mkdirSync(parent, { recursive: true });
  if (fs.realpathSync(parent) !== parent) throw new Error("Rust facts host destination changed");
  fs.mkdirSync(attemptDirectory);
  if (fs.realpathSync(attemptDirectory) !== attemptDirectory)
    throw new Error("Rust facts host attempt directory changed");
  return attemptDirectory;
}

export function publishRustFactsHostBinary(
  root: string,
  receipt: string,
  attempt: string,
  buildOutput: z.infer<typeof hostBuildOutputRecordSchema>,
): z.infer<typeof hostBinaryRecordSchema> {
  const source = rustFactsHostBinaryPath(root);
  if (buildOutput.path !== source) throw new Error("Rust facts host build output path differs");
  const directory = path.dirname(rustFactsHostArtifactPath(root, receipt, attempt, "0".repeat(64)));
  if (!fs.existsSync(directory) || fs.realpathSync(directory) !== directory)
    throw new Error("Rust facts host attempt directory differs");
  const { bytes, record: captured } = hostBuildOutputSnapshot(source);
  if (!isDeepStrictEqual(captured, buildOutput))
    throw new Error("Rust facts host build output changed during capture");
  const digest = captured.sha256;
  const published = rustFactsHostArtifactPath(root, receipt, attempt, digest);
  if (fs.existsSync(published)) throw new Error("Rust facts host artifact already exists");
  const staged = path.join(directory, `.mathjax-consumer-facts-${randomUUID()}.pending`);
  fs.writeFileSync(staged, bytes, { flag: "wx", mode: buildOutput.mode });
  fs.chmodSync(staged, buildOutput.mode);
  const stagedRecord = hostFileRecord(staged);
  const stagedStat = fs.statSync(staged, { bigint: true });
  const sourceAfterCopy = hostBuildOutputRecord(source);
  if (
    !isDeepStrictEqual(sourceAfterCopy, buildOutput) ||
    stagedRecord.sha256 !== digest ||
    stagedRecord.size !== bytes.length ||
    (stagedStat.dev.toString() === buildOutput.device &&
      stagedStat.ino.toString() === buildOutput.inode) ||
    Number(stagedStat.mode & 0o777n) !== buildOutput.mode
  )
    throw new Error("Rust facts host build output changed during publication");
  fs.copyFileSync(staged, published, fs.constants.COPYFILE_EXCL);
  const artifact = hostFileRecord(published);
  const publishedStat = fs.statSync(published, { bigint: true });
  if (
    artifact.sha256 !== digest ||
    artifact.size !== bytes.length ||
    Number(publishedStat.mode & 0o777n) !== buildOutput.mode
  )
    throw new Error("Rust facts host artifact differs after publication");
  fs.unlinkSync(staged);
  return { ...artifact, mode: Number(publishedStat.mode & 0o777n) };
}

export function assertRustFactsHostBinaryRecord(
  expected: z.infer<typeof hostBinaryRecordSchema>,
): void {
  assertHostPhysicalRecord(expected);
  const actual = Number(fs.statSync(expected.path, { bigint: true }).mode & 0o777n);
  if (actual !== expected.mode) throw new Error("Rust facts host artifact mode changed");
}

function runObserved(root: string, argv: string[], directory: string, label: string) {
  let stdout: Buffer = Buffer.alloc(0),
    stderr: Buffer = Buffer.alloc(0);
  let exitCode: number | null = null,
    signal: string | number | null = null,
    error: string | null = null;
  try {
    const child = Bun.spawnSync(argv, {
      cwd: root,
      stdout: "pipe",
      stderr: "pipe",
      timeout: 120_000,
    });
    stdout = Buffer.from(child.stdout);
    stderr = Buffer.from(child.stderr);
    exitCode = child.exitCode;
    signal = child.signalCode ?? null;
  } catch (cause) {
    error = cause instanceof Error ? cause.message : String(cause);
  }
  const output = path.join(directory, `${label}.stdout`),
    errors = path.join(directory, `${label}.stderr`);
  fs.writeFileSync(output, stdout, { flag: "wx" });
  fs.writeFileSync(errors, stderr, { flag: "wx" });
  const proof = hostProcessSchema.parse({
    argv,
    cwd: root,
    exitCode,
    signal,
    error,
    stdout: hostFileRecord(output),
    stderr: hostFileRecord(errors),
  });
  fs.writeFileSync(path.join(directory, `${label}.process.json`), JSON.stringify(proof), {
    flag: "wx",
  });
  return { proof, stdout, stderr };
}

export function observeRustFactsHostRecipe(
  root: string,
  just: string,
  recipe: "rust-facts-host-build" | "rust-facts-host-metadata",
  directory: string,
  label: "metadata-before" | "metadata-after" | "build",
) {
  if (!path.isAbsolute(just) || path.resolve(just) !== just)
    throw new Error("Rust facts host Just invocation path differs");
  canonicalRegularFile(fs.realpathSync(just), "Rust facts host Just");
  return runObserved(root, [just, recipe, root], directory, label);
}

export function observeRustFactsHostBinary(
  root: string,
  binary: z.infer<typeof hostBinaryRecordSchema>,
  receipt: string,
  attempt: string,
  mode: RustFactsHostMode,
  sourceRoot: string,
  directory: string,
) {
  const expected = rustFactsHostArtifactPath(root, receipt, attempt, binary.sha256);
  if (binary.path !== expected) throw new Error("Rust facts executable path differs");
  assertRustFactsHostBinaryRecord(binary);
  const modes = {
    mathjax: [],
    "native-width": ["--native-width-hash"],
    classic: ["--mermaid-classic-scripts"],
    "macho-noso": ["--macho-noso"],
  };
  if (!Object.hasOwn(modes, mode)) throw new Error("unknown Rust facts host mode");
  return runObserved(root, [binary.path, ...modes[mode], sourceRoot], directory, "facts");
}

export function requireRustFactsHostSuccess(proof: z.infer<typeof hostProcessSchema>): void {
  if (proof.exitCode !== 0 || proof.signal !== null || proof.error !== null)
    throw new Error(`Rust facts host preparation failed; preserved output: ${proof.stderr.path}`);
  const observation = path.join(
    path.dirname(proof.stdout.path),
    `${path.basename(proof.stdout.path, ".stdout")}.process.json`,
  );
  hostFileRecord(observation);
  const observed = hostProcessSchema.parse(JSON.parse(fs.readFileSync(observation, "utf8")));
  if (!isDeepStrictEqual(observed, proof))
    throw new Error("Rust facts host process observation differs");
}

function selectedHostEnvironment(
  root: string,
  environment: NodeJS.ProcessEnv = process.env,
): NodeJS.ProcessEnv {
  if (
    [
      "RUSTC",
      "RUSTC_WRAPPER",
      "RUSTC_WORKSPACE_WRAPPER",
      "RUSTC_BOOTSTRAP",
      "CARGO_ENCODED_RUSTFLAGS",
    ].some((name) => environment[name])
  )
    throw new Error("Rust facts host tool override is unsupported");
  const rtk =
    environment.RTK ?? Bun.which("rtk", { cwd: root, PATH: environment.PATH ?? "" }) ?? "";
  const cargo = rtk === "" ? "cargo" : `${rtk} proxy cargo`;
  if (environment.CARGO && environment.CARGO !== cargo)
    throw new Error("Rust facts host Cargo override is unsupported");
  if (
    environment.CARGO_TARGET_DIR &&
    path.resolve(environment.CARGO_TARGET_DIR) !== path.join(root, "target")
  )
    throw new Error("Rust facts host target override is unsupported");
  const entries = Object.entries(environment).filter(([name]) =>
    /^(?:CARGO_|RUST|RTK$|CC$|AR$|PATH$|HOME$|LANG$|LC_|SOURCE_DATE_EPOCH$)/u.test(name),
  );
  const selectedEnvironment = Object.fromEntries(entries);
  selectedEnvironment.CARGO = cargo;
  selectedEnvironment.CARGO_TARGET_DIR = path.join(root, "target");
  selectedEnvironment.RUSTFLAGS ??= "-D warnings";
  return selectedEnvironment;
}

function digestSelectedHostEnvironment(environment: NodeJS.ProcessEnv): string {
  return sha256(
    Buffer.from(JSON.stringify(Object.entries(environment).sort(([a], [b]) => a.localeCompare(b)))),
  );
}

export function hostEnvironmentDigest(
  root: string,
  environment: NodeJS.ProcessEnv = process.env,
): string {
  return digestSelectedHostEnvironment(selectedHostEnvironment(root, environment));
}

export function hostEnvironmentDiagnostics(
  root: string,
  environment: NodeJS.ProcessEnv = process.env,
): { environmentSha256: string; selected: Array<{ name: string; valueSha256: string | null }> } {
  const selectedEnvironment = selectedHostEnvironment(root, environment);
  return {
    environmentSha256: digestSelectedHostEnvironment(selectedEnvironment),
    selected: Object.entries(selectedEnvironment)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([name, value]) => ({
        name,
        valueSha256: value === undefined ? null : sha256(Buffer.from(value)),
      })),
  };
}
