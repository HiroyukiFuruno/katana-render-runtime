import fs from "node:fs";
import path from "node:path";
import type { FileRecord } from "./native-build-receipt";
import { createManagedReaderInputs } from "./native-cli-managed-build-input-test-fixture";
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
import {
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
export function createManagedReaderFixture() {
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
      tool("cargo").path,
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
    schema: "krr-native-cli-guard-request-v1",
    attemptToken: token,
    cwd: root,
    target: policy.target,
    binaryPath: path.join(root, "target/debug/krr"),
    issuedMonotonicNs: "1",
    expiresMonotonicNs: "120000000001",
    cargo: tool("cargo"),
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
      "--set",
      "CARGO",
      tool("cargo").path,
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
      tool("cargo").path,
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
  const pins = {
    source: source.files,
    cargo: tool("cargo"),
    guardCodePins,
    capacityAdmission: capacity,
    observedPathRustc: tool("rustc"),
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
    cleanup_status: "not-needed",
    actual_removed_bytes: 0,
    post_clean_bytes: null,
    remaining_over_capacity: false,
    baseline_eligible: false,
  };
  const observation = managedObservationSchema.parse({
    schema: "krr-native-cli-guard-observation-v1",
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
  const toolProbes = ["cargo", "rustc"].map((name, index) =>
    owned(
      "tool-query",
      name,
      [tool("rtk").path, "proxy", tool(name).path, name === "cargo" ? "--version" : "-Vv"],
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
  const fixtureHost = hostFixtureInputs(root);
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
  const hostInputs = currentRustFactsHostInputs(root, hostMetadata, hostTools);
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
    save(
      `tmp/${token}/rust-facts-host/${suffix}.process.json`,
      JSON.stringify(proof),
    );
    return proof;
  };
  const hostValue = rustFactsHostReceiptSchema.parse({
    schema: "krr-rust-facts-host/v2",
    status: "prepared",
    root,
    attempt: hostAttempt,
    inputs: { ...hostInputs, environmentSha256: hostEnvironmentDigest(root) },
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
    schema: "krr-native-cli-managed-build-v1",
    status: "completed",
    accepted: true,
    attemptToken: token,
    cwd: root,
    expectedSource,
    expectedProtectedRegistry: registry,
    request: requestPin,
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
    observation,
    receiptPath,
    binaryPath: binary.path,
    runtimePath: runtime.path,
  };
}
