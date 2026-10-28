import { randomUUID } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { type Fixture, FixtureRepository } from "./diagram_update_fixtures";
import { fileSha256, type JsonValue, parseJsonValue, sha256 } from "./mermaid-pair-manifest";
import {
  canonicalRegularFile,
  type FileRecord,
  type NativeBuildReceiptSummary,
  sameFileRecords,
  verifyNativeBuildReceipt,
} from "./native-build-receipt";
import {
  CLASSIC_CLI_ATTESTATION_SCOPE,
  LEGACY_CLI_ATTESTATION_SCOPE,
  verifyNativeClassicCapture,
} from "./native-classic-capture";
import { spawnNativeCli } from "./native-cli-process";
import { type NativeRenderBatchResult, renderNativeFixtureBatch } from "./native-render-batch";

interface Options {
  fixturesDir: string;
  fixtureGlob: string;
  outputDir: string;
  binary: string;
  runtime: string;
  buildReceipt: string;
  renderConcurrency: 1 | 2;
}

export interface CliAttestationExpected {
  cliAttemptToken: string;
  pid: number | null;
  inputPath: string;
  inputSha256: string;
  binarySha256: string;
  runtimeSha256: string;
  outputSvgSha256: string | null;
  cliAttestationSha256: string | null;
  actualRenderSourceSha256: string | null;
  tempRoot?: string;
}

export interface NativeRenderRow {
  slug: string;
  markdownPath: string;
  inputMarkdownSha256: string;
  inputPath: string;
  inputSha256: string;
  inputSha256After: string;
  originalFenceSourceSha256: string;
  canonicalSourceSha256: string;
  sourceAdapterPolicy: string;
  sourceParserSha256Before: string;
  sourceParserSha256After: string;
  binaryPath: string;
  binarySha256Before: string;
  binarySha256: string;
  binarySha256After: string;
  runtimePath: string;
  runtimeSha256: string;
  buildReceiptSha256: string;
  cliAttemptToken: string;
  argv: string[];
  pid: number | null;
  exitStatus: number | null;
  signal: string | null;
  spawnError: string | null;
  stdoutPath: string;
  stdoutSha256: string;
  stderrPath: string;
  stderrSha256: string;
  outputPath: string;
  outputSvgSha256: string | null;
  inputMarkdownSha256After: string;
  cliAttestationPath: string;
  cliAttestationSha256: string | null;
  tempRoot: string;
  attestedTempDir: string | null;
  actualRenderSourceSha256: string | null;
  cliActualRenderInput: JsonValue | null;
  renderError: string | null;
}

export interface NativeRenderReceipt {
  schema: "katana-mermaid-native-render-bindings-v1";
  status: "completed" | "failed";
  diagnosticOnly: true;
  accepted: false;
  referenceMode: "strictVendorOriginal";
  referenceScope: "formal";
  workingDirectory: string;
  formalAccepted: false;
  effectiveVendorConfigObserved: false;
  vendorCacheBranchObserved: false;
  externalRuntimeConfigObserved: false;
  expectedFenceCount: number;
  expectedNonemptyCount: number;
  outputCount: number;
  sourceManifestPath: string;
  sourceManifestSha256: string;
  sourceDigest: string;
  sourceFileCount: number;
  sourceParserPath: string;
  sourceParserSha256Before: string;
  sourceParserSha256After: string;
  sourceCurrentPins: FileRecord[];
  codePins: FileRecord[];
  buildReceiptPath: string;
  buildReceiptSha256: string;
  cargoLockPath: string;
  cargoLockSha256: string;
  binaryPath: string;
  binarySha256Before: string;
  binarySha256: string;
  binarySha256After: string;
  runtimePath: string;
  runtimeSha256: string;
  runtimeAssetPins: FileRecord[];
  skippedEmpty: Array<{ slug: string; reason: string }>;
  renderConcurrency: 1 | 2;
  errors: string[];
  rows: NativeRenderRow[];
}

export interface VerifiedNativeRenderBinding {
  receiptSha256: string;
  row: NativeRenderRow;
  receipt: NativeRenderReceipt;
}

if (import.meta.main) {
  void run(parseNativeRenderOptions(process.argv.slice(2))).catch((error) => {
    const detail = error instanceof Error ? error.message : String(error);
    console.error(detail);
    process.exitCode = 1;
  });
}

export function requireNativeRenderBinding(
  bindingsPath: string,
  fixture: Fixture,
  renderedSvg: Buffer,
): { receiptSha256: string; row: NativeRenderRow; receipt: NativeRenderReceipt } {
  if (fixture.originalFenceSource === undefined || fixture.canonicalSource === undefined) {
    throw new Error(`Native render source is incomplete for ${fixture.slug}`);
  }
  if (!fs.existsSync(bindingsPath) || fs.lstatSync(bindingsPath).isSymbolicLink()) {
    throw new Error(`Native render bindings are missing or unsafe: ${bindingsPath}`);
  }
  const parsed = parseJsonValue(fs.readFileSync(bindingsPath, "utf8"), "native render bindings");
  const receipt = parseReceipt(parsed);
  const current = verifyReceiptPins(receipt);
  const row = receipt.rows.find((candidate) => candidate.slug === fixture.slug);
  if (row === undefined) throw new Error(`Native render binding is missing ${fixture.slug}`);
  validateNativeRow(receipt, current, row, fixture, renderedSvg, path.dirname(bindingsPath));
  return { receiptSha256: fileSha256(bindingsPath), row, receipt };
}

export function requireNativeRenderInventory(
  bindingsPath: string,
  fixtures: Fixture[],
  svgDirectory: string,
): Map<string, VerifiedNativeRenderBinding> {
  assertDirectoryWithoutSymlink(svgDirectory, "native render SVG directory");
  if (
    path.resolve(bindingsPath) !== path.join(path.resolve(svgDirectory), "render-bindings.json") ||
    !fs.existsSync(bindingsPath) ||
    fs.lstatSync(bindingsPath).isSymbolicLink() ||
    !fs.lstatSync(bindingsPath).isFile()
  ) {
    throw new Error("Native render bindings must be a regular file beside the source SVGs");
  }
  const receiptSha256 = fileSha256(bindingsPath);
  const parsed = parseJsonValue(fs.readFileSync(bindingsPath, "utf8"), "native render bindings");
  const receipt = parseReceipt(parsed);
  const current = verifyReceiptPins(receipt);
  const nonempty = fixtures.filter((fixture) => (fixture.originalFenceSource ?? "").trim() !== "");
  const emptySlugs = fixtures
    .filter((fixture) => (fixture.originalFenceSource ?? "").trim() === "")
    .map((fixture) => fixture.slug)
    .sort();
  const skippedSlugs = receipt.skippedEmpty.map((item) => item.slug).sort();
  for (const fixture of fixtures) {
    assertRegularFile(fixture.filePath, `fixture source ${fixture.slug}`);
  }
  const expectedSlugs = nonempty.map((fixture) => fixture.slug).sort();
  const actualSlugs = receipt.rows.map((row) => row.slug).sort();
  if (
    receipt.expectedFenceCount !== fixtures.length ||
    receipt.expectedNonemptyCount !== nonempty.length ||
    receipt.rows.length !== nonempty.length ||
    JSON.stringify(emptySlugs) !== JSON.stringify(skippedSlugs) ||
    receipt.skippedEmpty.some((item) => item.reason !== "empty captured fence body") ||
    JSON.stringify(expectedSlugs) !== JSON.stringify(actualSlugs)
  ) {
    throw new Error("Native render bindings do not cover the exact fixture inventory");
  }
  const bindings = new Map<string, VerifiedNativeRenderBinding>();
  for (const fixture of nonempty) {
    const svgPath = path.join(svgDirectory, `${fixture.slug}.svg`);
    if (!fs.existsSync(svgPath) || fs.lstatSync(svgPath).isSymbolicLink()) {
      throw new Error(`Native render SVG is missing or unsafe: ${svgPath}`);
    }
    const row = receipt.rows.find((candidate) => candidate.slug === fixture.slug);
    if (row === undefined) throw new Error(`Native render binding is missing ${fixture.slug}`);
    validateNativeRow(receipt, current, row, fixture, fs.readFileSync(svgPath), svgDirectory);
    if (
      fileSha256(bindingsPath) !== receiptSha256 ||
      path.basename(row.outputPath) !== `${fixture.slug}.svg`
    ) {
      throw new Error(`Native render binding path or receipt changed for ${fixture.slug}`);
    }
    bindings.set(fixture.slug, { receiptSha256, row, receipt });
  }
  if (fileSha256(bindingsPath) !== receiptSha256) {
    throw new Error("Native render bindings changed during inventory verification");
  }
  return bindings;
}

function validateNativeRow(
  receipt: NativeRenderReceipt,
  build: NativeBuildReceiptSummary,
  row: NativeRenderRow,
  fixture: Fixture,
  renderedSvg: Buffer,
  bindingsDirectory: string,
) {
  if (fixture.originalFenceSource === undefined || fixture.canonicalSource === undefined) {
    throw new Error(`Native render source is incomplete for ${fixture.slug}`);
  }
  const canonicalBytes = Buffer.from(fixture.canonicalSource, "utf8");
  const markdownPath = path.resolve(fixture.filePath);
  const inputPath = path.join(bindingsDirectory, `${fixture.slug}.request.mmd`);
  const outputPath = path.join(bindingsDirectory, `${fixture.slug}.svg`);
  const stdoutPath = path.join(bindingsDirectory, row.stdoutPath);
  const stderrPath = path.join(bindingsDirectory, row.stderrPath);
  const attestationPath = path.join(bindingsDirectory, `${fixture.slug}.cli-attestation.json`);
  validateFixtureTempDirectory(row, bindingsDirectory);
  if (row.cliAttestationPath !== path.basename(attestationPath)) {
    throw new Error(`Native render attestation path differs for ${fixture.slug}`);
  }
  const expectedArgv = nativeRenderArgv(
    receipt.binaryPath,
    inputPath,
    outputPath,
    receipt.runtimePath,
    attestationPath,
    row.cliAttemptToken,
  );
  if (
    receipt.status !== "completed" ||
    receipt.referenceMode !== "strictVendorOriginal" ||
    receipt.referenceScope !== "formal" ||
    receipt.workingDirectory !== path.resolve(process.cwd()) ||
    receipt.formalAccepted !== false ||
    receipt.diagnosticOnly !== true ||
    receipt.accepted !== false ||
    receipt.effectiveVendorConfigObserved !== false ||
    receipt.vendorCacheBranchObserved !== false ||
    receipt.externalRuntimeConfigObserved !== false ||
    receipt.expectedFenceCount < receipt.expectedNonemptyCount ||
    receipt.outputCount !== receipt.expectedNonemptyCount ||
    receipt.rows.length !== receipt.expectedNonemptyCount ||
    receipt.buildReceiptSha256 !== build.receiptSha256 ||
    receipt.sourceDigest !== build.sourceDigest ||
    receipt.binaryPath !== build.binaryPath ||
    receipt.binarySha256 !== build.binarySha256 ||
    receipt.binarySha256Before !== build.binarySha256 ||
    receipt.binarySha256After !== build.binarySha256 ||
    receipt.runtimePath !== path.resolve(receipt.runtimePath) ||
    row.markdownPath !== markdownPath ||
    row.inputMarkdownSha256 !== fileSha256(markdownPath) ||
    row.inputMarkdownSha256After !== fileSha256(markdownPath) ||
    row.inputPath !== inputPath ||
    row.inputSha256 !== sha256(canonicalBytes) ||
    row.inputSha256After !== sha256(canonicalBytes) ||
    row.originalFenceSourceSha256 !== sha256(Buffer.from(fixture.originalFenceSource, "utf8")) ||
    row.canonicalSourceSha256 !== sha256(canonicalBytes) ||
    row.sourceAdapterPolicy !== sourceAdapterPolicy(fixture) ||
    receipt.sourceParserPath !==
      path.relative(
        process.cwd(),
        path.resolve("crates/katana-render-runtime-cli/src/diagram_source.rs"),
      ) ||
    receipt.sourceParserSha256Before !==
      fileSha256(path.resolve("crates/katana-render-runtime-cli/src/diagram_source.rs")) ||
    receipt.sourceParserSha256After !== receipt.sourceParserSha256Before ||
    row.sourceParserSha256Before !== receipt.sourceParserSha256Before ||
    row.sourceParserSha256After !== receipt.sourceParserSha256After ||
    row.binaryPath !== receipt.binaryPath ||
    row.binarySha256Before !== receipt.binarySha256 ||
    row.binarySha256 !== receipt.binarySha256 ||
    row.binarySha256After !== receipt.binarySha256 ||
    row.runtimePath !== receipt.runtimePath ||
    row.runtimeSha256 !== receipt.runtimeSha256 ||
    row.buildReceiptSha256 !== receipt.buildReceiptSha256 ||
    row.outputPath !== `${fixture.slug}.svg` ||
    row.argv.join("\0") !== expectedArgv.join("\0") ||
    row.pid === null ||
    row.exitStatus !== 0 ||
    row.signal !== null ||
    row.spawnError !== null ||
    row.renderError !== null ||
    row.outputSvgSha256 !== sha256(renderedSvg) ||
    !isSha256(row.stdoutSha256) ||
    !isSha256(row.stderrSha256) ||
    !isSafeSibling(stdoutPath, bindingsDirectory) ||
    !isSafeSibling(stderrPath, bindingsDirectory) ||
    !fs.existsSync(stdoutPath) ||
    !fs.existsSync(stderrPath) ||
    fileSha256(stdoutPath) !== row.stdoutSha256 ||
    fileSha256(stderrPath) !== row.stderrSha256 ||
    fileSha256(outputPath) !== row.outputSvgSha256 ||
    fileSha256(inputPath) !== row.inputSha256 ||
    fileSha256(attestationPath) !== row.cliAttestationSha256
  ) {
    throw new Error(
      `Native render binding does not match current source/output for ${fixture.slug}`,
    );
  }
  validateCliAttestation(
    attestationPath,
    row,
    canonicalBytes,
    outputPath,
    receipt.runtimePath,
    receipt.binaryPath,
  );
  const attestation = record(
    parseJsonValue(fs.readFileSync(attestationPath, "utf8"), "CLI render attestation"),
    "CLI render attestation",
  );
  if (row.attestedTempDir !== scalarString(attestation.tempDir, "CLI temporary directory")) {
    throw new Error(
      `Native render binding has an unbound CLI temporary directory for ${fixture.slug}`,
    );
  }
}

function verifyReceiptPins(receipt: NativeRenderReceipt): NativeBuildReceiptSummary {
  const build = verifyNativeBuildReceipt(
    receipt.buildReceiptPath,
    receipt.binaryPath,
    receipt.runtimePath,
  );
  if (
    build.receiptSha256 !== receipt.buildReceiptSha256 ||
    build.sourceManifestPath !== receipt.sourceManifestPath ||
    build.sourceManifestSha256 !== receipt.sourceManifestSha256 ||
    build.sourceDigest !== receipt.sourceDigest ||
    build.sourceFileCount !== receipt.sourceFileCount ||
    build.binarySha256 !== receipt.binarySha256 ||
    build.cargoLockPath !== receipt.cargoLockPath ||
    build.cargoLockSha256 !== receipt.cargoLockSha256 ||
    !sameFileRecords(build.sourceFiles, receipt.sourceCurrentPins) ||
    !sameFileRecords(captureCodePins(), receipt.codePins) ||
    JSON.stringify(build.runtimeAssetPins) !== JSON.stringify(receipt.runtimeAssetPins)
  ) {
    throw new Error("Native render bindings differ from the current native build receipt");
  }
  for (const pin of receipt.sourceCurrentPins)
    verifyCurrentFileRecord(pin, "native source code pin");
  return build;
}

function verifyCurrentFileRecord(item: FileRecord, label: string) {
  const actualPath = canonicalRegularFile(item.path, label);
  if (
    actualPath !== item.path ||
    fs.statSync(actualPath).size !== item.size ||
    fileSha256(actualPath) !== item.sha256
  ) {
    throw new Error(`${label} changed after native rendering`);
  }
}

export function validateCliAttestation(
  attestationPath: string,
  row: CliAttestationExpected,
  canonicalBytes: Buffer,
  outputPath: string,
  runtimePath: string,
  binaryPath: string,
) {
  if (!fs.existsSync(attestationPath) || fs.lstatSync(attestationPath).isSymbolicLink()) {
    throw new Error(`CLI render attestation is missing or unsafe: ${attestationPath}`);
  }
  const parsed = parseJsonValue(fs.readFileSync(attestationPath, "utf8"), "CLI render attestation");
  const cli = record(parsed, "CLI render attestation");
  const input = cliFileRecord(cli.input, "CLI input", row.inputPath);
  const runtime = cliFileRecord(cli.runtime, "CLI runtime", runtimePath);
  const executable = cliFileRecord(cli.executable, "CLI executable", binaryPath);
  const artifacts = record(cli.artifacts, "CLI artifacts");
  const artifactDir = path.join(
    path.dirname(attestationPath),
    `${path.basename(attestationPath, ".json")}.artifacts`,
  );
  const rawInput = cliFileRecord(
    artifacts.rawInput,
    "raw input artifact",
    path.join(artifactDir, "raw-input.bin"),
  );
  const renderSource = cliFileRecord(
    artifacts.renderSource,
    "render source artifact",
    path.join(artifactDir, "request-source.mmd"),
  );
  const renderInput = cliFileRecord(
    artifacts.renderInput,
    "render input artifact",
    path.join(artifactDir, "render-input.json"),
  );
  const outputArtifact = cliFileRecord(
    artifacts.output,
    "output artifact",
    path.join(artifactDir, "output.svg"),
  );
  const actualRenderInput = record(cli.actualRenderInput, "actual RenderInput");
  const bindingsBefore = record(cli.bindingsBefore, "pre-render CLI bindings");
  const bindingsAfter = record(cli.bindingsAfter, "post-render CLI bindings");
  const runtimeMetadata = record(cli.runtimeMetadata, "CLI runtime metadata");
  const declaredRuntime = record(runtimeMetadata.declaredRuntime, "declared runtime");
  const profile = record(runtimeMetadata.profile, "CLI profile");
  const diagnostics = record(cli.diagnostics, "CLI diagnostics");
  const declaredRuntimeName = scalarString(declaredRuntime.name, "declared runtime name");
  const declaredRuntimeVersion = scalarString(declaredRuntime.version, "declared runtime version");
  const declaredRuntimeChecksum = nullableString(declaredRuntime, "checksum");
  const profileId = scalarString(profile.id, "CLI profile id");
  nullableString(profile, "description");
  const output = cliFileRecord(cli.output, "CLI output", outputPath);
  const tempDir = scalarString(cli.tempDir, "CLI temporary directory");
  const hasClassicCapture = Object.hasOwn(artifacts, "classicCapture");
  if (hasClassicCapture) {
    verifyNativeClassicCapture(artifacts.classicCapture, artifactDir, canonicalBytes, [
      input,
      runtime,
      executable,
      output,
      rawInput,
      renderSource,
      renderInput,
      outputArtifact,
    ]);
  }
  if (
    scalarString(cli.schema, "schema") !== "katana-native-render-attestation-v1" ||
    scalarString(cli.status, "status") !== "output-written" ||
    cli.accepted !== false ||
    cli.actualExit !== null ||
    cli.requiresExternalExitEvidence !== true ||
    scalarString(cli.cwd, "CLI cwd") !== path.resolve(process.cwd()) ||
    scalarString(cli.scope, "CLI attestation scope") !==
      (hasClassicCapture ? CLASSIC_CLI_ATTESTATION_SCOPE : LEGACY_CLI_ATTESTATION_SCOPE) ||
    scalarString(cli.cliVersion, "CLI version").length === 0 ||
    !path.isAbsolute(tempDir) ||
    path.resolve(tempDir) !== tempDir ||
    fs.realpathSync(tempDir) !== tempDir ||
    (row.tempRoot !== undefined && tempDir !== row.tempRoot) ||
    declaredRuntimeName.length === 0 ||
    declaredRuntimeVersion.length === 0 ||
    declaredRuntimeChecksum !== runtime.sha256 ||
    profileId.length === 0 ||
    !Array.isArray(diagnostics.warnings) ||
    diagnostics.warnings.some((warning: unknown) => typeof warning !== "string") ||
    !Array.isArray(diagnostics.errors) ||
    diagnostics.errors.some((error: unknown) => typeof error !== "string") ||
    scalarString(cli.attemptToken, "attempt token") !== row.cliAttemptToken ||
    scalarNumber(cli.pid, "CLI pid") !== row.pid ||
    scalarString(cli.inputFormat, "CLI input format") !== "diagram-source" ||
    scalarString(cli.actualRenderSourceSha256, "actual source SHA") !== sha256(canonicalBytes) ||
    scalarString(actualRenderInput.kind, "RenderInput kind") !== "Mermaid" ||
    scalarString(actualRenderInput.source, "RenderInput source") !==
      canonicalBytes.toString("utf8") ||
    !defaultRenderInputOptions(actualRenderInput) ||
    !sameCliFileRecord(
      cliFileRecord(bindingsBefore.input, "pre-render input", input.path),
      input,
    ) ||
    !sameCliFileRecord(
      cliFileRecord(bindingsAfter.input, "post-render input", input.path),
      input,
    ) ||
    !sameCliFileRecord(
      cliFileRecord(bindingsBefore.runtime, "pre-render runtime", runtime.path),
      runtime,
    ) ||
    !sameCliFileRecord(
      cliFileRecord(bindingsAfter.runtime, "post-render runtime", runtime.path),
      runtime,
    ) ||
    !sameCliFileRecord(
      cliFileRecord(bindingsBefore.executable, "pre-render executable", executable.path),
      executable,
    ) ||
    !sameCliFileRecord(
      cliFileRecord(bindingsAfter.executable, "post-render executable", executable.path),
      executable,
    ) ||
    input.path !== row.inputPath ||
    input.sha256 !== row.inputSha256 ||
    runtime.path !== runtimePath ||
    runtime.sha256 !== row.runtimeSha256 ||
    executable.path !== binaryPath ||
    executable.sha256 !== row.binarySha256 ||
    output.path !== outputPath ||
    output.sha256 !== row.outputSvgSha256 ||
    rawInput.sha256 !== sha256(canonicalBytes) ||
    renderSource.sha256 !== sha256(canonicalBytes) ||
    outputArtifact.sha256 !== row.outputSvgSha256 ||
    fs.readFileSync(rawInput.path).compare(canonicalBytes) !== 0 ||
    fs.readFileSync(renderSource.path).compare(canonicalBytes) !== 0 ||
    fs.readFileSync(outputArtifact.path).compare(fs.readFileSync(outputPath)) !== 0 ||
    !sameJsonValue(
      parseJsonValue(fs.readFileSync(renderInput.path, "utf8"), "serialized RenderInput"),
      actualRenderInput,
    ) ||
    row.actualRenderSourceSha256 !== cli.actualRenderSourceSha256 ||
    row.cliAttestationSha256 !== fileSha256(attestationPath)
  ) {
    throw new Error("CLI render attestation does not prove the fixed canonical source/output");
  }
}

function validateFixtureTempDirectory(row: NativeRenderRow, bindingsDirectory: string): void {
  const outputDirectory = fs.realpathSync(bindingsDirectory);
  const relative = path.relative(outputDirectory, row.tempRoot);
  if (
    !path.isAbsolute(row.tempRoot) ||
    path.resolve(row.tempRoot) !== row.tempRoot ||
    fs.realpathSync(row.tempRoot) !== row.tempRoot ||
    !fs.lstatSync(row.tempRoot).isDirectory() ||
    relative === "" ||
    relative === ".." ||
    relative.startsWith(`..${path.sep}`) ||
    path.isAbsolute(relative)
  ) {
    throw new Error(`Native CLI temporary directory is not a canonical output child: ${row.slug}`);
  }
}

function defaultRenderInputOptions(input: { [key: string]: JsonValue }): boolean {
  const config = record(input.config, "RenderInput config");
  const context = record(input.context, "RenderInput context");
  const policy = record(input.policy, "RenderInput policy");
  return (
    config.vendor_config === null &&
    context.document_id === null &&
    context.theme === null &&
    context.theme_fingerprint === null &&
    policy.background === null &&
    policy.cache_profile === null &&
    policy.max_height === null &&
    policy.max_width === null &&
    policy.padding === null
  );
}

function cliFileRecord(
  value: JsonValue | undefined,
  label: string,
  expectedPath: string,
): FileRecord {
  const item = record(value, label);
  const file = {
    path: scalarString(item.path, `${label} path`),
    sha256: scalarString(item.sha256, `${label} SHA`),
    size: scalarNumber(item.size, `${label} size`),
  };
  if (!isSha256(file.sha256)) throw new Error(`Invalid ${label} SHA`);
  if (file.path !== expectedPath) throw new Error(`${label} path differs from the owned attempt`);
  canonicalRegularFile(file.path, label);
  if (fs.statSync(file.path).size !== file.size || fileSha256(file.path) !== file.sha256) {
    throw new Error(`${label} bytes differ from CLI attestation`);
  }
  return file;
}

function sameCliFileRecord(left: FileRecord, right: FileRecord): boolean {
  return left.path === right.path && left.sha256 === right.sha256 && left.size === right.size;
}

function sameJsonValue(left: JsonValue, right: JsonValue): boolean {
  if (left === right) return true;
  if (Array.isArray(left) && Array.isArray(right)) {
    return (
      left.length === right.length &&
      left.every((item, index) => {
        const other = right[index];
        return other !== undefined && sameJsonValue(item, other);
      })
    );
  }
  if (!isJsonRecord(left) || !isJsonRecord(right)) {
    return false;
  }
  const leftKeys = Object.keys(left).sort();
  const rightKeys = Object.keys(right).sort();
  return (
    leftKeys.length === rightKeys.length &&
    leftKeys.every((key, index) => {
      const leftValue = left[key];
      const rightValue = right[key];
      return (
        key === rightKeys[index] &&
        leftValue !== undefined &&
        rightValue !== undefined &&
        sameJsonValue(leftValue, rightValue)
      );
    })
  );
}

function isJsonRecord(value: JsonValue | undefined): value is { [key: string]: JsonValue } {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function nativeRenderArgv(
  binary: string,
  input: string,
  output: string,
  runtime: string,
  attestation: string,
  token: string,
): string[] {
  return [
    binary,
    "mermaid",
    "render",
    "--input",
    input,
    "--output",
    output,
    "--runtime",
    runtime,
    "--input-format",
    "diagram-source",
    "--render-attestation",
    attestation,
    "--attempt-token",
    token,
  ];
}

function sourceAdapterPolicy(fixture: Fixture): string {
  return fixture.fenceLanguage === "zenuml" ? "zenuml-prefix-v1" : "mermaid-fence-lines-v1";
}

function captureCodePins(): FileRecord[] {
  const relativePaths = [
    "Justfile",
    "scripts/mermaid/native-render-provenance.ts",
    "scripts/mermaid/native-cli-process.ts",
    "scripts/mermaid/native-classic-capture.ts",
    "scripts/mermaid/native-build-receipt.ts",
    "scripts/mermaid/diagram_update_fixtures.ts",
    "scripts/mermaid/mermaid-pair-manifest.ts",
    "scripts/mermaid/official-renderer.ts",
    "scripts/mermaid/official-renderer-provenance.ts",
    "scripts/mermaid/rasterize-svg-dir.ts",
    "scripts/mermaid/raster-comparison-binding.ts",
    "scripts/mermaid/raster-production-binding.ts",
    "scripts/mermaid/reference-compare.ts",
    "scripts/mermaid/reference_pair_repository.ts",
    "scripts/mermaid/strict-svg-external-images.ts",
    "scripts/mermaid/svg-viewbox-size.ts",
    "package.json",
    "bun.lock",
  ];
  return relativePaths.map((relativePath) => currentFileRecord(path.resolve(relativePath)));
}

function currentFileRecord(filePath: string): FileRecord {
  const canonicalPath = canonicalRegularFile(filePath, "current source pin");
  return {
    path: canonicalPath,
    sha256: fileSha256(canonicalPath),
    size: fs.statSync(canonicalPath).size,
  };
}

export function parseNativeRenderOptions(argv: string[]): Options {
  const fixturesDir = value(argv, "--fixtures-dir");
  const fixtureGlob = value(argv, "--fixture-glob");
  const outputDir = value(argv, "--output-dir");
  const binary = value(argv, "--binary");
  const runtime = value(argv, "--runtime");
  const buildReceipt = value(argv, "--build-receipt");
  const concurrencyFlags = argv.filter((argument) => argument === "--render-concurrency");
  if (concurrencyFlags.length > 1) {
    throw new Error("--render-concurrency must be specified at most once");
  }
  const concurrencyIndex = argv.indexOf("--render-concurrency");
  const renderConcurrency = concurrencyIndex < 0 ? "2" : argv.at(concurrencyIndex + 1);
  if (
    fixturesDir === undefined ||
    fixtureGlob === undefined ||
    outputDir === undefined ||
    binary === undefined ||
    runtime === undefined ||
    buildReceipt === undefined ||
    buildReceipt.length === 0
  ) {
    throw new Error(
      "Usage: native-render-provenance.ts --fixtures-dir DIR --fixture-glob GLOB --output-dir DIR --binary FILE --runtime FILE --build-receipt FILE [--render-concurrency 1|2]",
    );
  }
  if (renderConcurrency !== "1" && renderConcurrency !== "2") {
    throw new Error("--render-concurrency must be 1 or 2");
  }
  return {
    fixturesDir: path.resolve(fixturesDir),
    fixtureGlob,
    outputDir: path.resolve(outputDir),
    binary: path.resolve(binary),
    runtime: path.resolve(runtime),
    buildReceipt: path.resolve(buildReceipt),
    renderConcurrency: renderConcurrency === "1" ? 1 : 2,
  };
}

async function run(input: Options) {
  const prepared = prepareRender(input);
  fs.mkdirSync(input.outputDir, { recursive: true });
  assertDirectoryWithoutSymlink(input.outputDir, "native renderer output directory");
  const rendered = prepared.fixtures.filter(
    (fixture) => (fixture.originalFenceSource ?? "").trim() !== "",
  );
  const skippedEmpty = prepared.fixtures
    .filter((fixture) => (fixture.originalFenceSource ?? "").trim() === "")
    .map((fixture) => ({ slug: fixture.slug, reason: "empty captured fence body" }));
  const renderedResult = await renderFixtures(
    rendered,
    input,
    prepared.build,
    input.renderConcurrency,
  );
  const rows = renderedResult.rows;
  const afterBuild = verifyNativeBuildReceipt(input.buildReceipt, input.binary, input.runtime);
  const afterCodePins = captureCodePins();
  const codeStable = sameFileRecords(prepared.beforeCodePins, afterCodePins);
  const success =
    codeStable &&
    afterBuild.receiptSha256 === prepared.build.receiptSha256 &&
    afterBuild.sourceDigest === prepared.build.sourceDigest &&
    rows.length === rendered.length &&
    rows.every(
      (row) =>
        row.exitStatus === 0 &&
        row.spawnError === null &&
        row.renderError === null &&
        row.inputMarkdownSha256 === row.inputMarkdownSha256After &&
        row.outputSvgSha256 !== null,
    ) &&
    renderedResult.errors.length === 0;
  const receipt = {
    schema: "katana-mermaid-native-render-bindings-v1",
    diagnosticOnly: true,
    accepted: false,
    formalAccepted: false,
    referenceMode: "strictVendorOriginal",
    referenceScope: "formal",
    workingDirectory: process.cwd(),
    sourceParserPath: path.relative(process.cwd(), prepared.sourceParserPath),
    sourceParserSha256Before: prepared.sourceParserSha256,
    sourceParserSha256After: fileSha256(prepared.sourceParserPath),
    effectiveVendorConfigObserved: false,
    vendorCacheBranchObserved: false,
    externalRuntimeConfigObserved: false,
    binaryPath: input.binary,
    binarySha256Before: prepared.build.binarySha256,
    binarySha256: prepared.build.binarySha256,
    binarySha256After: fileSha256(input.binary),
    runtimePath: input.runtime,
    runtimeSha256:
      prepared.build.runtimeAssetPins.find((item) => item.path === input.runtime)?.sha256 ?? "",
    sourceManifestPath: prepared.build.sourceManifestPath,
    sourceManifestSha256: prepared.build.sourceManifestSha256,
    sourceDigest: prepared.build.sourceDigest,
    sourceFileCount: prepared.build.sourceFileCount,
    sourceCurrentPins: prepared.build.sourceFiles,
    codePins: prepared.beforeCodePins,
    buildReceiptPath: prepared.build.receiptPath,
    buildReceiptSha256: prepared.build.receiptSha256,
    cargoLockPath: prepared.build.cargoLockPath,
    cargoLockSha256: prepared.build.cargoLockSha256,
    runtimeAssetPins: prepared.build.runtimeAssetPins,
    fixturesDir: input.fixturesDir,
    fixtureGlob: input.fixtureGlob,
    expectedFenceCount: prepared.fixtures.length,
    expectedNonemptyCount: rendered.length,
    skippedEmpty,
    outputCount: rows.length,
    renderConcurrency: input.renderConcurrency,
    errors: renderedResult.errors,
    status: success ? "completed" : "failed",
    rows,
  };
  const receiptPath = path.join(
    input.outputDir,
    success ? "render-bindings.json" : "render-bindings.failed.json",
  );
  fs.writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`, { flag: "wx" });
  if (!success)
    throw new Error(
      `Native renderer did not produce a complete strict source binding receipt: ${receiptPath}`,
    );
}

function prepareRender(input: Options) {
  assertDirectoryWithoutSymlink(input.fixturesDir, "native renderer fixture directory");
  assertFreshDirectory(input.outputDir);
  assertRegularFile(input.binary, "native renderer binary");
  assertRegularFile(input.runtime, "native Mermaid runtime");
  if (
    input.binary !== fs.realpathSync(input.binary) ||
    input.runtime !== fs.realpathSync(input.runtime)
  ) {
    throw new Error("Native render binary and runtime paths must be canonical");
  }
  const build = verifyNativeBuildReceipt(input.buildReceipt, input.binary, input.runtime);
  const beforeCodePins = captureCodePins();
  const sourceParserPath = path.resolve("crates/katana-render-runtime-cli/src/diagram_source.rs");
  if (!build.sourceFiles.some((item) => item.path === sourceParserPath)) {
    throw new Error("Native build source manifest does not include the Mermaid source parser");
  }
  const fixtures = new FixtureRepository(input.fixturesDir)
    .list()
    .filter((fixture) => matchesGlob(fixture.fileName, input.fixtureGlob));
  validateFixtures(fixtures);
  return {
    build,
    beforeCodePins,
    sourceParserPath,
    sourceParserSha256: fileSha256(sourceParserPath),
    fixtures,
  };
}

function validateFixtures(fixtures: Fixture[]) {
  if (fixtures.length === 0) throw new Error("Native renderer fixture inventory is empty");
  const slugs = fixtures.map((fixture) => fixture.slug);
  if (new Set(slugs).size !== slugs.length) {
    throw new Error("Native renderer fixture slugs are not unique");
  }
  for (const fixture of fixtures) {
    assertRegularFile(fixture.filePath, `fixture source ${fixture.slug}`);
    if (
      (fixture.originalFenceSource ?? "").trim() !== "" &&
      fixture.canonicalSource === undefined
    ) {
      throw new Error(`Canonical Mermaid source is missing for ${fixture.slug}`);
    }
  }
}

async function renderFixtures(
  fixtures: Fixture[],
  input: Options,
  build: NativeBuildReceiptSummary,
  concurrency: 1 | 2,
): Promise<{ rows: NativeRenderRow[]; errors: string[] }> {
  const batch = await renderNativeFixtureBatch(fixtures, concurrency, async (fixture) => {
    const row = await renderFixture(fixture, input, build);
    return { row, renderError: row.renderError } satisfies NativeRenderBatchResult;
  });
  return {
    rows: batch.results.flatMap(({ result }) => (result.row === null ? [] : [result.row])),
    errors: batch.errors,
  };
}

async function renderFixture(
  fixture: Fixture,
  input: Options,
  build: NativeBuildReceiptSummary,
): Promise<NativeRenderRow> {
  if (fixture.originalFenceSource === undefined || fixture.canonicalSource === undefined) {
    throw new Error(`Native render source is incomplete for ${fixture.slug}`);
  }
  const canonicalBytes = Buffer.from(fixture.canonicalSource, "utf8");
  const sourceParserPath = path.resolve("crates/katana-render-runtime-cli/src/diagram_source.rs");
  const sourceParserPin = build.sourceFiles.find((item) => item.path === sourceParserPath);
  if (sourceParserPin === undefined) {
    throw new Error("Native build source manifest does not include the Mermaid source parser");
  }
  const markdownPath = path.resolve(fixture.filePath);
  const inputPath = path.join(input.outputDir, `${fixture.slug}.request.mmd`);
  const outputPath = path.join(input.outputDir, `${fixture.slug}.svg`);
  const stdoutPath = path.join(input.outputDir, `${fixture.slug}.stdout.log`);
  const stderrPath = path.join(input.outputDir, `${fixture.slug}.stderr.log`);
  const attestationPath = path.join(input.outputDir, `${fixture.slug}.cli-attestation.json`);
  const outputDirectory = fs.realpathSync(input.outputDir);
  const tempRoot = fs.realpathSync(
    fs.mkdtempSync(path.join(outputDirectory, `${fixture.slug}.tmp-`)),
  );
  const token = randomUUID();
  fs.writeFileSync(inputPath, canonicalBytes, { flag: "wx" });
  const inputSha256 = fileSha256(inputPath);
  const inputMarkdownSha256 = fileSha256(markdownPath);
  const argv = nativeRenderArgv(
    input.binary,
    inputPath,
    outputPath,
    input.runtime,
    attestationPath,
    token,
  );
  const child = await spawnNativeCli(input.binary, argv, stdoutPath, stderrPath, tempRoot);
  const outputSvgSha256 = safeFileSha256(outputPath);
  const cliAttestationSha256 = safeFileSha256(attestationPath);
  let cliActualRenderInput: JsonValue | null = null;
  let actualRenderSourceSha256: string | null = null;
  let attestedTempDir: string | null = null;
  let renderError: string | null = null;
  if (
    child.pid === null ||
    child.exitStatus !== 0 ||
    child.signal !== null ||
    child.spawnError !== null ||
    outputSvgSha256 === null
  ) {
    renderError = "Owned native CLI did not exit successfully with an SVG output";
  } else {
    try {
      validateCliAttestation(
        attestationPath,
        {
          cliAttemptToken: token,
          pid: child.pid,
          inputPath,
          inputSha256,
          binarySha256: build.binarySha256,
          runtimeSha256: fileSha256(input.runtime),
          outputSvgSha256,
          cliAttestationSha256,
          actualRenderSourceSha256: sha256(canonicalBytes),
          tempRoot,
        },
        canonicalBytes,
        outputPath,
        input.runtime,
        input.binary,
      );
      const attestation = record(
        parseJsonValue(fs.readFileSync(attestationPath, "utf8"), "CLI render attestation"),
        "CLI render attestation",
      );
      actualRenderSourceSha256 = scalarString(
        attestation.actualRenderSourceSha256,
        "actual source SHA",
      );
      attestedTempDir = scalarString(attestation.tempDir, "CLI temporary directory");
      cliActualRenderInput = attestation.actualRenderInput ?? null;
    } catch (error) {
      renderError = error instanceof Error ? error.message : String(error);
    }
  }
  const stdoutSha256 = fileSha256(stdoutPath);
  const stderrSha256 = fileSha256(stderrPath);
  return {
    slug: fixture.slug,
    markdownPath,
    inputMarkdownSha256,
    inputPath,
    inputSha256,
    inputSha256After: safeFileSha256(inputPath) ?? "",
    originalFenceSourceSha256: sha256(Buffer.from(fixture.originalFenceSource, "utf8")),
    canonicalSourceSha256: sha256(canonicalBytes),
    sourceAdapterPolicy: sourceAdapterPolicy(fixture),
    sourceParserSha256Before: sourceParserPin.sha256,
    sourceParserSha256After: fileSha256(sourceParserPath),
    binaryPath: input.binary,
    binarySha256Before: build.binarySha256,
    binarySha256: build.binarySha256,
    binarySha256After: build.binarySha256,
    runtimePath: input.runtime,
    runtimeSha256: fileSha256(input.runtime),
    buildReceiptSha256: build.receiptSha256,
    cliAttemptToken: token,
    argv,
    pid: child.pid,
    exitStatus: child.exitStatus,
    signal: child.signal,
    spawnError: child.spawnError,
    stdoutPath: path.basename(stdoutPath),
    stdoutSha256,
    stderrPath: path.basename(stderrPath),
    stderrSha256,
    outputPath: path.basename(outputPath),
    outputSvgSha256,
    inputMarkdownSha256After: fileSha256(markdownPath),
    cliAttestationPath: path.basename(attestationPath),
    cliAttestationSha256,
    tempRoot,
    attestedTempDir,
    actualRenderSourceSha256,
    cliActualRenderInput,
    renderError,
  };
}

function safeFileSha256(filePath: string): string | null {
  try {
    canonicalRegularFile(filePath, "native render artifact");
    return fileSha256(filePath);
  } catch {
    return null;
  }
}

function assertFreshDirectory(directory: string) {
  if (fs.existsSync(directory)) {
    if (fs.lstatSync(directory).isSymbolicLink()) {
      throw new Error(`Native render output directory must not be a symlink: ${directory}`);
    }
    if (fs.readdirSync(directory).length > 0) {
      throw new Error(`Native render requires a fresh output directory: ${directory}`);
    }
  }
}

function assertDirectoryWithoutSymlink(directory: string, label: string) {
  const stats = fs.lstatSync(directory);
  if (stats.isSymbolicLink() || !stats.isDirectory()) {
    throw new Error(`${label} is missing or uses an unsafe path: ${directory}`);
  }
}

function assertRegularFile(filePath: string, label: string) {
  const stats = fs.lstatSync(filePath);
  if (stats.isSymbolicLink() || !stats.isFile())
    throw new Error(`${label} is not a regular file: ${filePath}`);
}

function value(argv: string[], name: string): string | undefined {
  const index = argv.indexOf(name);
  return index < 0 ? undefined : argv.at(index + 1);
}

function matchesGlob(fileName: string, glob: string): boolean {
  const escaped = glob
    .replace(/[.+^${}()|[\]\\]/g, "\\$&")
    .replace(/\*/g, ".*")
    .replace(/\?/g, ".");
  return new RegExp(`^${escaped}$`).test(fileName);
}

function parseReceipt(value: JsonValue): NativeRenderReceipt {
  const root = record(value, "receipt");
  if (
    root.schema !== "katana-mermaid-native-render-bindings-v1" ||
    root.status !== "completed" ||
    root.referenceMode !== "strictVendorOriginal" ||
    root.referenceScope !== "formal" ||
    root.diagnosticOnly !== true ||
    root.accepted !== false ||
    root.formalAccepted !== false ||
    root.effectiveVendorConfigObserved !== false ||
    root.vendorCacheBranchObserved !== false ||
    root.externalRuntimeConfigObserved !== false ||
    (root.renderConcurrency !== 1 && root.renderConcurrency !== 2) ||
    !Array.isArray(root.errors) ||
    root.errors.length !== 0 ||
    !Array.isArray(root.rows) ||
    !Array.isArray(root.skippedEmpty)
  ) {
    throw new Error("Native render bindings have an unsupported schema or status");
  }
  const rows = root.rows.map((value) => {
    const row = record(value, "row");
    return {
      slug: stringField(row, "slug"),
      markdownPath: stringField(row, "markdownPath"),
      inputMarkdownSha256: stringField(row, "inputMarkdownSha256"),
      inputPath: stringField(row, "inputPath"),
      inputSha256: stringField(row, "inputSha256"),
      inputSha256After: stringField(row, "inputSha256After"),
      originalFenceSourceSha256: stringField(row, "originalFenceSourceSha256"),
      canonicalSourceSha256: stringField(row, "canonicalSourceSha256"),
      sourceAdapterPolicy: stringField(row, "sourceAdapterPolicy"),
      sourceParserSha256Before: stringField(row, "sourceParserSha256Before"),
      sourceParserSha256After: stringField(row, "sourceParserSha256After"),
      binaryPath: stringField(row, "binaryPath"),
      binarySha256Before: stringField(row, "binarySha256Before"),
      binarySha256: stringField(row, "binarySha256"),
      binarySha256After: stringField(row, "binarySha256After"),
      runtimePath: stringField(row, "runtimePath"),
      runtimeSha256: stringField(row, "runtimeSha256"),
      buildReceiptSha256: stringField(row, "buildReceiptSha256"),
      cliAttemptToken: stringField(row, "cliAttemptToken"),
      argv: stringArray(row.argv),
      pid: nullableNumber(row, "pid"),
      exitStatus: nullableNumber(row, "exitStatus"),
      signal: nullableString(row, "signal"),
      spawnError: nullableString(row, "spawnError"),
      stdoutPath: stringField(row, "stdoutPath"),
      stdoutSha256: stringField(row, "stdoutSha256"),
      stderrPath: stringField(row, "stderrPath"),
      stderrSha256: stringField(row, "stderrSha256"),
      outputPath: stringField(row, "outputPath"),
      outputSvgSha256: nullableString(row, "outputSvgSha256"),
      inputMarkdownSha256After: stringField(row, "inputMarkdownSha256After"),
      cliAttestationPath: stringField(row, "cliAttestationPath"),
      cliAttestationSha256: nullableString(row, "cliAttestationSha256"),
      tempRoot: stringField(row, "tempRoot"),
      attestedTempDir: nullableString(row, "attestedTempDir"),
      actualRenderSourceSha256: nullableString(row, "actualRenderSourceSha256"),
      cliActualRenderInput: nullableJsonValue(row.cliActualRenderInput),
      renderError: nullableString(row, "renderError"),
    } satisfies NativeRenderRow;
  });
  const skippedEmpty = root.skippedEmpty.map((value) => {
    const item = record(value, "empty fixture");
    return { slug: stringField(item, "slug"), reason: stringField(item, "reason") };
  });
  if (new Set(rows.map((row) => row.slug)).size !== rows.length) {
    throw new Error("Native render bindings contain duplicate slugs");
  }
  if (rows.some((row) => row.attestedTempDir !== row.tempRoot)) {
    throw new Error("Native render bindings contain an unbound CLI temporary directory");
  }
  if (new Set(rows.map((row) => row.tempRoot)).size !== rows.length) {
    throw new Error("Native render bindings contain duplicate CLI temporary directories");
  }
  return {
    schema: root.schema,
    status: root.status,
    diagnosticOnly: true,
    accepted: false,
    referenceMode: root.referenceMode,
    referenceScope: root.referenceScope,
    workingDirectory: stringField(root, "workingDirectory"),
    formalAccepted: false,
    effectiveVendorConfigObserved: false,
    vendorCacheBranchObserved: false,
    externalRuntimeConfigObserved: false,
    expectedFenceCount: numberField(root, "expectedFenceCount"),
    expectedNonemptyCount: numberField(root, "expectedNonemptyCount"),
    outputCount: numberField(root, "outputCount"),
    sourceManifestPath: stringField(root, "sourceManifestPath"),
    sourceManifestSha256: stringField(root, "sourceManifestSha256"),
    sourceDigest: stringField(root, "sourceDigest"),
    sourceFileCount: numberField(root, "sourceFileCount"),
    sourceParserPath: stringField(root, "sourceParserPath"),
    sourceParserSha256Before: stringField(root, "sourceParserSha256Before"),
    sourceParserSha256After: stringField(root, "sourceParserSha256After"),
    sourceCurrentPins: fileRecordArray(root.sourceCurrentPins, "source current pins"),
    codePins: fileRecordArray(root.codePins, "native code pins"),
    buildReceiptPath: stringField(root, "buildReceiptPath"),
    buildReceiptSha256: stringField(root, "buildReceiptSha256"),
    cargoLockPath: stringField(root, "cargoLockPath"),
    cargoLockSha256: stringField(root, "cargoLockSha256"),
    binaryPath: stringField(root, "binaryPath"),
    binarySha256Before: stringField(root, "binarySha256Before"),
    binarySha256: stringField(root, "binarySha256"),
    binarySha256After: stringField(root, "binarySha256After"),
    runtimePath: stringField(root, "runtimePath"),
    runtimeSha256: stringField(root, "runtimeSha256"),
    runtimeAssetPins: fileRecordArray(root.runtimeAssetPins, "runtime asset pins"),
    skippedEmpty,
    renderConcurrency: root.renderConcurrency,
    errors: stringArray(root.errors),
    rows,
  };
}

function record(value: JsonValue | undefined, label: string): { [key: string]: JsonValue } {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`Invalid native render ${label}`);
  }
  return value;
}

function fileRecordArray(value: JsonValue | undefined, label: string): FileRecord[] {
  if (!Array.isArray(value)) throw new Error(`Invalid native render ${label}`);
  return value.map((entry) => {
    const item = record(entry, label);
    return {
      path: stringField(item, "path"),
      sha256: stringField(item, "sha256"),
      size: numberField(item, "size"),
    };
  });
}

function nullableJsonValue(value: JsonValue | undefined): JsonValue | null {
  if (value === null) return null;
  if (value === undefined) throw new Error("Missing native render JSON value");
  return value;
}

function stringField(value: { [key: string]: JsonValue }, key: string): string {
  const field = value[key];
  if (typeof field !== "string") throw new Error(`Invalid native render field: ${key}`);
  return field;
}

function numberField(value: { [key: string]: JsonValue }, key: string): number {
  const field = value[key];
  if (typeof field !== "number" || !Number.isSafeInteger(field) || field < 0) {
    throw new Error(`Invalid native render field: ${key}`);
  }
  return field;
}

function nullableNumber(value: { [key: string]: JsonValue }, key: string): number | null {
  const field = value[key];
  if (field !== null && (typeof field !== "number" || !Number.isSafeInteger(field))) {
    throw new Error(`Invalid native render field: ${key}`);
  }
  return field;
}

function nullableString(value: { [key: string]: JsonValue }, key: string): string | null {
  const field = value[key];
  if (field !== null && typeof field !== "string") {
    throw new Error(`Invalid native render field: ${key}`);
  }
  return field;
}

function scalarString(value: JsonValue | undefined, label: string): string {
  if (typeof value !== "string") throw new Error(`Invalid native render field: ${label}`);
  return value;
}

function scalarNumber(value: JsonValue | undefined, label: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw new Error(`Invalid native render field: ${label}`);
  }
  return value;
}

function stringArray(value: JsonValue | undefined): string[] {
  if (!Array.isArray(value) || value.some((field) => typeof field !== "string")) {
    throw new Error("Invalid native render argv");
  }
  return value.filter((field): field is string => typeof field === "string");
}

function isSha256(value: string): boolean {
  return /^[a-f0-9]{64}$/.test(value);
}

function isSafeSibling(filePath: string, directory: string): boolean {
  if (fs.existsSync(filePath) && fs.lstatSync(filePath).isSymbolicLink()) return false;
  return path.dirname(path.resolve(filePath)) === path.resolve(directory);
}
