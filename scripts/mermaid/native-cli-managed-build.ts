import fs from "node:fs";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";
import { fileSha256 } from "./mermaid-pair-manifest";
import {
  canonicalRegularFile,
  type NativeBuildReceiptSummary,
  verifyCurrentBinaryIdentity,
  verifyFileRecord,
} from "./native-build-receipt";
import {
  managedObservationSchema,
  managedReceiptSchema,
  managedRequestSchema,
  managedSourceSchema,
  type ManagedReceipt,
  type ManagedRequest,
} from "./native-cli-managed-build-schema";
import { verifyManagedCargo } from "./native-cli-managed-cargo";
import { verifyManagedInputs } from "./native-cli-managed-inputs";
import { verifyRustFactsHost } from "../runtime-bundles/runtime-rust-facts-host";
import {
  owned,
  read,
  requireBound,
  same,
  verifyManagedProcessEnvironment,
  verifyManagedRoles,
} from "./native-cli-managed-process";

export function verifyManagedNativeBuildReceipt(
  value: unknown,
  receiptPath: string,
  expectedBinary: string,
  expectedRuntime: string,
  expectedRoot: string,
): NativeBuildReceiptSummary {
  const receipt = managedReceiptSchema.parse(value);
  requireBound(
    receipt.cwd === path.resolve(expectedRoot) && fs.realpathSync(receipt.cwd) === receipt.cwd,
    "working directory differs",
  );
  const attempt = path.dirname(receiptPath);
  requireBound(
    path.basename(attempt) === receipt.attemptToken && attempt.startsWith(`${receipt.cwd}/tmp/`),
    "attempt path differs",
  );
  requireBound(
    receipt.request.path === path.join(attempt, "request.json") &&
      receipt.observation.path === path.join(attempt, "observation.json") &&
      receipt.expectedSource.path.startsWith(`${receipt.cwd}/tmp/`) &&
      receipt.expectedProtectedRegistry.path.startsWith(`${receipt.cwd}/tmp/`),
    "admission/request path boundary differs",
  );
  verifyManagedInputs(receipt);
  const request = managedRequestSchema.parse(read(receipt.request));
  const observation = managedObservationSchema.parse(read(receipt.observation));
  verifyManagedRoles(receipt, request, observation, receiptPath);
  same(
    observation.pinsAfter.unsupportedCargoConfigsAbsent,
    verifyManagedProcessEnvironment(receipt),
    "Cargo configuration absence inventory",
  );
  requireBound(
    receipt.request.path === path.join(attempt, "request.json") &&
      receipt.observation.path === path.join(attempt, "observation.json"),
    "request/observation role differs",
  );
  same(request.attemptToken, receipt.attemptToken, "request nonce");
  same(observation.request, receipt.request, "observed request pin");
  same(observation.attemptToken, receipt.attemptToken, "observation nonce");
  for (const key of [
    "sourceManifest",
    "sourceDigest",
    "metadata",
    "metadataObservation",
    "expectedPackage",
    "expectedTarget",
    "cargo",
    "guardCodePins",
  ] as const)
    same(observation[key], request[key], key);
  requireBound(
    request.cwd === receipt.cwd &&
      request.target === path.join(receipt.cwd, "target") &&
      request.binaryPath === canonicalRegularFile(expectedBinary, "managed CLI binary") &&
      request.binaryPath === path.join(receipt.cwd, "target/debug/krr") &&
      request.expectedPackage.manifestPath ===
        path.join(receipt.cwd, "crates/katana-render-runtime-cli/Cargo.toml") &&
      request.expectedTarget.srcPath ===
        path.join(receipt.cwd, "crates/katana-render-runtime-cli/src/main.rs") &&
      BigInt(request.expiresMonotonicNs) - BigInt(request.issuedMonotonicNs) === 120000000000n &&
      BigInt(observation.ownedCargo.spawnObservedMonotonicNs) >=
        BigInt(request.issuedMonotonicNs) &&
      BigInt(observation.ownedCargo.spawnObservedMonotonicNs) < BigInt(request.expiresMonotonicNs),
    "request path/deadline differs",
  );
  same(request.environment, receipt.snapshotAfter.environment, "request child environment");
  same(
    request.environmentSha256,
    receipt.snapshotAfter.environmentSha256,
    "request environment digest",
  );
  const outerEnvironment: { [key: string]: string } = {
    ...request.environment,
    KRR_NATIVE_CLI_GUARD_REQUEST: receipt.request.path,
    KRR_NATIVE_CLI_GUARD_REQUEST_SHA256: receipt.request.sha256,
  };
  delete outerEnvironment.PWD;
  delete outerEnvironment._;
  same(receipt.buildObservation.environment, outerEnvironment, "exact outer Just environment");
  requireBound(
    receipt.buildObservation.attemptToken === receipt.attemptToken &&
      receipt.buildObservation.cwd === receipt.cwd &&
      receipt.buildObservation.label === "probe-6-native-cli-build",
    "outer build attempt binding differs",
  );
  verifyHostPrebuild(receipt, request);
  same(request.cargo, receipt.snapshotAfter.tools.cargo, "request Cargo tool");
  requireBound(/^\/[a-z0-9_@%+=:,./-]+$/i.test(request.cargo.path), "Cargo shell path differs");
  same(request.capacityAdmission, receipt.snapshotAfter.capacityAdmission, "request capacity");
  same(observation.capacityProtection, request.capacityAdmission, "observed capacity");
  same(
    request.guardCodePins,
    [
      "cargo_capacity_guard.py",
      "cargo_capacity_guard_lock.py",
      "cargo_capacity_guard_native_cli_observation.py",
    ].map((name) =>
      receipt.snapshotAfter.control.find(
        (pin) => pin.path === path.join(receipt.cwd, "scripts/build", name),
      ),
    ),
    "guard code inventory",
  );
  const source = managedSourceSchema.parse(read(request.sourceManifest));
  same(source, managedSourceSchema.parse(read(receipt.expectedSource)), "request source admission");
  requireBound(
    source.digest === request.sourceDigest &&
      request.sourceManifest.path === path.join(attempt, "source-manifest.json"),
    "source manifest role/digest differs",
  );
  verifyManagedCargo(receipt, request, observation);
  same(receipt.binary, observation.binaryAfter, "observed binary identity");
  verifyFileRecord(receipt.binary, "managed binary");
  verifyCurrentBinaryIdentity(receipt.binary);
  requireBound(
    receipt.runtimeAssetPins.some(
      (pin) =>
        pin.path === canonicalRegularFile(expectedRuntime, "selected runtime") &&
        source.files.some((row) => isDeepStrictEqual(row, pin)),
    ),
    "selected runtime absent from source",
  );
  for (const [index, probe] of receipt.toolProbes.entries()) {
    owned(probe.observation, probe.lifecycle, "tool-query");
    requireBound(
      probe.observation.attemptToken === receipt.attemptToken &&
        probe.observation.cwd === receipt.cwd,
      "tool probe attempt binding differs",
    );
    same(probe.observation.environment, request.environment, "tool probe environment");
    const name = index === 0 ? "cargo" : "rustc";
    same(
      probe.observation.argv,
      [
        receipt.snapshotAfter.tools.rtk.path,
        "proxy",
        receipt.snapshotAfter.tools[name].path,
        name === "cargo" ? "--version" : "-Vv",
      ],
      "tool probe argv",
    );
    requireBound(fs.statSync(probe.observation.stdout.path).size > 0, "empty tool version");
  }
  const cargoLock = source.files.find((pin) => pin.path === path.join(receipt.cwd, "Cargo.lock"));
  requireBound(cargoLock !== undefined, "Cargo.lock absent");
  if (cargoLock === undefined) throw new Error("Cargo.lock absent");
  return {
    receiptPath,
    receiptSha256: fileSha256(receiptPath),
    sourceManifestPath: request.sourceManifest.path,
    sourceManifestSha256: request.sourceManifest.sha256,
    sourceDigest: source.digest,
    sourceFileCount: source.files.length,
    sourceFiles: source.files,
    binaryPath: receipt.binary.path,
    binarySha256: receipt.binary.sha256,
    cargoLockPath: cargoLock.path,
    cargoLockSha256: cargoLock.sha256,
    runtimeAssetPins: receipt.runtimeAssetPins,
  };
}

function verifyHostPrebuild(receipt: ManagedReceipt, request: ManagedRequest): void {
  const proof = receipt.hostPrebuild.observation;
  owned(proof, receipt.hostPrebuild.lifecycle, "build");
  const expectedReceiptPath = path.join(receipt.cwd, "tmp/quality/rust-facts-host/current/receipt.json");
  requireBound(
    receipt.hostPrebuild.receipt.path === expectedReceiptPath,
    "host prebuild receipt path differs",
  );
  verifyFileRecord(receipt.hostPrebuild.receipt, "managed Rust facts host receipt");
  verifyRustFactsHost(receipt.cwd, receipt.hostPrebuild.receipt.path);
  same(proof.environment, request.environment, "host prebuild environment");
  same(
    proof.argv,
    [
      receipt.snapshotAfter.tools.rtk.path,
      "proxy",
      receipt.snapshotAfter.tools.just.path,
      "rust-facts-host-prebuild",
    ],
    "host prebuild argv",
  );
  requireBound(
    proof.attemptToken === receipt.attemptToken &&
      proof.cwd === receipt.cwd &&
      proof.label === "rust-facts-host-prebuild" &&
      BigInt(proof.endedNs) < BigInt(receipt.buildObservation.startedNs),
    "host prebuild order/attempt differs",
  );
}
