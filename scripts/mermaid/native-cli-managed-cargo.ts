import fs from "node:fs";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";
import { z } from "zod";
import type { JsonValue } from "./mermaid-pair-manifest";
import {
  canonicalRegularFile,
  digestStringRecord,
  type FileRecord,
  validateCapacityReport,
  verifyFileRecord,
} from "./native-build-receipt";
import {
  type ManagedObservation,
  type ManagedReceipt,
  type ManagedRequest,
  managedCapacityObservationSchema,
  managedCapacityPreflightSchema,
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

type CapacityPreflight = ReturnType<typeof managedCapacityPreflightSchema.parse>;
type CapacityObservation = ReturnType<typeof managedCapacityObservationSchema.parse>;
type CapacityEvidence = CapacityPreflight | CapacityObservation;
type CapacityEvidenceSchema = CapacityEvidence["schema"];
type PreflightLogDirectory = "capacity-preflight" | "capacity-preflight-refresh";

function verifyPreflightObservation(
  pin: FileRecord,
  expectedPath: string,
  schema: "krr-native-cli-capacity-preflight-v1",
  root: string,
  python: string,
  attempt: string,
  attemptToken: string,
  logDirectory: PreflightLogDirectory,
  environment: { [key: string]: string },
): CapacityPreflight;
function verifyPreflightObservation(
  pin: FileRecord,
  expectedPath: string,
  schema:
    | "krr-native-cli-capacity-preflight-cleanup-v1"
    | "krr-native-cli-capacity-preflight-refresh-v1",
  root: string,
  python: string,
  attempt: string,
  attemptToken: string,
  logDirectory: PreflightLogDirectory,
  environment: { [key: string]: string },
): CapacityObservation;
function verifyPreflightObservation(
  pin: FileRecord,
  expectedPath: string,
  schema: CapacityEvidenceSchema,
  root: string,
  python: string,
  attempt: string,
  attemptToken: string,
  logDirectory: PreflightLogDirectory,
  environment: { [key: string]: string },
): CapacityEvidence {
  requireBound(pin.path === expectedPath, "capacity preflight evidence path differs");
  const evidence = parseCapacityEvidence(read(pin));
  requireBound(
    evidence.schema === schema && evidence.cwd === root,
    "capacity preflight evidence differs",
  );
  verifyPreflightOutputs(evidence, root, attempt, attemptToken, logDirectory, environment);
  verifyPreflightCommand(evidence, root, python);
  return evidence;
}

function parseCapacityEvidence(value: unknown): CapacityEvidence {
  const accepted = managedCapacityPreflightSchema.safeParse(value);
  return accepted.success ? accepted.data : managedCapacityObservationSchema.parse(value);
}

function verifyPreflightOutputs(
  evidence: CapacityEvidence,
  root: string,
  attempt: string,
  attemptToken: string,
  logDirectory: PreflightLogDirectory,
  environment: { [key: string]: string },
): void {
  const lifecycle = managedLifecycleSchema.parse(read(evidence.lifecycle));
  requireBound(
    path.basename(evidence.lifecycle.path) === "lifecycle.json" &&
      path.dirname(path.dirname(evidence.lifecycle.path)) === path.join(attempt, logDirectory),
    "capacity preflight log directory differs",
  );
  owned(
    managedOwnedSchema.parse({
      argv: evidence.argv,
      stdout: evidence.stdout,
      stderr: evidence.stderr,
      label: "capacity preflight",
      cwd: root,
      attemptToken,
      environment,
      environmentSha256: digestStringRecord(environment),
      pid: lifecycle.identity.pid,
      actualExit: evidence.actualExit,
      startedNs: evidence.startedNs,
      endedNs: evidence.endedNs,
      reaped: evidence.reaped,
      errors: [],
    }),
    evidence.lifecycle,
    "preflight",
  );
  const actualReport = parseManagedJson(
    fs.readFileSync(evidence.stdout.path, "utf8"),
    "capacity preflight report",
  );
  same(actualReport, evidence.report, "capacity preflight raw report");
}

function verifyPreflightCommand(evidence: CapacityEvidence, root: string, python: string): void {
  const expected = [
    python,
    path.join(root, "scripts/build/cargo_capacity_guard.py"),
    "--root",
    root,
    "--target",
    path.join(root, "target"),
    "--state",
    path.join(root, "tmp/.cargo-capacity-guard.json"),
    "--require-capacity",
    "--print-json",
  ];
  const fixedArguments = isDeepStrictEqual(evidence.argv, expected);
  const retainedDebugProfile =
    evidence.argv.length === expected.length + 2 &&
    isDeepStrictEqual(evidence.argv.slice(0, expected.length), expected) &&
    evidence.argv[expected.length] === "--retain-profile" &&
    evidence.argv[expected.length + 1] === path.join(root, "target/debug");
  requireBound(fixedArguments || retainedDebugProfile, "capacity preflight admission argv differs");
}

function verifyAcceptedPreflight(receipt: ManagedReceipt): CapacityPreflight {
  const attempt = path.dirname(receipt.request.path);
  const acceptedContents = managedCapacityPreflightSchema.parse(read(receipt.capacityPreflight));
  const accepted = verifyPreflightObservation(
    receipt.capacityPreflight,
    path.join(attempt, "capacity-preflight.json"),
    "krr-native-cli-capacity-preflight-v1",
    receipt.cwd,
    receipt.snapshotAfter.tools.python3.path,
    attempt,
    receipt.attemptToken,
    acceptedContents.refreshObservation === null
      ? "capacity-preflight"
      : "capacity-preflight-refresh",
    receipt.snapshotAfter.environment,
  );
  return managedCapacityPreflightSchema.parse(accepted);
}

function verifyPreflightReport(value: JsonValue, root: string): void {
  const report = z.record(z.string(), z.unknown()).parse(value);
  validateCapacityReport(value, "capacity preflight", root);
  requireBound(
    report.cleanup_status === "not-needed" &&
      Array.isArray(report.removed) &&
      report.removed.length === 0 &&
      (report.cleanup_attempts === undefined ||
        (Array.isArray(report.cleanup_attempts) && report.cleanup_attempts.length === 0)),
    "accepted capacity preflight includes cleanup",
  );
}

function verifyCleanupPreflightPair(receipt: ManagedReceipt, accepted: CapacityPreflight): void {
  const root = receipt.cwd;
  const attempt = path.dirname(receipt.request.path);
  const cleanupPin = accepted.cleanupObservation;
  const refreshPin = accepted.refreshObservation;
  if (cleanupPin === null || refreshPin === null)
    throw new Error("Managed native CLI capacity preflight evidence pair differs");
  const cleanup = verifyPreflightObservation(
    cleanupPin,
    path.join(attempt, "capacity-preflight-cleanup-observation.json"),
    "krr-native-cli-capacity-preflight-cleanup-v1",
    root,
    receipt.snapshotAfter.tools.python3.path,
    attempt,
    receipt.attemptToken,
    "capacity-preflight",
    receipt.snapshotAfter.environment,
  );
  const refresh = verifyPreflightObservation(
    refreshPin,
    path.join(attempt, "capacity-preflight-refresh-observation.json"),
    "krr-native-cli-capacity-preflight-refresh-v1",
    root,
    receipt.snapshotAfter.tools.python3.path,
    attempt,
    receipt.attemptToken,
    "capacity-preflight-refresh",
    receipt.snapshotAfter.environment,
  );
  verifyCleanupRefreshAgreement(accepted, cleanup, refresh, root);
}

function verifyCleanupRefreshAgreement(
  accepted: CapacityPreflight,
  cleanup: CapacityObservation,
  refresh: CapacityObservation,
  root: string,
): void {
  same(accepted.argv, cleanup.argv, "cleanup capacity preflight argv");
  same(accepted.argv, refresh.argv, "refresh capacity preflight argv");
  same(accepted.report, refresh.report, "fresh capacity preflight report");
  same(accepted.lifecycle, refresh.lifecycle, "fresh capacity preflight lifecycle");
  same(accepted.stdout, refresh.stdout, "fresh capacity preflight stdout");
  same(accepted.stderr, refresh.stderr, "fresh capacity preflight stderr");
  const rawCleanup = parseManagedJson(
    fs.readFileSync(cleanup.stdout.path, "utf8"),
    "cleanup capacity report",
  );
  verifyCleanupCapacityReport(rawCleanup, root);
}

function verifyCleanupCapacityReport(value: JsonValue, root: string): void {
  const initial = z.record(z.string(), z.unknown()).parse(value);
  const policy = managedCapacitySchema.shape.capacityPolicy.strip().parse(initial);
  requireBound(
    (initial.cleanup_status === "completed" || initial.cleanup_status === "incomplete") &&
      initial.metric_available === true &&
      initial.clean_exitcode === 0 &&
      initial.over_capacity === true &&
      typeof initial.current_bytes === "number" &&
      Number.isSafeInteger(initial.current_bytes) &&
      initial.current_bytes > policy.threshold_bytes &&
      initial.remaining_over_capacity === false &&
      typeof initial.post_clean_bytes === "number" &&
      Number.isSafeInteger(initial.post_clean_bytes) &&
      initial.post_clean_bytes >= 0 &&
      initial.post_clean_bytes <= policy.threshold_bytes &&
      policy.target === path.join(root, "target"),
    "cleanup capacity preflight report differs",
  );
}

export function verifyManagedCapacityPreflight(receipt: ManagedReceipt): void {
  const accepted = verifyAcceptedPreflight(receipt);
  const rawReport = parseManagedJson(
    fs.readFileSync(accepted.stdout.path, "utf8"),
    "accepted capacity preflight report",
  );
  verifyPreflightReport(rawReport, receipt.cwd);
  const hasCleanup = accepted.cleanupObservation !== null;
  requireBound(
    hasCleanup === (accepted.refreshObservation !== null),
    "capacity preflight evidence pair differs",
  );
  if (hasCleanup) verifyCleanupPreflightPair(receipt, accepted);
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
  verifyManagedCapacityPreflight(receipt);
  const metadata = managedOwnedSchema.parse(read(request.metadataObservation));
  owned(metadata, receipt.metadataLifecycle, "metadata");
  owned(receipt.buildObservation, receipt.buildLifecycle, "build");
  const tools = receipt.snapshotAfter.tools;
  const launchers = receipt.snapshotAfter.toolLaunchers;
  same(
    receipt.buildObservation.argv,
    [tools.rtk.path, "proxy", tools.just.path, "krr-build"],
    "sole build argv",
  );
  same(
    metadata.argv,
    [
      tools.rtk.path,
      "proxy",
      launchers.cargo.path,
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
  const cargoRtk = cargo.argv[0];
  requireBound(
    cargoRtk !== undefined && fs.realpathSync(cargoRtk) === tools.rtk.path,
    "guard RTK proxy identity differs",
  );
  same(
    cargo.argv.slice(1),
    [
      "proxy",
      "cargo",
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
  same(observation.pinsAfter.rustc, tools.rustc, "guard Rustc pin");
  same(observation.pinsAfter.cargoLauncher, launchers.cargo, "guard Cargo launcher");
  same(observation.pinsAfter.rustcLauncher, launchers.rustc, "guard Rustc launcher");
  same(observation.pinsAfter.guardCodePins, request.guardCodePins, "guard code pins");
  same(observation.pinsAfter.capacityAdmission, request.capacityAdmission, "guard capacity pins");
  for (const name of observation.pinsAfter.unsupportedCargoConfigsAbsent)
    requireBound(!fs.existsSync(name), "Cargo config appeared");
  same(observation.pinsAfter.rustupSelectedRustc, tools.rustc, "Rustup-selected Rustc pin");
  verifyRustupResolutionQueries(
    receipt,
    request,
    observation.ownedCargo,
    observation.rustupResolutionQueries,
  );
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

function verifyRustupResolutionQueries(
  receipt: ManagedReceipt,
  request: ManagedRequest,
  cargo: ManagedObservation["ownedCargo"],
  queries: ManagedObservation["rustupResolutionQueries"],
): void {
  const tools = receipt.snapshotAfter.tools;
  const seen = new Set(
    queries.map((query) => verifyRustupResolutionQuery(receipt, request, tools, query)),
  );
  requireBound(
    seen.has("cargo") &&
      seen.has("rustc") &&
      queries.some((query) => BigInt(query.observation.endedNs) <= BigInt(cargo.startedNs)) &&
      queries.some((query) => BigInt(query.observation.startedNs) >= BigInt(cargo.endedNs)),
    "Rustup query pair/endpoints are incomplete",
  );
}

function verifyRustupResolutionQuery(
  receipt: ManagedReceipt,
  request: ManagedRequest,
  tools: ManagedReceipt["snapshotAfter"]["tools"],
  query: ManagedObservation["rustupResolutionQueries"][number],
): "cargo" | "rustc" {
  const name = query.observation.argv[2];
  if (name !== "cargo" && name !== "rustc") throw new Error("Managed Rustup query tool differs");
  owned(query.observation, query.lifecycle, "tool-query");
  same(
    query.observation.argv,
    [receipt.snapshotAfter.toolLaunchers[name].resolvedPath, "which", name],
    "Rustup query argv",
  );
  same(query.observation.environment, request.environment, "Rustup query environment");
  requireBound(
    query.observation.cwd === receipt.cwd &&
      query.observation.attemptToken === receipt.attemptToken &&
      query.observation.label === `probe-rustup-${name}` &&
      query.lifecycle.path.startsWith(path.join(path.dirname(receipt.request.path), "lifecycle")),
    "Rustup query attempt binding differs",
  );
  requireBound(
    fs.readFileSync(query.observation.stdout.path, "utf8") === `${tools[name].path}\n` &&
      fs.statSync(query.observation.stderr.path).size === 0,
    "Rustup resolved executable output differs",
  );
  return name;
}
