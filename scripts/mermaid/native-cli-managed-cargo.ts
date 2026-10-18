import fs from "node:fs";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";
import { z } from "zod";
import {
  canonicalRegularFile,
  type FileRecord,
  validateCapacityReport,
  verifyFileRecord,
} from "./native-build-receipt";
import {
  type ManagedObservation,
  type ManagedReceipt,
  type ManagedRequest,
  managedLifecycleSchema,
  managedOwnedSchema,
} from "./native-cli-managed-build-schema";
import {
  cargoArtifactSchema,
  cargoMetadataSchema,
  managedCapacitySchema,
} from "./native-cli-managed-input-schema";
import { parseManagedJson } from "./native-cli-managed-json";
import { owned, read, requireBound, same } from "./native-cli-managed-process";

export function validateManagedCliArtifact(
  value: unknown,
): ReturnType<typeof cargoArtifactSchema.parse> {
  const artifact = cargoArtifactSchema.parse(value);
  same(artifact.features, [], "CLI artifact selected features");
  return artifact;
}

export function verifyManagedCapacity(root: string, before: FileRecord, after: FileRecord): void {
  const measurement = managedCapacitySchema.shape.capacityPolicy.extend({
    state: z.literal(path.join(root, "tmp/.cargo-capacity-guard.json")),
    current_bytes: z.number().int().nonnegative().max(4726095872),
    over_capacity: z.literal(false),
    metric_available: z.literal(true),
  });
  const execution = measurement
    .extend({
      removed: z.array(z.unknown()),
      cleanup_status: z.literal("not-needed"),
      actual_removed_bytes: z.literal(0),
      post_clean_bytes: z.null(),
      remaining_over_capacity: z.literal(false),
      baseline_eligible: z.boolean(),
    })
    .passthrough()
    .refine(
      (report) => report.baseline_eligible === (report.current_bytes === 0),
      "baseline eligibility must reflect an empty pre-build target",
    );
  const beforeValue = read(before),
    afterValue = read(after);
  for (const value of [beforeValue, afterValue])
    validateCapacityReport(value, "managed capacity", root);
  requireBound(execution.parse(beforeValue).removed.length === 0, "capacity cleanup occurred");
  measurement.strict().parse(afterValue);
}

export function verifyManagedCargo(
  receipt: ManagedReceipt,
  request: ManagedRequest,
  observation: ManagedObservation,
): void {
  const attempt = path.dirname(receipt.request.path);
  requireBound(
    observation.capacityBefore.path === path.join(attempt, "capacity.before.json") &&
      observation.capacityAfter.path === path.join(attempt, "capacity.after.json") &&
      observation.ownedCargo.stdout.path === path.join(attempt, "cargo.stdout.raw") &&
      observation.ownedCargo.stderr.path === path.join(attempt, "cargo.stderr.raw"),
    "guard output roles differ",
  );
  const metadata = managedOwnedSchema.parse(read(request.metadataObservation));
  owned(metadata, receipt.metadataLifecycle, "metadata");
  owned(receipt.buildObservation, receipt.buildLifecycle, "build");
  const tools = receipt.snapshotAfter.tools;
  same(
    receipt.buildObservation.argv,
    [tools.rtk.path, "proxy", tools.just.path, "--set", "CARGO", tools.cargo.path, "krr-build"],
    "sole build argv",
  );
  same(
    metadata.argv,
    [
      tools.rtk.path,
      "proxy",
      tools.cargo.path,
      "metadata",
      "--frozen",
      "--format-version",
      "1",
      "--no-deps",
      "--manifest-path",
      path.join(receipt.cwd, "Cargo.toml"),
    ],
    "metadata argv",
  );
  same(metadata.environment, request.environment, "metadata environment");
  requireBound(
    metadata.attemptToken === receipt.attemptToken &&
      metadata.cwd === receipt.cwd &&
      metadata.label === "probe-5-cargo-metadata" &&
      request.metadata.path === path.join(path.dirname(receipt.request.path), "metadata.json"),
    "metadata binding differs",
  );
  same(metadata.stdout, request.metadata, "metadata stdout");
  const data = cargoMetadataSchema.parse(read(request.metadata));
  requireBound(
    data.workspace_root === receipt.cwd && data.target_directory === request.target,
    "metadata root differs",
  );
  const packages = data.packages.filter(
    (item) =>
      item.id === request.expectedPackage.id ||
      item.name === request.expectedPackage.name ||
      item.manifest_path === request.expectedPackage.manifestPath,
  );
  requireBound(
    new Set(data.packages.map((item) => item.id)).size === data.packages.length &&
      packages.length === 1,
    "metadata package is not unique",
  );
  const pkg = packages[0];
  requireBound(
    pkg !== undefined &&
      pkg.id === request.expectedPackage.id &&
      pkg.name === request.expectedPackage.name &&
      pkg.manifest_path === path.join(receipt.cwd, "crates/katana-render-runtime-cli/Cargo.toml"),
    "package identity differs",
  );
  same(pkg?.features, {}, "CLI metadata feature inventory");
  const manifest = z
    .object({ features: z.record(z.string(), z.array(z.string())).optional() })
    .passthrough()
    .parse(Bun.TOML.parse(fs.readFileSync(request.expectedPackage.manifestPath, "utf8")));
  same(manifest.features ?? {}, {}, "CLI source feature inventory");
  const target = pkg?.targets.filter((item) => item.name === "krr");
  requireBound(
    target?.length === 1 &&
      isDeepStrictEqual(target[0]?.kind, ["bin"]) &&
      target[0]?.src_path ===
        path.join(receipt.cwd, "crates/katana-render-runtime-cli/src/main.rs"),
    "metadata target differs",
  );
  const cargo = observation.ownedCargo;
  const lifecycle = managedLifecycleSchema.parse(read(receipt.buildLifecycle));
  same(
    cargo.argv,
    [
      tools.cargo.path,
      "build",
      "-p",
      "katana-render-runtime-cli",
      "--message-format=json-render-diagnostics",
    ],
    "guard sole Cargo argv",
  );
  requireBound(
    cargo.pid !== metadata.pid &&
      cargo.pid !== receipt.buildObservation.pid &&
      cargo.parentPid !== cargo.pid &&
      cargo.sessionId === lifecycle.identity.pid &&
      cargo.processGroupId === lifecycle.identity.pid &&
      BigInt(cargo.startedNs) >= BigInt(receipt.buildObservation.startedNs) &&
      BigInt(cargo.startedNs) >= BigInt(metadata.endedNs) &&
      BigInt(cargo.endedNs) <= BigInt(receipt.buildObservation.endedNs) &&
      BigInt(cargo.endedNs) >= BigInt(cargo.startedNs),
    "Cargo PID/group/time differs",
  );
  same(cargo.environment, request.environment, "guard Cargo environment");
  same(cargo.environmentSha256, request.environmentSha256, "guard Cargo environment digest");
  requireBound(
    cargo.cwd === receipt.cwd && cargo.attemptToken === receipt.attemptToken,
    "guard Cargo attempt differs",
  );
  same(observation.pinsBefore, observation.pinsAfter, "guard input endpoints");
  same(observation.pinsAfter.source, receipt.snapshotAfter.source, "guard source pins");
  same(observation.pinsAfter.cargo, tools.cargo, "guard Cargo pin");
  same(observation.pinsAfter.guardCodePins, request.guardCodePins, "guard code pins");
  same(observation.pinsAfter.capacityAdmission, request.capacityAdmission, "guard capacity pins");
  for (const name of observation.pinsAfter.unsupportedCargoConfigsAbsent)
    requireBound(!fs.existsSync(name), "Cargo config appeared");
  verifyFileRecord(observation.pinsAfter.observedPathRustc, "observed PATH rustc");
  verifyManagedCapacity(receipt.cwd, observation.capacityBefore, observation.capacityAfter);
  const raw = fs.readFileSync(canonicalRegularFile(cargo.stdout.path, "Cargo raw"), "utf8");
  verifyFileRecord(cargo.stdout, "Cargo raw");
  verifyFileRecord(cargo.stderr, "Cargo raw");
  const events = raw
    .split(/\r?\n/)
    .filter((line) => line.trim())
    .map((line) =>
      z.object({ reason: z.string() }).passthrough().parse(parseManagedJson(line, "Cargo JSON")),
    );
  const artifacts = events.filter(
    (event) =>
      event.reason === "compiler-artifact" &&
      event.package_id === request.expectedPackage.id &&
      z.object({ name: z.string() }).passthrough().parse(event.target).name === "krr",
  );
  requireBound(
    artifacts.length === 1 &&
      events.filter((event) => event.reason === "build-finished").length === 1 &&
      events.at(-1)?.reason === "build-finished" &&
      events.at(-1)?.success === true,
    "Cargo artifact/finish is not unique terminal success",
  );
  same(artifacts[0], observation.artifact, "raw Cargo artifact");
  same(events.at(-1), observation.buildFinished, "raw Cargo finish");
  const artifact = validateManagedCliArtifact(observation.artifact);
  requireBound(
    artifact.executable === request.binaryPath &&
      artifact.filenames.includes(request.binaryPath) &&
      artifact.target.src_path === request.expectedTarget.srcPath,
    "Cargo binary/target differs",
  );
}
