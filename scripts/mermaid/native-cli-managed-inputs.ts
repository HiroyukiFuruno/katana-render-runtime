import fs from "node:fs";
import path from "node:path";
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
import { read, requireBound, same } from "./native-cli-managed-process";

const controlNames = [
  "Justfile",
  "scripts/build/cargo_capacity_guard.py",
  "scripts/build/cargo_capacity_guard_lock.py",
  "scripts/build/cargo_capacity_guard_native_cli_observation.py",
  "scripts/build/cargo_capacity_guard_build_script_ownership.py",
  "scripts/build/cargo_capacity_guard_selected_cache_ownership.py",
  "scripts/build/cargo_capacity_guard_library_alias_ownership.py",
  "scripts/build/cargo_capacity_guard_different_flags_ownership.py",
  "scripts/mermaid/produce_native_cli_build_receipt.py",
  "scripts/mermaid/native_cli_build_bindings.py",
  "scripts/mermaid/native-build-receipt.ts",
  "scripts/mermaid/native-cli-managed-build.ts",
  "scripts/mermaid/native-cli-managed-build-schema.ts",
  "scripts/mermaid/native-cli-managed-input-schema.ts",
  "scripts/mermaid/native-cli-managed-inputs.ts",
  "scripts/mermaid/native-cli-managed-process.ts",
  "scripts/mermaid/native-cli-managed-cargo.ts",
  "scripts/mermaid/native-cli-managed-json.ts",
  "scripts/mermaid/native-cli-managed-raw-fd.ts",

  "scripts/release/produce_html_performance_evidence.py",
  "scripts/release/verify_html_performance_evidence.py",
  "scripts/release/toml_loader.py",
];
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
    controlNames.map((name) => path.join(root, name)),
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
  requireBound(
    snapshot.tools.bun.path === fs.realpathSync(process.execPath),
    "current reader Bun differs",
  );
  requireBound(
    path.dirname(snapshot.tools.cargo.path) === path.dirname(snapshot.tools.rustc.path) &&
      snapshot.environment.PATH?.split(path.delimiter)[0] ===
        path.dirname(snapshot.tools.cargo.path),
    "selected toolchain PATH binding differs",
  );
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
