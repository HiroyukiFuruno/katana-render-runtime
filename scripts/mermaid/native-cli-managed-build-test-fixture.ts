import fs from "node:fs";
import path from "node:path";
import { rustFactsHostReceiptSchema } from "../runtime-bundles/runtime-rust-facts-host";
import {
  currentRustFactsHostInputs,
  hostFileRecord,
} from "../runtime-bundles/runtime-rust-facts-host-inputs";
import {
  createRustFactsHostAttemptDirectory,
  hostBuildOutputRecord,
  hostEnvironmentDigest,
  publishRustFactsHostBinary,
} from "../runtime-bundles/runtime-rust-facts-host-protocol";
import { hostFixtureInputs } from "../runtime-bundles/runtime-rust-facts-host-test-fixture";
import type { FileRecord } from "./native-build-receipt";
import { createManagedReaderInputs } from "./native-cli-managed-build-input-test-fixture";
import {
  managedCapacityPreflightSchema,
  managedObservationSchema,
  managedReceiptSchema,
  managedRequestSchema,
} from "./native-cli-managed-build-schema";

function identity(pin: FileRecord) {
  const stat = fs.statSync(pin.path, { bigint: true });
  return {
    ...pin,
    device: Number(stat.dev),
    inode: Number(stat.ino),
    mtimeNs: stat.mtimeNs.toString(),
    ctimeNs: stat.ctimeNs.toString(),
  };
}
export function createManagedReaderFixture(withCapacityCleanup = false) {
  const {
    root,
    token,
    save,
    json,
    source,
    sourcePin,
    expectedSource,
    tool,
    registry,
    policy,
    capacity,
    environment,
    absentConfigs,
    snapshot,
    owned,
    manifest,
    main,
    runtime,
    control,
  } = createManagedReaderInputs();
  const expectedPackage = {
    id: "unit#cli",
    name: "katana-render-runtime-cli",
    manifestPath: manifest.path,
  };
  const expectedTarget = { name: "krr", kind: ["bin"], srcPath: main.path };
  const metadataData = {
    version: 1,
    workspace_root: root,
    target_directory: policy.target,
    packages: [
      {
        id: expectedPackage.id,
        name: expectedPackage.name,
        manifest_path: manifest.path,
        features: {},
        targets: [{ name: "krr", kind: ["bin"], src_path: main.path }],
      },
    ],
  };
  const metadata = owned(
    "metadata",
    "metadata",
    [
      tool("rtk").path,
      "proxy",
      snapshot.toolLaunchers.cargo.path,
      "metadata",
      "--frozen",
      "--format-version",
      "1",
      "--no-deps",
      "--manifest-path",
      path.join(root, "Cargo.toml"),
    ],
    101,
    "probe-5-cargo-metadata",
    JSON.stringify(metadataData),
  );
  const metadataPin = json(`tmp/${token}/metadata.json`, metadataData);
  const metadataObservation = json(`tmp/${token}/metadata-observation.json`, {
    ...metadata.observation,
    stdout: metadataPin,
    stderr: save(`tmp/${token}/metadata.stderr.raw`, ""),
  });
  const guardCodePins = [
    "cargo_capacity_guard.py",
    "cargo_capacity_guard_lock.py",
    "cargo_capacity_guard_native_cli_observation.py",
  ].map((name) => {
    const pin = control.find((pin) => pin.path === path.join(root, "scripts/build", name));
    if (pin === undefined) throw new Error("unit guard code missing");
    return pin;
  });
  const request = managedRequestSchema.parse({
    schema: "krr-native-cli-guard-request-v2",
    attemptToken: token,
    cwd: root,
    target: policy.target,
    binaryPath: path.join(root, "target/debug/krr"),
    issuedMonotonicNs: "1",
    expiresMonotonicNs: "120000000001",
    cargo: tool("cargo"),
    rustc: tool("rustc"),
    cargoLauncher: snapshot.toolLaunchers.cargo,
    rustcLauncher: snapshot.toolLaunchers.rustc,
    sourceManifest: sourcePin,
    sourceDigest: source.digest,
    metadata: metadataPin,
    metadataObservation,
    expectedPackage,
    expectedTarget,
    environment,
    environmentSha256: snapshot.environmentSha256,
    guardCodePins,
    capacityAdmission: capacity,
  });
  const requestPin = json(`tmp/${token}/request.json`, request);
  const outerEnv: { [key: string]: string } = {
    ...environment,
    KRR_NATIVE_CLI_GUARD_REQUEST: requestPin.path,
    KRR_NATIVE_CLI_GUARD_REQUEST_SHA256: requestPin.sha256,
  };
  delete outerEnv.PWD;
  delete outerEnv._;
  const build = owned(
    "build",
    "build",
    [
      tool("rtk").path,
      "proxy",
      tool("just").path,
      "krr-build",
    ],
    102,
    "probe-6-native-cli-build",
    "unit outer output",
    outerEnv,
  );
  const binary = identity(save("target/debug/krr", "unexecuted unit binary data"));
  const artifact = {
    reason: "compiler-artifact",
    package_id: expectedPackage.id,
    target: { name: "krr", kind: ["bin"], src_path: main.path },
    executable: binary.path,
    fresh: true,
    filenames: [binary.path],
    features: [],
    profile: {
      opt_level: "0",
      debuginfo: 2,
      debug_assertions: true,
      overflow_checks: true,
      test: false,
    },
  };
  const finished = { reason: "build-finished", success: true };
  const cargo = {
    argv: [
      tool("rtk").path,
      "proxy",
      "cargo",
      "build",
      "-p",
      "katana-render-runtime-cli",
      "--message-format=json-render-diagnostics",
    ],
    stdout: save(
      `tmp/${token}/cargo.stdout.raw`,
      `${JSON.stringify(artifact)}\n${JSON.stringify(finished)}\n`,
    ),
    stderr: save(`tmp/${token}/cargo.stderr.raw`, ""),
    cwd: root,
    attemptToken: token,
    environment,
    environmentSha256: snapshot.environmentSha256,
    pid: 104,
    parentPid: 103,
    sessionId: 102,
    processGroupId: 102,
    spawnObservedMonotonicNs: "2",
    startedNs: "35",
    endedNs: "45",
    actualExit: 0,
    signal: null,
    reaped: true,
    rawFdsClosed: true,
    errors: [],
  };
  const rustupResolutionQueries = (["cargo", "rustc"] as const).flatMap((name, index) => [
    owned(
      "tool-query",
      `rustup-${name}`,
      [snapshot.toolLaunchers[name].resolvedPath, "which", name],
      108 + index,
      `probe-rustup-${name}`,
      `${tool(name).path}\n`,
    ),
    owned(
      "tool-query",
      `rustup-${name}-after`,
      [snapshot.toolLaunchers[name].resolvedPath, "which", name],
      110 + index,
      `probe-rustup-${name}`,
      `${tool(name).path}\n`,
      environment,
      { startedNs: "46", endedNs: "48" },
    ),
  ]);
  const pins = {
    source: source.files,
    cargo: tool("cargo"),
    rustc: tool("rustc"),
    cargoLauncher: snapshot.toolLaunchers.cargo,
    rustcLauncher: snapshot.toolLaunchers.rustc,
    guardCodePins,
    capacityAdmission: capacity,
    rustupSelectedRustc: tool("rustc"),
    unsupportedCargoConfigsAbsent: absentConfigs,
  };
  const measurement = {
    ...policy,
    state: capacity.state.path,
    current_bytes: 1,
    over_capacity: false,
    metric_available: true,
  };
  const report = {
    ...measurement,
    removed: [],
    cleanup_attempts: [],
    cleanup_status: "not-needed",
    actual_removed_bytes: 0,
    post_clean_bytes: null,
    remaining_over_capacity: false,
    baseline_eligible: false,
  };
  const observation = managedObservationSchema.parse({
    schema: "krr-native-cli-guard-observation-v2",
    status: "cargo-completed",
    diagnosticOnly: true,
    accepted: false,
    attemptToken: token,
    request: requestPin,
    sourceManifest: sourcePin,
    sourceDigest: source.digest,
    metadata: metadataPin,
    metadataObservation,
    expectedPackage,
    expectedTarget,
    cargo: tool("cargo"),
    rustc: tool("rustc"),
    cargoLauncher: snapshot.toolLaunchers.cargo,
    rustcLauncher: snapshot.toolLaunchers.rustc,
    rustupResolutionQueries,
    guardCodePins,
    pinsBefore: pins,
    pinsAfter: pins,
    capacityBefore: json(`tmp/${token}/capacity.before.json`, report),
    capacityAfter: json(`tmp/${token}/capacity.after.json`, measurement),
    capacityProtection: capacity,
    ownedCargo: cargo,
    binaryBefore: null,
    binaryAfter: binary,
    artifact,
    buildFinished: finished,
    errors: [],
    externalGroupClosureRequired: true,
    scope: "synthetic reader unit graph; not native execution",
  });
  const observationPin = json(`tmp/${token}/observation.json`, observation);
  const preflightReport = {
    ...policy,
    state: capacity.state.path,
    current_bytes: 1,
    over_capacity: false,
    metric_available: true,
    removed: [],
    cleanup_status: "not-needed",
    actual_removed_bytes: 0,
    post_clean_bytes: null,
    remaining_over_capacity: false,
    baseline_eligible: false,
  };
  const preflightArgv = [
    tool("python3").path,
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
  const preflightRun = (label: string, stdoutReport: Record<string, unknown>, pid: number) =>
    owned("preflight", label, preflightArgv, pid, label, JSON.stringify(stdoutReport));
  const preflightRecord = (
    schema: string,
    run: ReturnType<typeof preflightRun>,
    report: Record<string, unknown>,
  ) => ({
    schema,
    argv: run.observation.argv,
    cwd: run.observation.cwd,
    actualExit: run.observation.actualExit,
    reaped: run.observation.reaped,
    stdout: run.observation.stdout,
    stderr: run.observation.stderr,
    lifecycle: run.lifecycle,
    startedNs: run.observation.startedNs,
    endedNs: run.observation.endedNs,
    report,
  });
  let cleanupObservation: FileRecord | null = null;
  let refreshObservation: FileRecord | null = null;
  let acceptedRun = preflightRun("capacity-preflight", preflightReport, 106);
  if (withCapacityCleanup) {
    const cleanupReport = {
      ...policy,
      state: capacity.state.path,
      current_bytes: 4726096872,
      over_capacity: true,
      metric_available: true,
      removed: ["finite artifact eviction"],
      cleanup_status: "incomplete",
      clean_exitcode: 0,
      post_clean_bytes: 4726094872,
      remaining_over_capacity: false,
      cleanup_attempts: [{ exit_code: 0, dry_run: false }],
      finite_cleanup: {},
    };
    const cleanupRun = preflightRun("capacity-preflight-cleanup", cleanupReport, 107);
    const cleanupEvidence = preflightRecord(
      "krr-native-cli-capacity-preflight-cleanup-v1",
      cleanupRun,
      cleanupReport,
    );
    cleanupObservation = json(
      `tmp/${token}/capacity-preflight-cleanup-observation.json`,
      cleanupEvidence,
    );
    const refreshRun = preflightRun("capacity-preflight-refresh", preflightReport, 108);
    const refreshEvidence = preflightRecord(
      "krr-native-cli-capacity-preflight-refresh-v1",
      refreshRun,
      preflightReport,
    );
    refreshObservation = json(
      `tmp/${token}/capacity-preflight-refresh-observation.json`,
      refreshEvidence,
    );
    acceptedRun = refreshRun;
  }
  const preflightEvidence = managedCapacityPreflightSchema.parse({
    ...preflightRecord("krr-native-cli-capacity-preflight-v1", acceptedRun, preflightReport),
    cleanupObservation,
    refreshObservation,
  });
  const capacityPreflight = json(`tmp/${token}/capacity-preflight.json`, preflightEvidence);
  const toolProbes = (["cargo", "rustc"] as const).map((name, index) =>
    owned(
      "tool-query",
      name,
      [tool("rtk").path, "proxy", snapshot.toolLaunchers[name].path,
        name === "cargo" ? "--version" : "-Vv"],
      105 + index,
      `probe-${3 + index}-${name}`,
      "unit version output",
    ),
  );
  const hostPrebuildEvidence = owned(
    "build",
    "host-prebuild",
    [tool("rtk").path, "proxy", tool("just").path, "rust-facts-host-prebuild"],
    100,
    "rust-facts-host-prebuild",
    "",
    environment,
    { startedNs: "25", endedNs: "29" },
  );
  const fixtureHost = hostFixtureInputs(root, environment);
  const hostMetadata = fixtureHost.metadata;
  const hostTools = Object.fromEntries(
    Object.entries(fixtureHost.tools).map(([name]) => [
      name,
      {
        launcher: tool(name),
        executable: tool(name),
      },
    ]),
  ) as ReturnType<typeof hostFixtureInputs>["tools"];
  const hostInputs = currentRustFactsHostInputs(root, hostMetadata, hostTools, environment);
  const hostReceiptPath = path.join(root, "tmp/quality/rust-facts-host/current/receipt.json");
  const hostAttempt = "00000000-0000-4000-8000-000000000002";
  createRustFactsHostAttemptDirectory(root, hostReceiptPath, hostAttempt);
  const hostBinarySource = save(
    "target/debug/mathjax_consumer_facts",
    "unexecuted Rust facts host unit binary data",
  );
  const hostBuildOutput = hostBuildOutputRecord(hostBinarySource.path);
  const hostBinary = publishRustFactsHostBinary(
    root,
    hostReceiptPath,
    hostAttempt,
    hostBuildOutput,
  );
  const hostProof = (label: string, suffix: string) => {
    const stdout = save(
      `tmp/${token}/rust-facts-host/${suffix}.stdout`,
      JSON.stringify(hostMetadata),
    );
    const stderr = save(`tmp/${token}/rust-facts-host/${suffix}.stderr`, "");
    const proof = {
      argv: [hostTools.just.executable.path, label, root],
      cwd: root,
      exitCode: 0,
      signal: null,
      error: null,
      stdout,
      stderr,
    };
    save(`tmp/${token}/rust-facts-host/${suffix}.process.json`, JSON.stringify(proof));
    return proof;
  };
  const hostValue = rustFactsHostReceiptSchema.parse({
    schema: "krr-rust-facts-host/v2",
    status: "prepared",
    root,
    attempt: hostAttempt,
    inputs: { ...hostInputs, environmentSha256: hostEnvironmentDigest(root, environment) },
    buildOutput: hostBuildOutput,
    binary: hostBinary,
    metadataBefore: hostProof("rust-facts-host-metadata", "metadata-before"),
    build: hostProof("rust-facts-host-build", "build"),
    metadataAfter: hostProof("rust-facts-host-metadata", "metadata-after"),
  });
  fs.mkdirSync(path.dirname(hostReceiptPath), { recursive: true });
  fs.writeFileSync(hostReceiptPath, JSON.stringify(hostValue));
  const hostReceipt = hostFileRecord(hostReceiptPath);
  const hostPrebuild = {
    ...hostPrebuildEvidence,
    receipt: hostReceipt,
  };
  const receipt = managedReceiptSchema.parse({
    schema: "krr-native-cli-managed-build-v3",
    status: "completed",
    accepted: true,
    attemptToken: token,
    cwd: root,
    expectedSource,
    expectedProtectedRegistry: registry,
    request: requestPin,
    capacityPreflight,
    observation: observationPin,
    metadataLifecycle: metadata.lifecycle,
    hostPrebuild,
    buildLifecycle: build.lifecycle,
    buildObservation: build.observation,
    snapshotBefore: snapshot,
    snapshotAfter: snapshot,
    binary,
    runtimeAssetPins: [runtime],
    toolProbes,
    scope: {
      sourceEndpointsOnly: true,
      effectiveCompilerReadsProved: false,
      registryAtomicityProved: false,
      installedDependencyClosureProved: false,
      descendantClosureProved: false,
    },
  });
  const receiptPath = json(`tmp/${token}/receipt.json`, receipt).path;
  return {
    root,
    receipt,
    request,
    hostReceiptPath,
    observation,
    receiptPath,
    binaryPath: binary.path,
    runtimePath: runtime.path,
  };
}
