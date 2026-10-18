import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileSha256, type JsonValue, parseJsonValue, sha256 } from "./mermaid-pair-manifest";
import { verifyManagedNativeBuildReceipt } from "./native-cli-managed-build";
import { managedBuildSchemaName } from "./native-cli-managed-build-schema";
import { declaresManagedReceipt, parseManagedJson } from "./native-cli-managed-json";

export interface NativeBuildReceiptSummary {
  receiptPath: string;
  receiptSha256: string;
  sourceManifestPath: string;
  sourceManifestSha256: string;
  sourceDigest: string;
  sourceFileCount: number;
  sourceFiles: FileRecord[];
  binaryPath: string;
  binarySha256: string;
  cargoLockPath: string;
  cargoLockSha256: string;
  runtimeAssetPins: FileRecord[];
}

export interface FileRecord {
  path: string;
  sha256: string;
  size: number;
}

export function validateNativeBuildArtifactEnvelope(
  value: JsonValue | undefined,
  attemptToken: string,
  sourceManifestSha256: string,
  sourceDigest: string,
): void {
  const proof = objectValue(value, "Cargo compiler artifact proof");
  if (
    stringValue(proof.schema, "Cargo artifact schema") !== "krr-native-cargo-artifact-v1" ||
    stringValue(proof.attemptToken, "Cargo artifact attempt token") !== attemptToken ||
    stringValue(proof.sourceManifestSha256, "Cargo artifact source manifest SHA") !==
      sourceManifestSha256 ||
    stringValue(proof.sourceDigest, "Cargo artifact source digest") !== sourceDigest
  ) {
    throw new Error("Cargo artifact proof is not bound to this source/build attempt");
  }
  const observationPin = fileRecord(proof.metadataObservation, "Cargo metadata observation");
  verifyFileRecord(observationPin, "Cargo metadata observation");
  const observation = objectValue(
    parseJsonValue(fs.readFileSync(observationPin.path, "utf8"), "Cargo metadata observation"),
    "Cargo metadata observation",
  );
  validateWriterProbeLabel(stringValue(observation.label, "Cargo metadata process label"));
}

interface BuildSourceManifest {
  schema: "krr-native-build-source-manifest-v1";
  digest: string;
  files: FileRecord[];
}

export function verifyNativeBuildReceipt(
  receiptPath: string,
  expectedBinaryPath: string,
  expectedRuntimePath: string,
  expectedWorkingDirectory = process.cwd(),
): NativeBuildReceiptSummary {
  const canonicalReceiptPath = canonicalRegularFile(receiptPath, "build receipt");
  const receiptSha256 = fileSha256(canonicalReceiptPath);
  const serialized = fs.readFileSync(canonicalReceiptPath, "utf8");
  const root = objectValue(
    parseJsonValue(serialized, "native build receipt"),
    "native build receipt",
  );
  if (
    root.schema === managedBuildSchemaName ||
    declaresManagedReceipt(serialized, managedBuildSchemaName)
  ) {
    const summary = verifyManagedNativeBuildReceipt(
      parseManagedJson(serialized, "managed native build receipt"),
      canonicalReceiptPath,
      expectedBinaryPath,
      expectedRuntimePath,
      expectedWorkingDirectory,
    );
    if (summary.receiptSha256 !== receiptSha256) {
      throw new Error("Managed native build receipt changed during verification");
    }
    return summary;
  }
  assertReceiptStatus(root);

  const build = objectValue(root.build, "build");
  const workingDirectory = validateOwnedBuild(build, expectedWorkingDirectory);
  validateBuildCommand(build);
  const argv = stringArray(build.argv, "build argv");
  if (argv.length === 0) throw new Error("Native build receipt does not contain build arguments");
  const stdout = fileRecord(build.stdout, "build stdout");
  const stderr = fileRecord(build.stderr, "build stderr");
  verifyFileRecord(stdout, "build stdout");
  verifyFileRecord(stderr, "build stderr");
  stringArray(build.flags, "build flags");
  const environment = stringRecord(build.environment, "build environment");
  if (
    shaField(build.environmentSha256, "build environment SHA") !== digestStringRecord(environment)
  ) {
    throw new Error("Native build environment digest is invalid");
  }

  const binary = fileRecord(root.binary, "binary");
  verifyFileRecord(binary, "native CLI binary");
  const canonicalBinaryPath = canonicalRegularFile(expectedBinaryPath, "native CLI binary");
  if (binary.path !== canonicalBinaryPath) {
    throw new Error("Native build receipt points to a different CLI binary");
  }

  const cargoLock = fileRecord(root.cargoLock, "Cargo.lock");
  verifyFileRecord(cargoLock, "Cargo.lock");
  if (cargoLock.path !== path.join(workingDirectory, "Cargo.lock")) {
    throw new Error("Native build receipt points to a different Cargo.lock");
  }

  const toolchain = objectValue(root.toolchain, "toolchain");
  const rustc = fileRecord(toolchain.rustc, "rustc");
  const cargo = fileRecord(toolchain.cargo, "cargo");
  verifyFileRecord(rustc, "rustc toolchain");
  verifyFileRecord(cargo, "Cargo toolchain");
  if (path.basename(rustc.path) !== "rustc" || path.basename(cargo.path) !== "cargo") {
    throw new Error("Native build receipt must pin resolved rustc and Cargo executables");
  }
  nonemptyString(toolchain.rustcVersionVv, "rustc version");
  nonemptyString(toolchain.cargoVersion, "Cargo version");
  validateCapacityReport(root.capacityPreflight, "capacity preflight", workingDirectory);
  validateCapacityReport(root.capacityPost, "capacity postflight", workingDirectory);
  validateBinaryProvenance(root.binaryProvenance, binary, workingDirectory, stdout, stderr);

  const sourceSnapshot = validateBuildSource(root.source, cargoLock);
  const { manifestPath, manifestSha256, sourceDigest, sourceFiles } = sourceSnapshot;
  const manifest = parseSourceManifest(manifestPath);
  assertCurrentGitSourceSet(workingDirectory, manifest.files);
  const runtimeAssetPins = fileRecordArray(root.runtimeAssetPins, "runtime asset pins");
  if (runtimeAssetPins.length === 0) {
    throw new Error("Native build receipt has no runtime asset pins");
  }
  for (const item of runtimeAssetPins) verifyFileRecord(item, "runtime asset pin");
  const canonicalRuntimePath = canonicalRegularFile(expectedRuntimePath, "Mermaid runtime");
  if (!runtimeAssetPins.some((item) => item.path === canonicalRuntimePath)) {
    throw new Error("Native build receipt does not pin the selected Mermaid runtime");
  }
  const snapshotBindings = validateReceiptCodeAndSnapshots(
    root,
    sourceFiles,
    toolchain,
    runtimeAssetPins,
    cargoLock,
    binary,
  );
  const { codePins, observedTools } = snapshotBindings;
  const buildArgv = stringArray(build.argv, "build argv");
  if (buildArgv[0] !== observedTools.rtk.path || buildArgv[2] !== observedTools.just.path) {
    throw new Error("Owned krr-build command differs from the pinned rtk/just resolutions");
  }
  const compilerArtifact = objectValue(root.compilerArtifact, "Cargo compiler artifact proof");
  const observerRequest = fileRecord(compilerArtifact.request, "Cargo observer request");
  verifyFileRecord(observerRequest, "Cargo observer request");
  const protectionEnvironment = validateNativeCapacityProtection(
    root.capacityProtection,
    build,
    compilerArtifact,
    parseObserverRequest(observerRequest.path),
  );
  validateProducerEnvironment(
    environment,
    observerRequest,
    codePins,
    observedTools.python3,
    protectionEnvironment,
  );
  validateCompilerArtifact(
    root.compilerArtifact,
    build,
    binary,
    cargo,
    sourceFiles,
    manifestPath,
    manifestSha256,
    sourceDigest,
    workingDirectory,
    toolchain,
    runtimeAssetPins,
    environment,
    codePins,
    observedTools.rtk,
  );

  return {
    receiptPath: canonicalReceiptPath,
    receiptSha256,
    sourceManifestPath: manifestPath,
    sourceManifestSha256: manifestSha256,
    sourceDigest,
    sourceFileCount: manifest.files.length,
    sourceFiles: manifest.files,
    binaryPath: binary.path,
    binarySha256: binary.sha256,
    cargoLockPath: cargoLock.path,
    cargoLockSha256: cargoLock.sha256,
    runtimeAssetPins,
  };
}

function validateReceiptCodeAndSnapshots(
  root: { [key: string]: JsonValue },
  sourceFiles: FileRecord[],
  toolchain: { [key: string]: JsonValue },
  runtimeAssetPins: FileRecord[],
  cargoLock: FileRecord,
  binary: FileRecord,
): {
  codePins: FileRecord[];
  observedTools: { rtk: FileRecord; just: FileRecord; python3: FileRecord };
} {
  const codePins = fileRecordArray(root.codePins, "reviewed producer code pins");
  if (codePins.length === 0)
    throw new Error("Native build receipt has no reviewed producer code pins");
  for (const item of codePins) verifyFileRecord(item, "reviewed producer code pin");
  const reviewedCodeManifest = fileRecord(
    root.reviewedCodeManifest,
    "reviewed producer code manifest",
  );
  verifyReviewedCodeManifest(reviewedCodeManifest, codePins);
  const observedTools = validateObservationSnapshots(
    root.observationPins,
    sourceFiles,
    toolchain,
    runtimeAssetPins,
    codePins,
    reviewedCodeManifest,
    cargoLock,
    binary,
  );
  return { codePins, observedTools };
}

function validateCompilerArtifact(
  value: JsonValue | undefined,
  build: { [key: string]: JsonValue },
  binary: FileRecord,
  cargo: FileRecord,
  sourceFiles: FileRecord[],
  sourceManifestPath: string,
  sourceManifestSha256: string,
  sourceDigest: string,
  workingDirectory: string,
  rootToolchain: { [key: string]: JsonValue },
  rootRuntimePins: FileRecord[],
  buildEnvironment: { [key: string]: string },
  rootCodePins: FileRecord[],
  observedRtk: FileRecord,
) {
  const attemptToken = stringValue(build.attemptToken, "build attempt token");
  validateNativeBuildArtifactEnvelope(value, attemptToken, sourceManifestSha256, sourceDigest);
  const proof = objectValue(value, "Cargo compiler artifact proof");
  const observerReceipt = fileRecord(proof.observerReceipt, "Cargo observer receipt");
  const request = fileRecord(proof.request, "Cargo observer request");
  const rawJson = fileRecord(proof.rawJson, "Cargo JSON output");
  const rawStderr = fileRecord(proof.rawStderr, "Cargo stderr output");
  const observedCargo = fileRecord(proof.cargo, "observed Cargo executable");
  const metadata = fileRecord(proof.metadata, "Cargo metadata output");
  const metadataObservation = fileRecord(proof.metadataObservation, "Cargo metadata observation");
  verifyFileRecord(observerReceipt, "Cargo observer receipt");
  verifyFileRecord(request, "Cargo observer request");
  verifyFileRecord(rawJson, "Cargo JSON output");
  verifyFileRecord(rawStderr, "Cargo stderr output");
  verifyFileRecord(observedCargo, "observed Cargo executable");
  verifyFileRecord(metadata, "Cargo metadata output");
  verifyFileRecord(metadataObservation, "Cargo metadata observation");
  if (!sameRecord(observedCargo, cargo)) {
    throw new Error("Cargo artifact proof uses a different Cargo executable");
  }
  const expectedPackage = objectValue(proof.expectedPackage, "expected Cargo package");
  const expectedPackageId = stringValue(expectedPackage.id, "expected package id");
  const expectedManifestPath = canonicalRegularFile(
    stringValue(expectedPackage.manifestPath, "expected package manifest"),
    "expected package manifest",
  );
  if (
    stringValue(expectedPackage.name, "expected package name") !== "katana-render-runtime-cli" ||
    !sourceFiles.some((item) => item.path === expectedManifestPath)
  ) {
    throw new Error("Cargo artifact proof does not identify the pinned CLI package manifest");
  }
  const expectedTarget = objectValue(proof.expectedTarget, "expected Cargo target");
  const targetPath = path.join(workingDirectory, "crates/katana-render-runtime-cli/src/main.rs");
  if (
    stringValue(expectedTarget.name, "expected target name") !== "krr" ||
    !sameStringArray(stringArray(expectedTarget.kind, "expected target kind"), ["bin"]) ||
    stringValue(expectedTarget.srcPath, "expected target source") !== targetPath ||
    !sourceFiles.some((item) => item.path === targetPath)
  ) {
    throw new Error("Cargo artifact proof does not identify the expected krr binary target");
  }
  const ownedCargo = objectValue(proof.ownedCargo, "owned Cargo process");
  const artifact = objectValue(proof.artifact, "Cargo compiler artifact event");
  const buildFinished = objectValue(proof.buildFinished, "Cargo build-finished event");
  if (
    stringValue(artifact.reason, "Cargo artifact reason") !== "compiler-artifact" ||
    stringValue(artifact.package_id, "Cargo artifact package id") !== expectedPackageId ||
    typeof artifact.fresh !== "boolean" ||
    stringValue(buildFinished.reason, "Cargo completion reason") !== "build-finished" ||
    buildFinished.success !== true
  ) {
    throw new Error("Cargo artifact proof is missing a successful matching artifact event");
  }
  validateCargoArtifactEvents(
    artifact,
    buildFinished,
    rawJson,
    expectedPackageId,
    targetPath,
    binary,
  );
  validateObserverBindings(
    proof,
    observerReceipt,
    request,
    rawJson,
    rawStderr,
    observedCargo,
    metadata,
    metadataObservation,
    sourceManifestPath,
    sourceManifestSha256,
    sourceDigest,
    attemptToken,
    expectedPackage,
    expectedTarget,
    ownedCargo,
    artifact,
    buildFinished,
    binary,
    workingDirectory,
    sourceFiles,
    rootToolchain,
    rootRuntimePins,
    buildEnvironment,
    rootCodePins,
    observedRtk,
  );
}

function validateCargoArtifactEvents(
  artifact: { [key: string]: JsonValue },
  buildFinished: { [key: string]: JsonValue },
  rawJson: FileRecord,
  expectedPackageId: string,
  targetPath: string,
  binary: FileRecord,
) {
  const actualTarget = objectValue(artifact.target, "Cargo artifact target");
  const filenames = stringArray(artifact.filenames, "Cargo artifact filenames");
  if (
    stringValue(actualTarget.name, "Cargo artifact target name") !== "krr" ||
    !sameStringArray(stringArray(actualTarget.kind, "Cargo artifact target kind"), ["bin"]) ||
    stringValue(actualTarget.src_path, "Cargo artifact source") !== targetPath ||
    stringValue(artifact.executable, "Cargo artifact executable") !== binary.path ||
    !filenames.includes(binary.path)
  ) {
    throw new Error("Cargo artifact does not match the pinned CLI binary and target");
  }
  const events = readCargoEvents(rawJson.path);
  const artifactEvents = events.filter(
    (event) =>
      event.reason === "compiler-artifact" &&
      event.package_id === expectedPackageId &&
      isRecord(event.target) &&
      event.target.name === "krr",
  );
  const finishedEvents = events.filter((event) => event.reason === "build-finished");
  if (
    artifactEvents.length !== 1 ||
    finishedEvents.length !== 1 ||
    !sameJsonValue(artifactEvents[0], artifact) ||
    !sameJsonValue(finishedEvents[0], buildFinished)
  ) {
    throw new Error(
      "Cargo raw JSON does not contain the attested artifact and successful completion",
    );
  }
}

function validateObserverBindings(
  proof: { [key: string]: JsonValue },
  observerReceipt: FileRecord,
  request: FileRecord,
  rawJson: FileRecord,
  rawStderr: FileRecord,
  cargo: FileRecord,
  metadata: FileRecord,
  metadataObservation: FileRecord,
  sourceManifestPath: string,
  sourceManifestSha256: string,
  sourceDigest: string,
  attemptToken: string,
  expectedPackage: { [key: string]: JsonValue },
  expectedTarget: { [key: string]: JsonValue },
  ownedCargo: { [key: string]: JsonValue },
  artifact: { [key: string]: JsonValue },
  buildFinished: { [key: string]: JsonValue },
  binary: FileRecord,
  workingDirectory: string,
  sourceFiles: FileRecord[],
  rootToolchain: { [key: string]: JsonValue },
  rootRuntimePins: FileRecord[],
  buildEnvironment: { [key: string]: string },
  rootCodePins: FileRecord[],
  observedRtk: FileRecord,
) {
  const observer = parseObserverReceipt(observerReceipt.path);
  validateObserverReceiptFields(observer, {
    attemptToken,
    request: jsonFileRecord(request),
    rawJson: jsonFileRecord(rawJson),
    rawStderr: jsonFileRecord(rawStderr),
    cargo: jsonFileRecord(cargo),
    metadata: jsonFileRecord(metadata),
    metadataObservation: jsonFileRecord(metadataObservation),
    sourceManifestSha256,
    sourceDigest,
    expectedPackage,
    expectedTarget,
    ownedCargo,
    artifact,
    buildFinished,
  });
  const requestJson = parseObserverRequest(request.path);
  validateObserverRequestFields(
    requestJson,
    {
      attemptToken,
      workingDirectory,
      binary: jsonFileRecord(binary),
      sourceManifestPath,
      sourceManifestSha256,
      sourceDigest,
      metadata: jsonFileRecord(metadata),
      metadataObservation: jsonFileRecord(metadataObservation),
      cargo: jsonFileRecord(cargo),
      expectedPackage,
      expectedTarget,
    },
    buildEnvironment,
  );
  const pinsBefore = objectValue(proof.pinsBefore, "Cargo source pins before");
  const pinsAfter = objectValue(proof.pinsAfter, "Cargo source pins after");
  if (
    !sameJsonValue(pinsBefore, pinsAfter) ||
    !sameJsonValue(observer.pinsBefore, pinsBefore) ||
    !sameJsonValue(observer.pinsAfter, pinsAfter) ||
    !sameJsonValue(requestJson.pins, pinsBefore)
  ) {
    throw new Error("Cargo observer source/toolchain/runtime/code pins changed during the build");
  }
  const metadataProcess = objectValue(
    parseJsonValue(fs.readFileSync(metadataObservation.path, "utf8"), "Cargo metadata observation"),
    "Cargo metadata observation",
  );
  const childEnvironment = stringRecord(requestJson.environment, "observer Cargo environment");
  validateOwnedCargo(ownedCargo, cargo, workingDirectory, attemptToken, childEnvironment);
  const cargoStdout = fileRecord(ownedCargo.stdout, "owned Cargo stdout");
  const cargoStderr = fileRecord(ownedCargo.stderr, "owned Cargo stderr");
  verifyFileRecord(cargoStdout, "owned Cargo stdout");
  verifyFileRecord(cargoStderr, "owned Cargo stderr");
  if (!sameRecord(cargoStdout, rawJson) || !sameRecord(cargoStderr, rawStderr)) {
    throw new Error("Owned Cargo raw streams differ from the compiler event evidence");
  }
  validateMetadataObservation(
    metadataProcess,
    metadata,
    cargo,
    workingDirectory,
    attemptToken,
    observedRtk,
    childEnvironment,
  );
  validateCargoMetadata(metadata, expectedPackage, expectedTarget, workingDirectory);
  validateCompilerPins(pinsBefore, sourceFiles, rootToolchain, rootRuntimePins, rootCodePins);
}

function parseObserverReceipt(receiptPath: string): { [key: string]: JsonValue } {
  return objectValue(
    parseJsonValue(fs.readFileSync(receiptPath, "utf8"), "Cargo observer receipt"),
    "Cargo observer receipt",
  );
}

function parseObserverRequest(requestPath: string): { [key: string]: JsonValue } {
  return objectValue(
    parseJsonValue(fs.readFileSync(requestPath, "utf8"), "Cargo observer request"),
    "Cargo observer request",
  );
}

function validateObserverReceiptFields(
  observer: { [key: string]: JsonValue },
  expected: { [key: string]: JsonValue },
) {
  if (
    stringValue(observer.schema, "Cargo observer schema") !==
      "krr-native-cargo-observer-receipt-v1" ||
    stringValue(observer.status, "Cargo observer status") !== "completed" ||
    observer.diagnosticOnly !== true ||
    observer.accepted !== false ||
    stringValue(observer.attemptToken, "Cargo observer attempt token") !==
      stringValue(expected.attemptToken, "expected attempt token") ||
    !Array.isArray(observer.errors) ||
    observer.errors.length !== 0
  ) {
    throw new Error("Cargo observer receipt has an unsupported schema or status");
  }
  for (const key of [
    "request",
    "rawJson",
    "rawStderr",
    "cargo",
    "metadata",
    "metadataObservation",
  ]) {
    if (
      !sameRecord(
        fileRecord(observer[key], `observer ${key}`),
        fileRecord(expected[key], `expected ${key}`),
      )
    ) {
      throw new Error("Cargo observer receipt does not bind the same successful attempt");
    }
  }
  for (const key of [
    "sourceManifestSha256",
    "sourceDigest",
    "expectedPackage",
    "expectedTarget",
    "ownedCargo",
    "artifact",
    "buildFinished",
  ]) {
    if (!sameJsonValue(observer[key], expected[key])) {
      throw new Error("Cargo observer receipt does not bind the same successful attempt");
    }
  }
}

function validateObserverRequestFields(
  request: { [key: string]: JsonValue },
  expected: { [key: string]: JsonValue },
  buildEnvironment: { [key: string]: string },
) {
  if (
    stringValue(request.schema, "Cargo request schema") !==
      "krr-native-cargo-observer-request-v1" ||
    stringValue(request.attemptToken, "Cargo request attempt token") !==
      stringValue(expected.attemptToken, "expected attempt token") ||
    stringValue(request.cwd, "Cargo request cwd") !==
      stringValue(expected.workingDirectory, "expected working directory") ||
    stringValue(request.binaryPath, "Cargo request binary path") !==
      fileRecord(expected.binary, "expected binary").path ||
    stringValue(request.sourceManifestSha256, "request source manifest SHA") !==
      stringValue(expected.sourceManifestSha256, "expected source manifest SHA") ||
    stringValue(request.sourceDigest, "request source digest") !==
      stringValue(expected.sourceDigest, "expected source digest")
  ) {
    throw new Error("Cargo observer request does not bind the same attempt");
  }
  const manifestRecord = diskFileRecord(
    stringValue(expected.sourceManifestPath, "expected source manifest path"),
  );
  for (const [key, expectedValue] of [
    ["sourceManifest", jsonFileRecord(manifestRecord)],
    ["metadata", expected.metadata],
    ["metadataObservation", expected.metadataObservation],
    ["cargo", expected.cargo],
  ] as const) {
    if (
      !sameRecord(
        fileRecord(request[key], `request ${key}`),
        fileRecord(expectedValue, `expected ${key}`),
      )
    ) {
      throw new Error("Cargo observer request does not bind the same attempt");
    }
  }
  for (const key of ["expectedPackage", "expectedTarget"]) {
    if (!sameJsonValue(request[key], expected[key])) {
      throw new Error("Cargo observer request has a different package or target");
    }
  }
  if (
    !sameStringRecord(
      stringRecord(request.buildEnvironment, "outer build environment"),
      buildEnvironment,
    ) ||
    !sameStringRecord(
      stringRecord(request.environment, "Cargo child environment"),
      removeObserverEnvironment(buildEnvironment),
    )
  ) {
    throw new Error("Cargo observer request environments do not match their recorded scopes");
  }
}

function validateCompilerPins(
  value: { [key: string]: JsonValue },
  sourceFiles: FileRecord[],
  rootToolchain: { [key: string]: JsonValue },
  rootRuntimePins: FileRecord[],
  rootCodePins: FileRecord[],
) {
  const sourcePins = fileRecordArray(value.source, "Cargo source pins");
  const toolchainPins = objectValue(value.toolchain, "Cargo toolchain pins");
  const runtimePins = fileRecordArray(value.runtimeAssetPins, "Cargo runtime pins");
  const codePins = fileRecordArray(value.codePins, "Cargo code pins");
  if (
    !sameFileRecords(sourcePins, sourceFiles) ||
    !sameRecord(
      fileRecord(toolchainPins.rustc, "pinned rustc"),
      fileRecord(rootToolchain.rustc, "rustc"),
    ) ||
    !sameRecord(
      fileRecord(toolchainPins.cargo, "pinned Cargo"),
      fileRecord(rootToolchain.cargo, "Cargo"),
    ) ||
    stringValue(toolchainPins.rustcVersionVv, "pinned rustc version") !==
      stringValue(rootToolchain.rustcVersionVv, "rustc version") ||
    stringValue(toolchainPins.cargoVersion, "pinned Cargo version") !==
      stringValue(rootToolchain.cargoVersion, "Cargo version") ||
    !sameFileRecords(runtimePins, rootRuntimePins) ||
    !sameFileRecords(codePins, rootCodePins)
  ) {
    throw new Error("Cargo observer pins differ from the enclosing build receipt");
  }
  for (const pin of codePins) verifyFileRecord(pin, "Cargo code pin");
}

function validateCargoMetadata(
  metadata: FileRecord,
  expectedPackage: { [key: string]: JsonValue },
  expectedTarget: { [key: string]: JsonValue },
  workingDirectory: string,
) {
  const root = objectValue(
    parseJsonValue(fs.readFileSync(metadata.path, "utf8"), "Cargo metadata output"),
    "Cargo metadata output",
  );
  if (root.version !== 1) throw new Error("Cargo metadata format version is not 1");
  if (stringValue(root.workspace_root, "Cargo metadata workspace root") !== workingDirectory) {
    throw new Error("Cargo metadata output describes a different workspace");
  }
  if (!Array.isArray(root.packages)) throw new Error("Cargo metadata has no package list");
  const expectedId = stringValue(expectedPackage.id, "expected package id");
  const packages = root.packages.filter(
    (item): item is { [key: string]: JsonValue } => isRecord(item) && item.id === expectedId,
  );
  if (packages.length !== 1) {
    throw new Error("Cargo metadata does not contain exactly one expected CLI package");
  }
  const pkg = packages[0];
  if (pkg === undefined) throw new Error("Cargo metadata CLI package is missing");
  if (
    pkg.name !== stringValue(expectedPackage.name, "expected package name") ||
    pkg.manifest_path !== stringValue(expectedPackage.manifestPath, "expected package manifest")
  ) {
    throw new Error("Cargo metadata CLI package differs from its pinned identity");
  }
  if (!Array.isArray(pkg.targets)) throw new Error("Cargo metadata CLI package has no targets");
  const expectedTargetName = stringValue(expectedTarget.name, "expected target name");
  const targets = pkg.targets.filter(
    (item): item is { [key: string]: JsonValue } =>
      isRecord(item) && item.name === expectedTargetName,
  );
  if (targets.length !== 1) {
    throw new Error("Cargo metadata does not contain exactly one expected krr target");
  }
  const target = targets[0];
  if (target === undefined) throw new Error("Cargo metadata krr target is missing");
  if (
    !sameStringArray(
      stringArray(target.kind, "metadata target kind"),
      stringArray(expectedTarget.kind, "expected target kind"),
    ) ||
    stringValue(target.src_path, "metadata target source") !==
      stringValue(expectedTarget.srcPath, "expected target source")
  ) {
    throw new Error("Cargo metadata krr target differs from its pinned target");
  }
}

function validateMetadataObservation(
  observation: { [key: string]: JsonValue },
  metadata: FileRecord,
  cargo: FileRecord,
  workingDirectory: string,
  attemptToken: string,
  observedRtk: FileRecord,
  expectedEnvironment: { [key: string]: string },
) {
  const argv = stringArray(observation.argv, "Cargo metadata argv");
  const stdout = fileRecord(observation.stdout, "Cargo metadata stdout");
  const stderr = fileRecord(observation.stderr, "Cargo metadata stderr");
  verifyFileRecord(stdout, "Cargo metadata stdout");
  verifyFileRecord(stderr, "Cargo metadata stderr");
  const manifestPath = path.join(workingDirectory, "Cargo.toml");
  const label = stringValue(observation.label, "Cargo metadata process label");
  validateWriterProbeLabel(label);
  if (
    argv.length !== 10 ||
    argv[0] !== observedRtk.path ||
    argv[1] !== "proxy" ||
    argv[2] !== cargo.path ||
    argv[3] !== "metadata" ||
    argv[4] !== "--frozen" ||
    argv[5] !== "--format-version" ||
    argv[6] !== "1" ||
    argv[7] !== "--no-deps" ||
    argv[8] !== "--manifest-path" ||
    argv[9] !== manifestPath ||
    stringValue(observation.cwd, "Cargo metadata cwd") !== workingDirectory ||
    stringValue(observation.attemptToken, "Cargo metadata attempt token") !== attemptToken ||
    !sameStringRecord(
      stringRecord(observation.environment, "Cargo metadata environment"),
      expectedEnvironment,
    ) ||
    shaField(observation.environmentSha256, "Cargo metadata environment SHA") !==
      digestStringRecord(expectedEnvironment) ||
    integerValue(observation.pid, "Cargo metadata pid") < 1 ||
    integerValue(observation.actualExit, "Cargo metadata exit") !== 0 ||
    decimalBigInt(observation.endedNs, "Cargo metadata end timestamp") <
      decimalBigInt(observation.startedNs, "Cargo metadata start timestamp") ||
    observation.reaped !== true ||
    !sameRecord(stdout, metadata) ||
    !Array.isArray(observation.errors) ||
    observation.errors.length !== 0
  ) {
    throw new Error(
      "Cargo metadata process was not successfully observed for this source workspace",
    );
  }
}

function validateWriterProbeLabel(label: string) {
  if (!/^probe-[1-9][0-9]*-cargo-metadata$/.test(label)) {
    throw new Error("Cargo metadata process label does not match the writer probe convention");
  }
}

function validateOwnedCargo(
  processRecord: { [key: string]: JsonValue },
  cargo: FileRecord,
  workingDirectory: string,
  attemptToken: string,
  expectedEnvironment: { [key: string]: string },
) {
  const argv = stringArray(processRecord.argv, "owned Cargo argv");
  const startedNs = decimalBigInt(processRecord.startedNs, "owned Cargo start timestamp");
  const endedNs = decimalBigInt(processRecord.endedNs, "owned Cargo end timestamp");
  if (
    !sameStringArray(argv, [
      cargo.path,
      "build",
      "-p",
      "katana-render-runtime-cli",
      "--message-format=json-render-diagnostics",
    ]) ||
    stringValue(processRecord.cwd, "owned Cargo cwd") !== workingDirectory ||
    stringValue(processRecord.attemptToken, "owned Cargo attempt token") !== attemptToken ||
    !sameStringRecord(
      stringRecord(processRecord.environment, "owned Cargo environment"),
      expectedEnvironment,
    ) ||
    shaField(processRecord.environmentSha256, "owned Cargo environment SHA") !==
      digestStringRecord(expectedEnvironment) ||
    integerValue(processRecord.pid, "owned Cargo pid") < 1 ||
    integerValue(processRecord.actualExit, "owned Cargo exit") !== 0 ||
    endedNs < startedNs ||
    !Array.isArray(processRecord.errors) ||
    processRecord.errors.length !== 0 ||
    processRecord.reaped !== true
  ) {
    throw new Error("Cargo artifact observer did not own a successful JSON Cargo build");
  }
}

function readCargoEvents(filePath: string): Array<{ [key: string]: JsonValue }> {
  const lines = fs
    .readFileSync(filePath, "utf8")
    .split(/\r?\n/)
    .filter((line) => line.length > 0);
  return lines.map((line) =>
    objectValue(parseJsonValue(line, "Cargo JSON event"), "Cargo JSON event"),
  );
}

function sameRecord(left: FileRecord, right: FileRecord): boolean {
  return left.path === right.path && left.sha256 === right.sha256 && left.size === right.size;
}

function sameStringArray(left: string[], right: string[]): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index]);
}

function sameJsonValue(left: JsonValue | undefined, right: JsonValue | undefined): boolean {
  if (left === right) return true;
  if (left === undefined || right === undefined) return false;
  if (Array.isArray(left) && Array.isArray(right)) {
    return (
      left.length === right.length &&
      left.every((value, index) => sameJsonValue(value, right[index]))
    );
  }
  if (!isRecord(left) || !isRecord(right)) return false;
  const leftKeys = Object.keys(left).sort();
  const rightKeys = Object.keys(right).sort();
  return (
    leftKeys.length === rightKeys.length &&
    leftKeys.every((key, index) => key === rightKeys[index] && sameJsonValue(left[key], right[key]))
  );
}

function isRecord(value: JsonValue | undefined): value is { [key: string]: JsonValue } {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function assertReceiptStatus(root: { [key: string]: JsonValue }) {
  if (
    stringValue(root.schema, "schema") !== "krr-native-build-receipt-v1" ||
    stringValue(root.status, "status") !== "completed" ||
    root.diagnosticOnly !== true ||
    root.accepted !== false ||
    root.formalAccepted !== false ||
    root.childIssued !== true ||
    !Array.isArray(root.errors) ||
    root.errors.length !== 0 ||
    root.atomicSourceFreezeObserved !== false ||
    root.registryContentAttested !== false
  ) {
    throw new Error("Native build receipt has an unsupported schema or status");
  }
  nonemptyString(root.scope, "build receipt scope");
}

function validateOwnedBuild(
  build: { [key: string]: JsonValue },
  expectedWorkingDirectory: string,
): string {
  const workingDirectory = stringValue(build.cwd, "build cwd");
  const startedNs = decimalBigInt(build.startedNs, "build start timestamp");
  const endedNs = decimalBigInt(build.endedNs, "build end timestamp");
  if (
    !path.isAbsolute(workingDirectory) ||
    path.resolve(workingDirectory) !== workingDirectory ||
    workingDirectory !== path.resolve(expectedWorkingDirectory) ||
    integerValue(build.pid, "build pid") < 1 ||
    integerValue(build.actualExit, "build exit") !== 0 ||
    build.reaped !== true ||
    !Array.isArray(build.errors) ||
    build.errors.length !== 0 ||
    endedNs < startedNs
  ) {
    throw new Error("Native build receipt does not contain a successful owned build");
  }
  return workingDirectory;
}

function validateBuildCommand(build: { [key: string]: JsonValue }) {
  const argv = stringArray(build.argv, "build argv");
  const flags = stringArray(build.flags, "build flags");
  if (
    argv.length !== 4 ||
    !path.isAbsolute(argv[0] ?? "") ||
    argv[1] !== "proxy" ||
    !path.isAbsolute(argv.at(-2) ?? "") ||
    path.basename(argv.at(-2) ?? "") !== "just" ||
    argv.at(-1) !== "krr-build" ||
    !flags.includes("build") ||
    !flags.includes("-p") ||
    !flags.includes("katana-render-runtime-cli")
  ) {
    throw new Error("Native build receipt does not identify the owned krr-build target");
  }
}

interface ProtectedIdentity extends BinaryIdentity {
  mode: number;
}

function protectedIdentity(value: JsonValue | undefined, label: string): ProtectedIdentity {
  const result = {
    ...identityRecord(value, label),
    mode: integerValue(objectValue(value, label).mode, `${label} mode`),
  };
  if (result.mode > 0o7777) throw new Error(`Invalid ${label} mode`);
  verifyFileRecord(result, label);
  verifyCurrentBinaryIdentity(result);
  if ((fs.statSync(result.path).mode & 0o7777) !== result.mode) {
    throw new Error(`${label} mode changed`);
  }
  return result;
}

export function validateNativeCapacityProtection(
  value: JsonValue | undefined,
  build: { [key: string]: JsonValue },
  compilerArtifact: { [key: string]: JsonValue },
  request: { [key: string]: JsonValue },
): string | undefined {
  const ownedCargo = objectValue(compilerArtifact.ownedCargo, "protected owned Cargo");
  const requestPins = objectValue(request.pins, "protected request pins");
  const beforePins = objectValue(compilerArtifact.pinsBefore, "protected Cargo pins before");
  const afterPins = objectValue(compilerArtifact.pinsAfter, "protected Cargo pins after");
  const environments = [
    build.environment,
    request.buildEnvironment,
    request.environment,
    ownedCargo.environment,
  ].map((item) => stringRecord(item, "protected environment"));
  if (value === undefined) {
    if (
      environments.some((env) => Object.hasOwn(env, "CARGO_CAPACITY_PROTECTED_REFS")) ||
      [requestPins, beforePins, afterPins].some((pins) => Object.hasOwn(pins, "capacityProtection"))
    ) {
      throw new Error("Native capacity protection evidence is missing");
    }
    return undefined;
  }
  const proof = objectValue(value, "native capacity protection");
  const policy = objectValue(proof.capacityPolicy, "protected capacity policy");
  const expectedPolicy = {
    metric: "unique-inode-allocated-bytes",
    metric_version: 1,
    baseline_bytes: 2363047936,
    threshold_bytes: 4726095872,
    factor: 2,
    target: path.join(stringValue(build.cwd, "protected build cwd"), "target"),
  };
  if (
    proof.schema !== "krr-native-capacity-protection-v1" ||
    !sameJsonValue(policy, expectedPolicy) ||
    proof.attemptToken !== stringValue(build.attemptToken, "protected attempt token") ||
    decimalBigInt(proof.beforeNs, "protection before timestamp") >
      decimalBigInt(build.startedNs, "protected build start") ||
    decimalBigInt(proof.afterNs, "protection after timestamp") <
      decimalBigInt(build.endedNs, "protected build end")
  ) {
    throw new Error("Native capacity protection policy or build boundary differs");
  }
  const registry = protectedIdentity(proof.registryBefore, "protected registry before");
  protectedIdentity(proof.registryAfter, "protected registry after");
  if (!sameJsonValue(proof.registryBefore, proof.registryAfter)) {
    throw new Error("Native protected registry identity changed during build");
  }
  const rows = objectValue(
    parseJsonValue(fs.readFileSync(registry.path, "utf8"), "protected registry"),
    "protected registry",
  );
  const refCount = integerValue(rows.physicalRefCount, "protected registry physical count");
  if (
    rows.schema !== "krr-protected-refs-successor-v1" ||
    refCount === 0 ||
    proof.refCount !== refCount ||
    !Array.isArray(rows.refs) ||
    rows.refs.length !== refCount ||
    !Array.isArray(proof.refsBefore) ||
    proof.refsBefore.length !== refCount ||
    !Array.isArray(proof.refsAfter) ||
    proof.refsAfter.length !== refCount ||
    !sameJsonValue(proof.refsBefore, proof.refsAfter)
  ) {
    throw new Error("Native protected references are incomplete or changed");
  }
  const registryRows = rows.refs;
  const refs = proof.refsBefore.map((item, index) => {
    const actual = protectedIdentity(item, "protected reference");
    const expected = objectValue(registryRows[index], "protected registry reference");
    if (
      actual.path !== stringValue(expected.path, "registry ref path") ||
      actual.sha256 !== shaField(expected.sha256, "registry ref SHA") ||
      actual.size !== integerValue(expected.size, "registry ref size") ||
      actual.device !== integerValue(expected.dev, "registry ref device") ||
      actual.inode !== integerValue(expected.ino, "registry ref inode") ||
      actual.mode !== integerValue(expected.mode, "registry ref mode")
    ) {
      throw new Error("Native protected reference differs from the pinned registry");
    }
    return actual;
  });
  if (new Set(refs.map((ref) => ref.path)).size !== refCount) {
    throw new Error("Native protected reference paths are not unique");
  }
  const environment = JSON.stringify(refs.map((ref) => ref.path));
  if (
    proof.environmentValue !== environment ||
    shaField(proof.environmentSha256, "protection environment SHA") !==
      sha256(Buffer.from(environment, "utf8")) ||
    (proof.parentEnvironmentValue !== null && proof.parentEnvironmentValue !== environment) ||
    environments.some((env) => env.CARGO_CAPACITY_PROTECTED_REFS !== environment)
  ) {
    throw new Error("Native capacity protection environment differs from the derived registry");
  }
  const expectedPins = {
    registry: objectValue(proof.registryBefore, "protected registry pin"),
    refCount,
    refs: proof.refsBefore,
  };
  if (
    [requestPins, beforePins, afterPins].some(
      (pins) => !sameJsonValue(pins.capacityProtection, expectedPins),
    )
  ) {
    throw new Error("Native Cargo protection pins differ from the enclosing receipt");
  }
  return environment;
}

function validateProducerEnvironment(
  environment: { [key: string]: string },
  request: FileRecord,
  codePins: FileRecord[],
  python: FileRecord,
  protectionEnvironment?: string,
) {
  const producerKeys = ["CARGO", "KRR_BUILD_OBSERVER_REQUEST", "PYTHONDONTWRITEBYTECODE"];
  const cargoKeys = [
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "TERM",
    "TMPDIR",
    "SHELL",
    "RUSTFLAGS",
  ];
  if (protectionEnvironment !== undefined) {
    cargoKeys.push("CARGO_CAPACITY_PROTECTED_REFS");
    if (environment.CARGO_CAPACITY_PROTECTED_REFS !== protectionEnvironment) {
      throw new Error("Native build protection environment differs from its verified registry");
    }
  }
  const scripts = codePins.filter((item) => path.basename(item.path) === "cargo_observer.py");
  if (
    scripts.length !== 1 ||
    environment.PYTHONDONTWRITEBYTECODE !== "1" ||
    environment.KRR_BUILD_OBSERVER_REQUEST !== request.path ||
    environment.CARGO !== `${python.path} ${scripts[0]?.path}` ||
    Object.keys(environment).some((key) => !cargoKeys.includes(key) && !producerKeys.includes(key))
  ) {
    throw new Error("Native build environment does not match the pinned Cargo observer setup");
  }
  const child = removeObserverEnvironment(environment);
  if (
    child.PATH === undefined ||
    child.HOME === undefined ||
    child.RUSTFLAGS !== "-D warnings" ||
    Object.keys(child).some((key) => !cargoKeys.includes(key))
  ) {
    throw new Error("Native Cargo child environment includes unsupported or missing settings");
  }
}

function verifyReviewedCodeManifest(manifestRecord: FileRecord, codePins: FileRecord[]) {
  verifyFileRecord(manifestRecord, "reviewed producer code manifest");
  const manifest = objectValue(
    parseJsonValue(fs.readFileSync(manifestRecord.path, "utf8"), "reviewed code manifest"),
    "reviewed code manifest",
  );
  if (
    stringValue(manifest.schema, "reviewed code manifest schema") !==
      "krr-native-build-producer-code-v1" ||
    !sameFileRecords(fileRecordArray(manifest.files, "reviewed code manifest files"), codePins)
  ) {
    throw new Error("Reviewed producer code manifest differs from its physical code pins");
  }
}

function validateBuildSource(
  value: JsonValue | undefined,
  cargoLock: FileRecord,
): {
  manifestPath: string;
  manifestSha256: string;
  sourceDigest: string;
  sourceFiles: FileRecord[];
} {
  const source = objectValue(value, "build source");
  const manifestPath = canonicalRegularFile(
    stringValue(source.manifestPath, "source manifest path"),
    "build source manifest",
  );
  const manifestSha256 = shaField(source.manifestSha256, "source manifest SHA");
  if (fileSha256(manifestPath) !== manifestSha256) {
    throw new Error("Native build source manifest changed after the build");
  }
  const manifest = parseSourceManifest(manifestPath);
  const sourceDigest = shaField(source.sourceDigest, "build source digest");
  const filesBefore = fileRecordArray(source.filesBefore, "source files before build");
  const filesAfter = fileRecordArray(source.filesAfter, "source files after build");
  if (
    manifest.digest !== sourceDigest ||
    digestFileRecords(filesBefore) !== sourceDigest ||
    digestFileRecords(filesAfter) !== sourceDigest ||
    !sameFileRecords(filesBefore, manifest.files) ||
    !sameFileRecords(filesAfter, manifest.files)
  ) {
    throw new Error("Native build source snapshots differ from the pinned source manifest");
  }
  for (const item of manifest.files) verifyFileRecord(item, "build source pin");
  if (
    !manifest.files.some((item) => item.path === cargoLock.path && item.sha256 === cargoLock.sha256)
  ) {
    throw new Error("Native build source manifest omits the bound Cargo.lock");
  }
  return { manifestPath, manifestSha256, sourceDigest, sourceFiles: manifest.files };
}

function validateObservationSnapshots(
  value: JsonValue | undefined,
  sourceFiles: FileRecord[],
  toolchain: { [key: string]: JsonValue },
  runtimePins: FileRecord[],
  codePins: FileRecord[],
  reviewedCodeManifest: FileRecord,
  cargoLock: FileRecord,
  binary: FileRecord,
): { rtk: FileRecord; just: FileRecord; python3: FileRecord } {
  const snapshots = objectValue(value, "build observation pins");
  const beforeRecord = fileRecord(snapshots.before, "pre-build observation snapshot");
  const afterRecord = fileRecord(snapshots.after, "post-build observation snapshot");
  verifyFileRecord(beforeRecord, "pre-build observation snapshot");
  verifyFileRecord(afterRecord, "post-build observation snapshot");
  const before = parseSnapshot(beforeRecord.path, "before");
  const after = parseSnapshot(afterRecord.path, "after");
  validateSnapshotBindings(
    before,
    "before",
    sourceFiles,
    toolchain,
    runtimePins,
    codePins,
    reviewedCodeManifest,
    cargoLock,
  );
  validateSnapshotBindings(
    after,
    "after",
    sourceFiles,
    toolchain,
    runtimePins,
    codePins,
    reviewedCodeManifest,
    cargoLock,
  );
  if (
    !sameJsonValue(before.pins, after.pins) ||
    !sameRecord(fileRecord(after.binary, "post-build snapshot binary"), binary)
  ) {
    throw new Error("Pre-build and post-build observation snapshots differ");
  }
  const beforeResolutions = objectValue(
    objectValue(before.pins, "pre-build snapshot pins").resolutions,
    "pre-build tool resolutions",
  );
  return {
    rtk: fileRecord(objectValue(beforeResolutions.rtk, "rtk resolution").actual, "rtk executable"),
    just: fileRecord(
      objectValue(beforeResolutions.just, "just resolution").actual,
      "just executable",
    ),
    python3: fileRecord(
      objectValue(beforeResolutions.python3, "python3 resolution").actual,
      "python executable",
    ),
  };
}

function parseSnapshot(filePath: string, expectedLabel: string): { [key: string]: JsonValue } {
  const snapshot = objectValue(
    parseJsonValue(fs.readFileSync(filePath, "utf8"), `${expectedLabel} observation snapshot`),
    `${expectedLabel} observation snapshot`,
  );
  if (stringValue(snapshot.label, `${expectedLabel} snapshot label`) !== expectedLabel) {
    throw new Error(`Build observation snapshot is not labeled ${expectedLabel}`);
  }
  return snapshot;
}

function validateSnapshotBindings(
  snapshot: { [key: string]: JsonValue },
  label: string,
  sourceFiles: FileRecord[],
  toolchain: { [key: string]: JsonValue },
  runtimePins: FileRecord[],
  codePins: FileRecord[],
  reviewedCodeManifest: FileRecord,
  cargoLock: FileRecord,
) {
  if (
    !sameFileRecords(
      fileRecordArray(snapshot.files, `${label} snapshot source files`),
      sourceFiles,
    ) ||
    !sameJsonValue(snapshot.toolchain, toolchain) ||
    !sameFileRecords(
      fileRecordArray(snapshot.runtimeAssetPins, `${label} runtime pins`),
      runtimePins,
    ) ||
    !sameRecord(fileRecord(snapshot.cargoLock, `${label} snapshot Cargo.lock`), cargoLock)
  ) {
    throw new Error(`Build ${label} observation snapshot differs from the receipt bindings`);
  }
  const pins = objectValue(snapshot.pins, `${label} snapshot pins`);
  if (
    !sameFileRecords(
      fileRecordArray(pins.runtime, `${label} snapshot pinned runtime`),
      runtimePins,
    ) ||
    !sameJsonValue(pins.toolchain, toolchain) ||
    !sameFileRecords(fileRecordArray(pins.code, `${label} snapshot code pins`), codePins) ||
    !sameRecord(
      fileRecord(pins.reviewedCodeManifest, `${label} snapshot code manifest`),
      reviewedCodeManifest,
    ) ||
    !sameRecord(fileRecord(pins.cargoLock, `${label} snapshot Cargo.lock`), cargoLock)
  ) {
    throw new Error(`Build ${label} snapshot pins do not match the receipt`);
  }
  verifyFileRecord(fileRecord(pins.state, `${label} snapshot state`), `${label} snapshot state`);
  verifyResolutionPins(pins.resolutions, label);
  verifyConfigPins(pins.config, label);
}

function verifyResolutionPins(value: JsonValue | undefined, label: string) {
  const resolutions = objectValue(value, `${label} tool resolutions`);
  if (
    !sameStringArray(Object.keys(resolutions).sort(), [
      "bash",
      "cargo",
      "git",
      "just",
      "python3",
      "rtk",
      "rustc",
      "rustup",
    ])
  ) {
    throw new Error(`Build ${label} snapshot has an incomplete tool resolution set`);
  }
  for (const [name, value] of Object.entries(resolutions)) {
    const resolution = objectValue(value, `${label} ${name} resolution`);
    const invocationPath = stringValue(
      resolution.invocationPath,
      `${label} ${name} invocation path`,
    );
    if (!path.isAbsolute(invocationPath))
      throw new Error(`Build ${label} has a nonabsolute ${name} invocation path`);
    verifyFileRecord(
      fileRecord(resolution.actual, `${label} ${name} resolved tool`),
      `${label} ${name} resolved tool`,
    );
  }
}

function verifyConfigPins(value: JsonValue | undefined, label: string) {
  const config = objectValue(value, `${label} configuration pins`);
  const absent = stringArray(config.unsupportedCargoConfigsAbsent, `${label} absent Cargo configs`);
  for (const filePath of absent) {
    if (!path.isAbsolute(filePath) || fs.existsSync(filePath)) {
      throw new Error(`Build ${label} Cargo config absence changed`);
    }
  }
  for (const item of fileRecordArray(
    config.supplementalFileHashes,
    `${label} supplemental config pins`,
  )) {
    verifyFileRecord(item, `${label} supplemental config pin`);
  }
}

export function validateCapacityReport(value: JsonValue | undefined, label: string, root: string) {
  const report = objectValue(value, label);
  if (
    stringValue(report.metric, `${label} metric`) !== "unique-inode-allocated-bytes" ||
    integerValue(report.metric_version, `${label} metric version`) !== 1 ||
    integerValue(report.baseline_bytes, `${label} baseline`) !== 2363047936 ||
    numberValue(report.factor, `${label} factor`) !== 2 ||
    integerValue(report.threshold_bytes, `${label} threshold`) !== 4726095872 ||
    integerValue(report.current_bytes, `${label} current bytes`) > 4726095872 ||
    report.over_capacity !== false ||
    report.metric_available !== true ||
    stringValue(report.target, `${label} target`) !== path.join(root, "target")
  ) {
    throw new Error(`${label} does not prove an under-threshold repository target`);
  }
}

function validateBinaryProvenance(
  value: JsonValue | undefined,
  binary: FileRecord,
  workingDirectory: string,
  buildStdout: FileRecord,
  buildStderr: FileRecord,
) {
  const record = objectValue(value, "binary provenance");
  const before = nullableIdentityRecord(record.binaryBefore, "pre-build binary identity");
  const after = identityRecord(record.binaryAfter, "post-build binary identity");
  if (
    after.path !== binary.path ||
    after.sha256 !== binary.sha256 ||
    after.size !== binary.size ||
    (before !== null && before.path !== binary.path)
  ) {
    throw new Error("Native binary provenance does not prove a fresh compiler output");
  }
  verifyCurrentBinaryIdentity(after);
  validateCapacityReport(record.guard, "binary guard report", workingDirectory);
  const guard = objectValue(record.guard, "binary guard report");
  const rawGuard = objectValue(record.rawGuard, "raw binary guard report");
  const loggedGuard = readBuildGuard(buildStdout, buildStderr);
  if (!sameJsonValue(rawGuard, loggedGuard)) {
    throw new Error("Raw binary guard report differs from the owned build logs");
  }
  const normalizedGuard: { [key: string]: JsonValue } = { ...rawGuard };
  if (!Object.hasOwn(rawGuard, "metric_available")) normalizedGuard.metric_available = true;
  if (!sameJsonValue(guard, normalizedGuard)) {
    throw new Error("Normalized binary guard report differs from its raw trace");
  }
  if (
    stringValue(guard.cleanup_status, "binary guard cleanup status") !== "not-needed" ||
    !Array.isArray(guard.removed) ||
    guard.removed.length !== 0
  ) {
    throw new Error("Native build capacity guard did not preserve target artifacts");
  }
}

function readBuildGuard(stdout: FileRecord, stderr: FileRecord): { [key: string]: JsonValue } {
  const ansiCsi = new RegExp(`${String.fromCharCode(27)}\\[[0-9;]*[A-Za-z]`, "g");
  const streams = [stdout, stderr].map((record) =>
    fs.readFileSync(record.path, "utf8").replace(ansiCsi, ""),
  );
  const reports: { [key: string]: JsonValue }[] = [];
  for (const line of streams.join("\n").split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed.startsWith("{")) continue;
    try {
      const parsed = objectValue(
        parseJsonValue(trimmed, "build capacity guard trace"),
        "build capacity guard trace",
      );
      if (Object.hasOwn(parsed, "cleanup_status")) reports.push(parsed);
    } catch {}
  }
  if (reports.length !== 1) {
    throw new Error("Owned build logs do not contain exactly one capacity guard report");
  }
  const report = reports[0];
  if (report === undefined) throw new Error("Owned build capacity guard report is missing");
  return report;
}

interface BinaryIdentity extends FileRecord {
  device: number;
  inode: number;
  mtimeNs: string;
  ctimeNs: string;
}

function identityRecord(value: JsonValue | undefined, label: string): BinaryIdentity {
  const record = objectValue(value, label);
  return {
    ...fileRecord(record, label),
    device: integerValue(record.device, `${label} device`),
    inode: integerValue(record.inode, `${label} inode`),
    mtimeNs: decimalString(record.mtimeNs, `${label} mtime`),
    ctimeNs: decimalString(record.ctimeNs, `${label} ctime`),
  };
}

function nullableIdentityRecord(
  value: JsonValue | undefined,
  label: string,
): BinaryIdentity | null {
  if (value === null) return null;
  return identityRecord(value, label);
}

export function verifyCurrentBinaryIdentity(identity: BinaryIdentity) {
  const stats = fs.statSync(identity.path, { bigint: true });
  if (
    stats.dev !== BigInt(identity.device) ||
    stats.ino !== BigInt(identity.inode) ||
    stats.mtimeNs !== BigInt(identity.mtimeNs) ||
    stats.ctimeNs !== BigInt(identity.ctimeNs)
  ) {
    throw new Error("Native binary identity changed after its build receipt was written");
  }
}

export function parseSourceManifest(manifestPath: string): BuildSourceManifest {
  const root = objectValue(
    parseJsonValue(fs.readFileSync(manifestPath, "utf8"), "build source manifest"),
    "build source manifest",
  );
  if (
    stringValue(root.schema, "source manifest schema") !== "krr-native-build-source-manifest-v1"
  ) {
    throw new Error("Native build source manifest has an unsupported schema");
  }
  const files = fileRecordArray(root.files, "source manifest files");
  const sorted = [...files].sort((left, right) =>
    Buffer.compare(Buffer.from(left.path, "utf8"), Buffer.from(right.path, "utf8")),
  );
  if (
    !sameFileRecords(files, sorted) ||
    new Set(files.map((item) => item.path)).size !== files.length
  ) {
    throw new Error("Native build source manifest paths are not unique and sorted");
  }
  const digest = shaField(root.digest, "source manifest digest");
  if (digestFileRecords(files) !== digest) {
    throw new Error("Native build source manifest digest is invalid");
  }
  return { schema: "krr-native-build-source-manifest-v1", digest, files };
}

export function digestFileRecords(records: FileRecord[]): string {
  const material = records.map((item) => `${item.path}\0${item.sha256}\0${item.size}\n`).join("");
  return sha256(Buffer.from(material, "utf8"));
}

export function digestStringRecord(record: { [key: string]: string }): string {
  const canonical = JSON.stringify(
    Object.fromEntries(
      Object.entries(record).sort(([left], [right]) => (left < right ? -1 : left > right ? 1 : 0)),
    ),
  );
  return sha256(Buffer.from(canonical, "utf8"));
}

export function verifyFileRecord(item: FileRecord, label: string) {
  const actualPath = canonicalRegularFile(item.path, label);
  if (actualPath !== item.path || fileSha256(actualPath) !== item.sha256) {
    throw new Error(`${label} changed after its receipt was written`);
  }
  if (fs.statSync(actualPath).size !== item.size) {
    throw new Error(`${label} size changed after its receipt was written`);
  }
}

export function assertCurrentGitSourceSet(
  workingDirectory: string,
  files: FileRecord[],
  executable = "git",
  environment?: { [key: string]: string },
) {
  const options =
    environment === undefined
      ? { cwd: workingDirectory }
      : { cwd: workingDirectory, env: environment, timeout: 30000 };
  const tracked = execFileSync(executable, ["ls-files", "-z"], options);
  const untracked = execFileSync(
    executable,
    ["ls-files", "--others", "--exclude-standard", "-z"],
    options,
  );
  const currentPaths = new Set(
    Buffer.concat([tracked, untracked])
      .toString("utf8")
      .split("\0")
      .filter((item) => item.length > 0)
      .map((item) => path.resolve(workingDirectory, item)),
  );
  const pinnedPaths = new Set(files.map((item) => item.path));
  if (
    currentPaths.size !== pinnedPaths.size ||
    [...currentPaths].some((filePath) => !pinnedPaths.has(filePath))
  ) {
    throw new Error("Native build source manifest differs from current tracked source files");
  }
}

export function canonicalRegularFile(filePath: string, label: string): string {
  if (!path.isAbsolute(filePath) || path.resolve(filePath) !== filePath) {
    throw new Error(`${label} path is not absolute and canonical`);
  }
  const stats = fs.lstatSync(filePath);
  if (stats.isSymbolicLink() || !stats.isFile() || fs.realpathSync(filePath) !== filePath) {
    throw new Error(`${label} is not a canonical regular file`);
  }
  return filePath;
}

function fileRecord(value: JsonValue | undefined, label: string): FileRecord {
  const record = objectValue(value, label);
  return {
    path: stringValue(record.path, `${label} path`),
    sha256: shaField(record.sha256, `${label} SHA`),
    size: integerValue(record.size, `${label} size`),
  };
}

function diskFileRecord(filePath: string): FileRecord {
  const stats = fs.statSync(filePath);
  return { path: filePath, sha256: fileSha256(filePath), size: stats.size };
}

function jsonFileRecord(record: FileRecord): { [key: string]: JsonValue } {
  return { path: record.path, sha256: record.sha256, size: record.size };
}

function fileRecordArray(value: JsonValue | undefined, label: string): FileRecord[] {
  if (!Array.isArray(value)) throw new Error(`Invalid ${label}`);
  return value.map((item) => fileRecord(item, label));
}

export function sameFileRecords(left: FileRecord[], right: FileRecord[]): boolean {
  return (
    left.length === right.length &&
    left.every((item, index) => {
      const other = right[index];
      return (
        other !== undefined &&
        item.path === other.path &&
        item.sha256 === other.sha256 &&
        item.size === other.size
      );
    })
  );
}

function objectValue(value: JsonValue | undefined, label: string): { [key: string]: JsonValue } {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`Invalid ${label}`);
  }
  return value;
}

function stringValue(value: JsonValue | undefined, label: string): string {
  if (typeof value !== "string") throw new Error(`Invalid ${label}`);
  return value;
}

function nonemptyString(value: JsonValue | undefined, label: string): string {
  const result = stringValue(value, label);
  if (result.length === 0) throw new Error(`Invalid ${label}`);
  return result;
}

function shaField(value: JsonValue | undefined, label: string): string {
  const result = stringValue(value, label);
  if (!/^[a-f0-9]{64}$/.test(result)) throw new Error(`Invalid ${label}`);
  return result;
}

function integerValue(value: JsonValue | undefined, label: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw new Error(`Invalid ${label}`);
  }
  return value;
}

function numberValue(value: JsonValue | undefined, label: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) throw new Error(`Invalid ${label}`);
  return value;
}

function decimalString(value: JsonValue | undefined, label: string): string {
  const result = stringValue(value, label);
  if (!/^(0|[1-9][0-9]*)$/.test(result)) throw new Error(`Invalid ${label}`);
  return result;
}

function decimalBigInt(value: JsonValue | undefined, label: string): bigint {
  return BigInt(decimalString(value, label));
}

function stringArray(value: JsonValue | undefined, label: string): string[] {
  if (!Array.isArray(value) || value.some((item) => typeof item !== "string")) {
    throw new Error(`Invalid ${label}`);
  }
  return value.filter((item): item is string => typeof item === "string");
}

function stringRecord(value: JsonValue | undefined, label: string): { [key: string]: string } {
  const record = objectValue(value, label);
  for (const item of Object.values(record)) {
    if (typeof item !== "string") throw new Error(`Invalid ${label}`);
  }
  return record as { [key: string]: string };
}

function sameStringRecord(
  left: { [key: string]: string },
  right: { [key: string]: string },
): boolean {
  const leftEntries = Object.entries(left).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
  const rightEntries = Object.entries(right).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
  return sameJsonValue(leftEntries, rightEntries);
}

function removeObserverEnvironment(environment: { [key: string]: string }) {
  const producerKeys = new Set(["CARGO", "KRR_BUILD_OBSERVER_REQUEST", "PYTHONDONTWRITEBYTECODE"]);
  return Object.fromEntries(Object.entries(environment).filter(([key]) => !producerKeys.has(key)));
}
