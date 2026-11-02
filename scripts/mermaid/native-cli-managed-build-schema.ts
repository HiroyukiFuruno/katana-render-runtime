import { z } from "zod";
import {
  digest,
  environment,
  files,
  managedCapacitySchema,
  managedFileSchema,
  managedIdentitySchema,
  managedSnapshotSchema,
  ns,
  packageSchema,
  positive,
  rustToolLauncherSchema,
  strings,
  targetSchema,
} from "./native-cli-managed-input-schema";
import { nativeRawFdSchema } from "./native-cli-managed-raw-fd";

export {
  managedCapacitySchema,
  managedFileSchema,
  managedIdentitySchema,
  managedSnapshotSchema,
  managedSourceSchema,
} from "./native-cli-managed-input-schema";
export const managedBuildSchemaName = "krr-native-cli-managed-build-v3";
export const frozenManagedBuildSchemaName = "krr-native-cli-managed-build-v4";
export const managedOwnedSchema = z
  .object({
    argv: strings,
    stdout: managedFileSchema,
    stderr: managedFileSchema,
    label: z.string(),
    cwd: z.string(),
    attemptToken: digest,
    environment,
    environmentSha256: digest,
    pid: positive,
    actualExit: z.literal(0),
    startedNs: ns,
    endedNs: ns,
    reaped: z.literal(true),
    errors: z.tuple([]),
  })
  .strict();
export const managedLifecycleSchema = z
  .object({
    schema: z.literal("krr-performance-command-lifecycle-v1"),
    argv: strings,
    role: z.enum(["tool-query", "metadata", "preflight", "build"]),
    cwd: z.string(),
    identity: z
      .object({
        pid: positive,
        parentPid: positive,
        sessionId: positive,
        processGroupId: positive,
        spawnObservedMonotonic: z.number().nonnegative(),
        observationLimit: z.literal("trusted dedicated session/group; non-atomic PID observations"),
      })
      .strict(),
    runLimitSeconds: positive,
    cleanupLimitSeconds: z.literal(10),
    elapsedSeconds: z.number().nonnegative(),
    groupClosed: z.literal(true),
    rawFdsClosed: z.literal(true),
    exitCode: z.literal(0),
    failure: z.null(),
    cleanupFailure: z.null(),
    exitedLeaderSignalObservations: strings,
    raw: z.tuple([
      z.object({ path: z.string(), sha256: digest }).strict(),
      z.object({ path: z.string(), sha256: digest }).strict(),
    ]),
    nativeRawFdClosure: nativeRawFdSchema,
    descendantClosureProved: z.literal(false),
  })
  .strict();
export const managedRequestSchema = z
  .object({
    schema: z.literal("krr-native-cli-guard-request-v2"),
    attemptToken: digest,
    cwd: z.string(),
    target: z.string(),
    binaryPath: z.string(),
    issuedMonotonicNs: ns,
    expiresMonotonicNs: ns,
    cargo: managedFileSchema,
    rustc: managedFileSchema,
    cargoLauncher: rustToolLauncherSchema,
    rustcLauncher: rustToolLauncherSchema,
    sourceManifest: managedFileSchema,
    sourceDigest: digest,
    metadata: managedFileSchema,
    metadataObservation: managedFileSchema,
    expectedPackage: packageSchema,
    expectedTarget: targetSchema,
    environment,
    environmentSha256: digest,
    guardCodePins: files,
    capacityAdmission: managedCapacitySchema,
  })
  .strict();
const guardPins = z
  .object({
    source: files,
    cargo: managedFileSchema,
    rustc: managedFileSchema,
    cargoLauncher: rustToolLauncherSchema,
    rustcLauncher: rustToolLauncherSchema,
    guardCodePins: files,
    capacityAdmission: managedCapacitySchema,
    rustupSelectedRustc: managedFileSchema,
    unsupportedCargoConfigsAbsent: strings,
  })
  .strict();
const cargoOwned = managedOwnedSchema
  .omit({ label: true })
  .extend({
    parentPid: positive,
    sessionId: positive,
    processGroupId: positive,
    spawnObservedMonotonicNs: ns,
    signal: z.null(),
    rawFdsClosed: z.literal(true),
  })
  .strict();
// Cargo自身のJSONは外部API。保存rawとの全文比較後に必要なAPI値を検査する。
const cargoJson = z.record(z.string(), z.unknown());
const capacityEvidence = z
  .object({
    argv: strings,
    cwd: z.string(),
    actualExit: z.literal(0),
    reaped: z.literal(true),
    stdout: managedFileSchema,
    stderr: managedFileSchema,
    lifecycle: managedFileSchema,
    startedNs: ns,
    endedNs: ns,
    report: z.record(z.string(), z.unknown()),
  })
  .strict();
export const managedCapacityObservationSchema = capacityEvidence
  .extend({
    schema: z.enum([
      "krr-native-cli-capacity-preflight-cleanup-v1",
      "krr-native-cli-capacity-preflight-refresh-v1",
    ]),
  })
  .strict();
export const managedCapacityPreflightSchema = capacityEvidence
  .extend({
    schema: z.literal("krr-native-cli-capacity-preflight-v1"),
    cleanupObservation: managedFileSchema.nullable(),
    refreshObservation: managedFileSchema.nullable(),
  })
  .strict();
export const managedObservationSchema = z
  .object({
    schema: z.literal("krr-native-cli-guard-observation-v2"),
    status: z.literal("cargo-completed"),
    diagnosticOnly: z.literal(true),
    accepted: z.literal(false),
    attemptToken: digest,
    request: managedFileSchema,
    sourceManifest: managedFileSchema,
    sourceDigest: digest,
    metadata: managedFileSchema,
    metadataObservation: managedFileSchema,
    expectedPackage: packageSchema,
    expectedTarget: targetSchema,
    cargo: managedFileSchema,
    rustc: managedFileSchema,
    cargoLauncher: rustToolLauncherSchema,
    rustcLauncher: rustToolLauncherSchema,
    rustupResolutionQueries: z
      .array(z.object({ observation: managedOwnedSchema, lifecycle: managedFileSchema }).strict())
      .min(1),
    guardCodePins: files,
    pinsBefore: guardPins,
    pinsAfter: guardPins,
    capacityBefore: managedFileSchema,
    capacityAfter: managedFileSchema,
    capacityProtection: managedCapacitySchema,
    ownedCargo: cargoOwned,
    binaryBefore: managedIdentitySchema.nullable(),
    binaryAfter: managedIdentitySchema,
    artifact: cargoJson,
    buildFinished: cargoJson,
    errors: z.tuple([]),
    externalGroupClosureRequired: z.literal(true),
    scope: z.string(),
  })
  .strict();
const managedReceiptV3Schema = z
  .object({
    schema: z.literal(managedBuildSchemaName),
    status: z.literal("completed"),
    accepted: z.literal(true),
    attemptToken: digest,
    cwd: z.string(),
    expectedSource: managedFileSchema,
    expectedProtectedRegistry: managedFileSchema,
    request: managedFileSchema,
    capacityPreflight: managedFileSchema,
    observation: managedFileSchema,
    metadataLifecycle: managedFileSchema,
    hostPrebuild: z
      .object({
        observation: managedOwnedSchema,
        lifecycle: managedFileSchema,
        receipt: managedFileSchema,
      })
      .strict(),
    buildLifecycle: managedFileSchema,
    buildObservation: managedOwnedSchema,
    snapshotBefore: managedSnapshotSchema,
    snapshotAfter: managedSnapshotSchema,
    binary: managedIdentitySchema,
    runtimeAssetPins: files.min(1),
    toolProbes: z
      .array(z.object({ observation: managedOwnedSchema, lifecycle: managedFileSchema }).strict())
      .length(2),
    scope: z
      .object({
        sourceEndpointsOnly: z.literal(true),
        effectiveCompilerReadsProved: z.literal(false),
        registryAtomicityProved: z.literal(false),
        installedDependencyClosureProved: z.literal(false),
        descendantClosureProved: z.literal(false),
      })
      .strict(),
  })
  .strict();

const managedReceiptV4Schema = managedReceiptV3Schema
  .extend({
    schema: z.literal(frozenManagedBuildSchemaName),
    frozenBinary: managedIdentitySchema,
    frozenFrom: managedIdentitySchema,
  })
  .strict();
export const managedReceiptSchema = z.discriminatedUnion("schema", [
  managedReceiptV3Schema,
  managedReceiptV4Schema,
]);

export type ManagedReceipt = z.infer<typeof managedReceiptSchema>;
export type ManagedOwned = z.infer<typeof managedOwnedSchema>;

export type ManagedRequest = z.infer<typeof managedRequestSchema>;
export type ManagedObservation = z.infer<typeof managedObservationSchema>;
