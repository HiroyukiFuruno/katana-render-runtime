import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";
import { z } from "zod";
import { sha256 } from "./mermaid-pair-manifest";
import {
  assertCurrentGitSourceSet,
  digestFileRecords,
  digestStringRecord,
  verifyFileRecord,
} from "./native-build-receipt";
import { type ManagedReceipt, managedSourceSchema } from "./native-cli-managed-build-schema";
import { parseManagedJson } from "./native-cli-managed-json";
import { read, requireBound, same } from "./native-cli-managed-process";

const CONTROL_MANIFEST_NAME = "scripts/mermaid/native-cli-managed-control-inputs.json";
const managedControlManifestSchema = z
  .object({
    schema: z.literal("krr-native-cli-managed-control-inputs-v1"),
    control: z.array(z.string().min(1)).length(27),
  })
  .strict();
export function parseManagedControlManifest(serialized: string): readonly string[] {
  const control = managedControlManifestSchema.parse(
    parseManagedJson(serialized, "managed control inventory"),
  ).control;
  if (
    new Set(control).size !== control.length ||
    control.some(
      (name) =>
        !/^[A-Za-z0-9_./-]+$/.test(name) ||
        path.posix.isAbsolute(name) ||
        name.split("/").some((part) => part === "" || part === "." || part === ".."),
    ) ||
    !control.includes(CONTROL_MANIFEST_NAME)
  ) {
    throw new Error("managed control inventory is not canonical");
  }
  return control;
}
export const managedControlNames = parseManagedControlManifest(
  fs.readFileSync(
    path.join(
      path.dirname(fileURLToPath(import.meta.url)),
      "native-cli-managed-control-inputs.json",
    ),
    "utf8",
  ),
);
const emptySelectors = [
  "KRR_BUILD_RECEIPT",
  "MERMAID_RASTER_SESSION_CONFIG",
  "MERMAID_RASTER_SESSION_CONFIG_SHA",
  "MERMAID_RASTER_BINDING_CONFIG",
  "MERMAID_RASTER_BINDING_CONFIG_SHA",
];
export function verifyManagedInputs(receipt: ManagedReceipt): void {
  const root = receipt.cwd;
  same(receipt.snapshotBefore, receipt.snapshotAfter, "outer input endpoints");
  const snapshot = receipt.snapshotAfter;
  const expected = managedSourceSchema.parse(read(receipt.expectedSource));
  same(expected.files, snapshot.source, "independent expected source");
  requireBound(expected.digest === digestFileRecords(expected.files), "source digest differs");
  const sorted = [...expected.files].sort((a, b) =>
    Buffer.compare(Buffer.from(a.path), Buffer.from(b.path)),
  );
  same(sorted, expected.files, "source order");
  requireBound(
    new Set(expected.files.map((item) => item.path)).size === expected.files.length,
    "source duplicate",
  );
  for (const pin of expected.files) {
    requireBound(
      pin.path.startsWith(`${root}/`) &&
        !pin.path.startsWith(`${root}/tmp/`) &&
        !pin.path.startsWith(`${root}/target/`),
      "source boundary differs",
    );
    verifyFileRecord(pin, "managed current source");
  }
  assertCurrentGitSourceSet(root, expected.files, snapshot.tools.git.path, snapshot.environment);
  same(
    snapshot.control.map((pin) => pin.path),
    managedControlNames.map((name) => path.join(root, name)),
    "managed code inventory",
  );
  for (const pin of snapshot.control)
    requireBound(
      expected.files.some((row) => isDeepStrictEqual(row, pin)),
      "managed code not admitted",
    );
  for (const pin of [
    ...Object.values(snapshot.tools),
    ...snapshot.toolchainFiles,
    ...receipt.runtimeAssetPins,
  ])
    verifyFileRecord(pin, "managed input");
  verifyJustLauncher(root, snapshot);
  requireBound(
    snapshot.tools.bun.path === fs.realpathSync(process.execPath),
    "current reader Bun differs",
  );
  requireBound(
    path.dirname(snapshot.tools.cargo.path) === path.dirname(snapshot.tools.rustc.path) &&
      path.dirname(snapshot.toolLaunchers.cargo.path) ===
        path.dirname(snapshot.toolLaunchers.rustc.path),
    "Rustup launcher directories differ",
  );
  for (const name of ["cargo", "rustc"] as const) {
    const launcher = snapshot.toolLaunchers[name];
    const selected = snapshot.environment.PATH?.split(path.delimiter)
      .map((directory) => path.resolve(root, directory, name))
      .find((candidate) => fs.existsSync(candidate) && (fs.statSync(candidate).mode & 0o111) !== 0);
    requireBound(
      selected === launcher.path &&
        launcher.path === path.join(path.dirname(launcher.path), name) &&
        path.basename(launcher.resolvedPath) === "rustup" &&
        fs.realpathSync(launcher.path) === launcher.resolvedPath &&
        sha256(fs.readFileSync(launcher.resolvedPath)) === launcher.sha256 &&
        isDeepStrictEqual(launcher.resolutionArgv, [launcher.resolvedPath, "which", name]),
      `${name} Rustup launcher identity differs`,
    );
  }
  for (const name of snapshot.toolchainAbsent)
    requireBound(!fs.existsSync(name), "toolchain config appeared");
  same(
    snapshot.environmentSha256,
    digestStringRecord(snapshot.environment),
    "child environment digest",
  );
  requireBound(
    snapshot.environment.RUSTFLAGS === "-D warnings" &&
      snapshot.environment.PWD === root &&
      snapshot.environment.SHLVL === "0" &&
      emptySelectors.every((key) => snapshot.environment[key] === ""),
    "child environment selectors differ",
  );
  const capacity = snapshot.capacityAdmission;
  same(receipt.expectedProtectedRegistry, capacity.protectedRegistry, "independent registry pin");
  verifyFileRecord(capacity.state, "capacity state");
  verifyFileRecord(capacity.protectedRegistry, "current protected registry");
  requireBound(
    capacity.state.path === path.join(root, "tmp/.cargo-capacity-guard.json"),
    "capacity state path differs",
  );
  const registry = z
    .object({
      schema: z.literal("krr-protected-refs-successor-v1"),
      physicalRefCount: z.number().int().positive(),
      refs: z.unknown(),
    })
    .strict()
    .parse(read(capacity.protectedRegistry));
  same(registry.refs, capacity.refs, "protected registry references");
  requireBound(registry.physicalRefCount === capacity.refs.length, "protected count differs");
  requireBound(
    new Set(capacity.refs.map((pin) => pin.path)).size === capacity.refs.length,
    "protected paths not unique",
  );
  for (const pin of capacity.refs) {
    verifyFileRecord(pin, "protected reference");
    const stat = fs.statSync(pin.path);
    requireBound(
      stat.dev === pin.dev && stat.ino === pin.ino && stat.mode === pin.mode,
      "protected reference identity differs",
    );
  }
  same(
    JSON.parse(capacity.environmentValue),
    capacity.refs.map((pin) => pin.path),
    "protected environment paths",
  );
  requireBound(
    capacity.environmentValue === snapshot.environment.CARGO_CAPACITY_PROTECTED_REFS &&
      capacity.environmentSha256 === sha256(Buffer.from(capacity.environmentValue)) &&
      capacity.capacityPolicy.target === path.join(root, "target"),
    "capacity input bindings differ",
  );
  z.object({
    metric: z.literal("unique-inode-allocated-bytes"),
    metric_version: z.literal(1),
    baseline_bytes: z.literal(2363047936),
    factor: z.literal(2),
    target: z.literal(path.join(root, "target")),
  })
    .passthrough()
    .parse(read(capacity.state));
  const candidates: string[] = [];
  for (let base = root; ; base = path.dirname(base)) {
    candidates.push(path.join(base, "rust-toolchain"), path.join(base, "rust-toolchain.toml"));
    if (base === path.dirname(base)) break;
  }
  const exists = (name: string) => fs.lstatSync(name, { throwIfNoEntry: false }) !== undefined;
  same(
    snapshot.toolchainFiles.map((pin) => pin.path),
    candidates.filter(exists),
    "toolchain input inventory",
  );
  same(
    snapshot.toolchainAbsent,
    candidates.filter((name) => !exists(name)),
    "toolchain absence inventory",
  );
}

function verifyJustLauncher(root: string, snapshot: ManagedReceipt["snapshotAfter"]): void {
  const tool = snapshot.tools.just;
  const launcher = snapshot.toolLaunchers.just;
  if (launcher === undefined) {
    requireBound(
      path.basename(tool.path) === "just" && fs.realpathSync(tool.path) === tool.path,
      "legacy just invocation is not canonical",
    );
    return;
  }
  const stat = fs.statSync(launcher.path);
  requireBound(
    stat.isFile() && (stat.mode & 0o111) !== 0,
    "logical just launcher is not executable",
  );
  const justfile = snapshot.source.find(
    (pin) => pin.path.toLowerCase() === path.join(root, "justfile").toLowerCase(),
  );
  if (justfile === undefined) throw new Error("tracked Justfile pin is absent");
  const declaration = "NATIVE_JUST_EXECUTABLE := just_executable()";
  requireBound(
    fs
      .readFileSync(justfile.path, "utf8")
      .split(/\r?\n/)
      .filter((line) => line.trim() === declaration).length === 1,
    "Just executable declaration is not uniquely source-bound",
  );
  requireBound(
    path.basename(launcher.path) === "just" &&
      fs.realpathSync(launcher.path) === launcher.resolvedPath &&
      sha256(fs.readFileSync(launcher.resolvedPath)) === launcher.sha256 &&
      fs.realpathSync(tool.path) === tool.path,
    "logical just launcher identity differs",
  );
}
