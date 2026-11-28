import { createHash, randomUUID } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";
import { z } from "zod";
import { justRecipes } from "../html/harness-contract-graph";
import { canonicalRegularFile } from "../mermaid/native-build-receipt";
import {
  currentRustFactsHostInputs,
  parseRustFactsHostMetadata,
  resolveRustFactsHostJust,
} from "./runtime-rust-facts-host-inputs";

import {
  assertHostPhysicalRecord,
  assertRustFactsHostBinaryRecord,
  rustFactsHostBinaryPath as binaryPath,
  createRustFactsHostAttemptDirectory,
  hostAttemptDirectory,
  hostBinaryRecordSchema,
  hostBuildOutputRecord,
  hostBuildOutputRecordSchema,
  hostEnvironmentDiagnostics,
  hostFileRecord,
  hostInputsSchema,
  hostProcessSchema,
  observeRustFactsHostBinary,
  observeRustFactsHostRecipe,
  publishRustFactsHostBinary,
  type RustFactsHostMode,
  requireRustFactsHostSuccess,
  rustFactsHostArtifactPath,
} from "./runtime-rust-facts-host-protocol";

export type { RustFactsHostMode } from "./runtime-rust-facts-host-protocol";

export const rustFactsHostReceiptSchema = z.strictObject({
  schema: z.literal("krr-rust-facts-host/v2"),
  status: z.literal("prepared"),
  root: z.string(),
  attempt: z.string().uuid(),
  inputs: hostInputsSchema,
  buildOutput: hostBuildOutputRecordSchema,
  binary: hostBinaryRecordSchema,
  metadataBefore: hostProcessSchema,
  build: hostProcessSchema,
  metadataAfter: hostProcessSchema,
});
export type RustFactsHostReceipt = z.infer<typeof rustFactsHostReceiptSchema>;
export type RustFactsHostRun = ReturnType<typeof observeRustFactsHostBinary>;
export type RustFactsHostInputDifference = {
  field: string;
  expectedSha256: string | null;
  actualSha256: string | null;
};

export type RustFactsHostPairRun = {
  nativeWidth: RustFactsHostRun;
  classic: RustFactsHostRun;
};

export function defaultRustFactsHostReceipt(root: string): string {
  return path.join(root, "tmp/quality/rust-facts-host/current/receipt.json");
}

function diagnosticDigest(value: unknown): string {
  const serialized = JSON.stringify(value);
  if (serialized === undefined) throw new Error("Rust facts host diagnostic value is not JSON");
  return createHash("sha256").update(serialized).digest("hex");
}

function appendInputDifference(
  differences: RustFactsHostInputDifference[],
  field: string,
  expected: unknown,
  actual: unknown,
  directDigest = false,
): void {
  if (isDeepStrictEqual(expected, actual)) return;
  differences.push({
    field,
    expectedSha256:
      expected === null ? null : directDigest ? String(expected) : diagnosticDigest(expected),
    actualSha256: actual === null ? null : directDigest ? String(actual) : diagnosticDigest(actual),
  });
}

function appendToolInputDifferences(
  differences: RustFactsHostInputDifference[],
  expected: RustFactsHostReceipt["inputs"]["tools"],
  actual: RustFactsHostReceipt["inputs"]["tools"],
): void {
  for (const name of ["just", "cargo", "rustc", "python3", "bun", "rtk"] as const) {
    const expectedTool = expected[name];
    const actualTool = actual[name];
    if (expectedTool === null || actualTool === null) {
      appendInputDifference(differences, `tools.${name}`, expectedTool, actualTool);
      continue;
    }
    appendInputDifference(
      differences,
      `tools.${name}.launcher`,
      expectedTool.launcher,
      actualTool.launcher,
    );
    appendInputDifference(
      differences,
      `tools.${name}.executable`,
      expectedTool.executable,
      actualTool.executable,
    );
    if (name === "just")
      appendInputDifference(
        differences,
        "tools.just.invocationPath",
        expected.just.invocationPath ?? null,
        actual.just.invocationPath ?? null,
      );
  }
}

function appendFileInputDifferences(
  differences: RustFactsHostInputDifference[],
  expected: RustFactsHostReceipt["inputs"]["files"],
  actual: RustFactsHostReceipt["inputs"]["files"],
): void {
  const fileCount = Math.max(expected.length, actual.length);
  for (let index = 0; index < fileCount; index += 1) {
    appendInputDifference(
      differences,
      `files[${String(index).padStart(6, "0")}]`,
      expected[index] ?? null,
      actual[index] ?? null,
    );
  }
}

/** WHY: 差分の原因を特定できるようにしつつ、パスや環境値は公開しない。 */
export function rustFactsHostInputDifferences(
  expected: RustFactsHostReceipt["inputs"],
  actual: RustFactsHostReceipt["inputs"],
): RustFactsHostInputDifference[] {
  const differences: RustFactsHostInputDifference[] = [];
  appendInputDifference(
    differences,
    "environmentSha256",
    expected.environmentSha256,
    actual.environmentSha256,
    true,
  );
  appendToolInputDifferences(differences, expected.tools, actual.tools);
  appendFileInputDifferences(differences, expected.files, actual.files);
  return differences;
}

export function rustFactsHostFingerprint(
  root: string,
  receipt = defaultRustFactsHostReceipt(root),
): string {
  const prepared = verifyRustFactsHost(root, receipt);
  const semantic = {
    inputs: prepared.inputs,
    binary: {
      sha256: prepared.binary.sha256,
      size: prepared.binary.size,
      mode: prepared.binary.mode,
    },
  };
  return createHash("sha256").update(JSON.stringify(semantic)).digest("hex");
}

function assertRecipes(root: string): void {
  const recipes = justRecipes(fs.readFileSync(path.join(root, "Justfile"), "utf8"));
  const expected = {
    "rust-facts-host-build":
      '@CARGO_TARGET_DIR={{quote(root + "/target")}} {{CARGO_BUILD}} build --locked -p kdr-linter --bin mathjax_consumer_facts',
    "rust-facts-host-metadata":
      '@CARGO_TARGET_DIR={{quote(root + "/target")}} {{CARGO}} metadata --locked --format-version 1',
  };
  for (const [name, command] of Object.entries(expected))
    if (recipes.get(name)?.trim() !== command)
      throw new Error(`Rust facts host recipe changed: ${name}`);
}

export function prepareRustFactsHost(
  root: string,
  receipt = defaultRustFactsHostReceipt(root),
): RustFactsHostReceipt {
  assertRecipes(root);
  const attempt = randomUUID();
  const directory = createRustFactsHostAttemptDirectory(root, receipt, attempt);
  const just = resolveRustFactsHostJust(root);
  if (!just.invocationPath) throw new Error("Rust facts host Just invocation is missing");
  const metadataBefore = observeRustFactsHostRecipe(
    root,
    just.invocationPath,
    "rust-facts-host-metadata",
    directory,
    "metadata-before",
  );
  requireRustFactsHostSuccess(metadataBefore.proof);
  const metadata = parseRustFactsHostMetadata(metadataBefore.stdout);
  const before = currentRustFactsHostInputs(root, metadata);
  if (!isDeepStrictEqual(just, before.tools.just))
    throw new Error("Rust facts host Just changed during metadata");
  const build = observeRustFactsHostRecipe(
    root,
    before.tools.just.invocationPath ?? before.tools.just.executable.path,
    "rust-facts-host-build",
    directory,
    "build",
  );
  requireRustFactsHostSuccess(build.proof);
  const buildOutput = hostBuildOutputRecord(binaryPath(root));
  const binary = publishRustFactsHostBinary(root, receipt, attempt, buildOutput);
  const metadataAfter = observeRustFactsHostRecipe(
    root,
    before.tools.just.invocationPath ?? before.tools.just.executable.path,
    "rust-facts-host-metadata",
    directory,
    "metadata-after",
  );
  requireRustFactsHostSuccess(metadataAfter.proof);
  const after = currentRustFactsHostInputs(
    root,
    parseRustFactsHostMetadata(metadataAfter.stdout),
    before.tools,
  );
  if (
    !isDeepStrictEqual(metadata, parseRustFactsHostMetadata(metadataAfter.stdout)) ||
    !isDeepStrictEqual(before, after)
  )
    throw new Error("Rust facts host inputs changed during preparation");
  const value = rustFactsHostReceiptSchema.parse({
    schema: "krr-rust-facts-host/v2",
    status: "prepared",
    root,
    attempt,
    inputs: after,
    buildOutput,
    binary,
    metadataBefore: metadataBefore.proof,
    build: build.proof,
    metadataAfter: metadataAfter.proof,
  });
  const temporary = path.join(path.dirname(receipt), `.receipt-${attempt}.pending.json`);
  fs.writeFileSync(temporary, JSON.stringify(value), { flag: "wx" });
  verifyRustFactsHost(root, temporary);
  if (fs.existsSync(receipt)) hostFileRecord(receipt);
  fs.renameSync(temporary, receipt);
  return value;
}

export function verifyRustFactsHost(
  root: string,
  receipt = defaultRustFactsHostReceipt(root),
  environment: NodeJS.ProcessEnv = process.env,
): RustFactsHostReceipt {
  const receiptRecord = hostFileRecord(receipt);
  const value = rustFactsHostReceiptSchema.parse(JSON.parse(fs.readFileSync(receipt, "utf8")));
  if (
    value.root !== root ||
    value.buildOutput.path !== binaryPath(root) ||
    value.binary.path !==
      rustFactsHostArtifactPath(root, receipt, value.attempt, value.binary.sha256) ||
    value.buildOutput.sha256 !== value.binary.sha256 ||
    value.buildOutput.size !== value.binary.size ||
    value.buildOutput.mode !== value.binary.mode
  )
    throw new Error("Rust facts host root/binary differs");
  assertRecipes(root);
  for (const [label, proof] of [
    ["rust-facts-host-metadata", value.metadataBefore],
    ["rust-facts-host-build", value.build],
    ["rust-facts-host-metadata", value.metadataAfter],
  ] as const) {
    requireRustFactsHostSuccess(proof);
    if (
      proof.cwd !== root ||
      !isDeepStrictEqual(proof.argv, [
        value.inputs.tools.just.invocationPath ?? value.inputs.tools.just.executable.path,
        label,
        root,
      ])
    )
      throw new Error("Rust facts host prepared command differs");
    assertHostPhysicalRecord(proof.stdout);
    assertHostPhysicalRecord(proof.stderr);
  }
  const metadata = parseRustFactsHostMetadata(fs.readFileSync(value.metadataBefore.stdout.path));
  const after = parseRustFactsHostMetadata(fs.readFileSync(value.metadataAfter.stdout.path));
  const currentInputs = currentRustFactsHostInputs(root, metadata, value.inputs.tools, environment);
  if (!isDeepStrictEqual(metadata, after)) throw new Error("Rust facts host metadata is stale");
  if (!isDeepStrictEqual(currentInputs, value.inputs)) {
    throw new Error(
      `Rust facts host prepared inputs are stale: ${JSON.stringify(
        rustFactsHostInputDifferences(value.inputs, currentInputs),
      )}`,
    );
  }
  assertRustFactsHostBinaryRecord(value.binary);
  assertHostPhysicalRecord(receiptRecord);
  return value;
}

export function runRustFactsHost(
  root: string,
  mode: RustFactsHostMode,
  sourceRoot: string,
  receipt = defaultRustFactsHostReceipt(root),
) {
  const prepared = verifyRustFactsHost(root, receipt);
  assertModeInput(mode, sourceRoot);
  const directory = hostAttemptDirectory(root, receipt);
  const result = observeRustFactsHostBinary(
    root,
    prepared.binary,
    receipt,
    prepared.attempt,
    mode,
    sourceRoot,
    directory,
  );
  const current = verifyRustFactsHost(root, receipt);
  if (!isDeepStrictEqual(prepared, current))
    throw new Error("Rust facts host receipt changed during inspection");
  return result;
}

function assertModeInput(mode: RustFactsHostMode, input: string): void {
  if (!path.isAbsolute(input) || fs.realpathSync(input) !== input)
    throw new Error("Rust facts input is not canonical");
  if (mode === "macho-noso") canonicalRegularFile(input, "Rust facts manifest");
  if (mode !== "macho-noso" && !fs.lstatSync(input).isDirectory())
    throw new Error("Rust facts source root is not a directory");
}

export function runRustFactsHostPair(
  root: string,
  sourceRoot: string,
  receipt = defaultRustFactsHostReceipt(root),
): RustFactsHostPairRun {
  const prepared = verifyRustFactsHost(root, receipt);
  if (
    !path.isAbsolute(sourceRoot) ||
    fs.realpathSync(sourceRoot) !== sourceRoot ||
    !fs.lstatSync(sourceRoot).isDirectory()
  )
    throw new Error("Rust facts source root is not canonical");
  const directory = hostAttemptDirectory(root, receipt);
  const nativeWidthDirectory = path.join(directory, "native-width");
  const classicDirectory = path.join(directory, "classic");
  fs.mkdirSync(nativeWidthDirectory);
  fs.mkdirSync(classicDirectory);
  const nativeWidth = observeRustFactsHostBinary(
    root,
    prepared.binary,
    receipt,
    prepared.attempt,
    "native-width",
    sourceRoot,
    nativeWidthDirectory,
  );
  const classic = observeRustFactsHostBinary(
    root,
    prepared.binary,
    receipt,
    prepared.attempt,
    "classic",
    sourceRoot,
    classicDirectory,
  );
  const current = verifyRustFactsHost(root, receipt);
  if (!isDeepStrictEqual(prepared, current))
    throw new Error("Rust facts host receipt changed during paired inspection");
  return { nativeWidth, classic };
}

function parseChildJson(result: RustFactsHostRun): unknown {
  return JSON.parse(result.stdout.toString("utf8"));
}

function writePairChildOutput(result: RustFactsHostRun): void {
  process.stderr.write(result.stderr);
  if (result.proof.exitCode !== 0 || result.proof.signal !== null || result.proof.error !== null)
    process.stderr.write(result.stdout);
  if (result.proof.error) process.stderr.write(result.proof.error);
}

if (import.meta.main) {
  const [command, root, modeOrReceipt, sourceRoot, receipt] = process.argv.slice(2);
  if (!root)
    throw new Error("Usage: prepare/verify ROOT [RECEIPT] | run ROOT MODE SOURCE_ROOT [RECEIPT]");
  if (command === "prepare") {
    const environment = hostEnvironmentDiagnostics(root);
    const prepared = prepareRustFactsHost(root, modeOrReceipt);
    if (prepared.inputs.environmentSha256 !== environment.environmentSha256)
      throw new Error("Rust facts host environment changed during preparation");
    process.stdout.write(
      `${JSON.stringify({ schema: "krr-rust-facts-host-environment-diagnostic-v1", ...environment })}\n`,
    );
  } else if (command === "verify") verifyRustFactsHost(root, modeOrReceipt);
  else if (command === "fingerprint") {
    const fingerprint = rustFactsHostFingerprint(root, modeOrReceipt);
    process.stdout.write(
      `${JSON.stringify({ schema: "krr-rust-facts-host-fingerprint-v1", fingerprint })}\n`,
    );
  } else if (
    command === "run" &&
    sourceRoot &&
    ["mathjax", "native-width", "classic", "macho-noso"].includes(modeOrReceipt ?? "")
  ) {
    const result = runRustFactsHost(root, modeOrReceipt as RustFactsHostMode, sourceRoot, receipt);
    process.stdout.write(result.stdout);
    process.stderr.write(result.stderr);
    if (result.proof.error) process.stderr.write(result.proof.error);
    process.exitCode = result.proof.exitCode ?? 1;
  } else if (command === "run" && modeOrReceipt === "native-width-classic" && sourceRoot) {
    const result = runRustFactsHostPair(root, sourceRoot, receipt);
    writePairChildOutput(result.nativeWidth);
    writePairChildOutput(result.classic);
    const failed = [result.nativeWidth, result.classic].find(
      (child) =>
        child.proof.exitCode !== 0 || child.proof.signal !== null || child.proof.error !== null,
    );
    if (failed) {
      process.exitCode =
        failed.proof.exitCode !== null && failed.proof.exitCode !== 0 ? failed.proof.exitCode : 1;
    } else {
      process.stdout.write(
        `${JSON.stringify({
          nativeWidth: parseChildJson(result.nativeWidth),
          classic: parseChildJson(result.classic),
        })}\n`,
      );
    }
  } else throw new Error("invalid Rust facts host command");
}
