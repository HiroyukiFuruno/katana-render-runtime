import fs from "node:fs";
import path from "node:path";
import { parseArgs } from "node:util";
import {
  type ManagedFullCompareRequest,
  type PinnedInputs,
  parseManagedFullCompareRequest,
  validateFreshInputState,
  validateLanguageInventory,
  validatePinnedInputs,
} from "./managed-full-compare-input";
import {
  type LanguageAcceptance,
  type ManagedStep,
  runManagedLanguage,
} from "./managed-full-compare-language";
import {
  parseChildJson,
  requireCommandSuccess,
  runManagedCommand,
  writeExclusive,
} from "./managed-full-compare-process";
import { type FullScoreFailure, requireFullScoreAcceptance } from "./managed-full-compare-results";
import { fileSha256, sha256 } from "./mermaid-pair-manifest";
import {
  parseSourceManifest,
  sameFileRecords,
  verifyNativeBuildReceipt,
} from "./native-build-receipt";
import { captureNativeSourceManifest } from "./native-source-manifest";
import { physicalFileIdentity, RasterComparisonBinding } from "./raster-comparison-binding";
import { type RasterFileIdentity, sameRasterFileIdentity } from "./raster-production-binding";

interface BuildResponse {
  receipt: string;
}

export type LanguageResult = LanguageAcceptance | FullScoreFailure;

export interface RunReceipt {
  schema: "krr-mermaid-managed-full-run-v1";
  status: "completed" | "failed";
  diagnosticOnly: false;
  accepted: boolean;
  request: { path: string; sha256: string };
  receiptPath: string;
  source: { path: string; sha256: string; digest: string; fileCount: number } | null;
  buildReceipt: { path: string; sha256: string } | null;
  languages: LanguageResult[];
  steps: ManagedStep[];
  acceptance: { R01: boolean; R02: boolean };
  error: string | null;
}

export async function runManagedFullComparison(
  requestPath: string,
  requestSha256: string,
): Promise<RunReceipt> {
  const requestFile = readPinnedRequest(requestPath, requestSha256);
  const request = parseManagedFullCompareRequest(
    JSON.parse(requestFile.bytes.toString("utf8")) as unknown,
  );
  const pins = validatePinnedInputs(request);
  const expectedSource = parseSourceManifest(request.sourceManifest.path);
  const sourceBefore = captureNativeSourceManifest(request.repositoryRoot);
  if (!sameFileRecords(expectedSource.files, sourceBefore.files)) {
    throw new Error("Current managed source differs from the caller-pinned source manifest");
  }
  createRunRoot(request.outputRoot);
  const steps: ManagedStep[] = [];
  const languages: LanguageResult[] = [];
  const receipt = initialReceipt(
    requestPath,
    requestSha256,
    steps,
    languages,
    request,
    expectedSource.digest,
    expectedSource.files.length,
  );
  try {
    validateOfficialBaselines(request, pins);
    const build = buildNativeCli(request, expectedSource.digest, expectedSource.files, steps);
    const buildReceiptPath = build.receiptPath;
    receipt.buildReceipt = { path: buildReceiptPath, sha256: fileSha256(buildReceiptPath) };
    await runLanguageComparisons(
      async (language) =>
        runManagedLanguage(
          request,
          language,
          buildReceiptPath,
          build.binaryPath,
          request.outputRoot,
          steps,
          pins.fixtureSources[language],
        ),
      () =>
        verifyFinalState(
          request,
          pins,
          requestPath,
          requestSha256,
          requestFile.identity,
          expectedSource.digest,
          expectedSource.files,
          buildReceiptPath,
          build.binaryPath,
        ),
      (result) => languages.push(result),
    );
    verifyFinalState(
      request,
      pins,
      requestPath,
      requestSha256,
      requestFile.identity,
      expectedSource.digest,
      expectedSource.files,
      buildReceiptPath,
      build.binaryPath,
    );
    requireFullScoreAcceptance(languages);
    receipt.status = "completed";
    receipt.accepted = true;
    receipt.acceptance = { R01: true, R02: true };
  } catch (error) {
    receipt.status = "failed";
    receipt.error = error instanceof Error ? error.message : String(error);
  }
  writeExclusive(path.join(request.outputRoot, "receipt.json"), receipt);
  return receipt;
}

export async function runLanguageComparisons(
  runLanguage: (language: "en" | "ja") => Promise<LanguageAcceptance>,
  verifyAfterFailure: () => void,
  onResult?: (result: LanguageResult) => void,
): Promise<LanguageResult[]> {
  const results: LanguageResult[] = [];
  for (const language of ["en", "ja"] as const) {
    try {
      const result = await runLanguage(language);
      results.push(result);
      onResult?.(result);
    } catch (error) {
      const result: FullScoreFailure = {
        kind: "failure",
        language,
        error: error instanceof Error ? error.message : String(error),
        accepted: false,
      };
      results.push(result);
      onResult?.(result);
      verifyAfterFailure();
    }
  }
  return results;
}

function validateOfficialBaselines(request: ManagedFullCompareRequest, pins: PinnedInputs): void {
  for (const language of ["en", "ja"] as const) {
    const input = request.languages[language];
    const inventory = validateLanguageInventory(request, language, pins.fixtureSources[language]);
    RasterComparisonBinding.validateOfficialRole(
      request.repositoryRoot,
      {
        receiptPath: input.officialReceipt.path,
        receiptSha256: input.officialReceipt.sha256,
        processEvidencePath: input.officialProcessEvidence.path,
        processEvidenceSha256: input.officialProcessEvidence.sha256,
        pairDirectory: input.officialPairDirectory,
      },
      inventory.slugs,
    );
  }
}

function buildNativeCli(
  request: ManagedFullCompareRequest,
  sourceDigest: string,
  sourceFiles: ReturnType<typeof parseSourceManifest>["files"],
  steps: ManagedStep[],
): { receiptPath: string; binaryPath: string } {
  const argv = [
    request.tools.python.path,
    "scripts/mermaid/produce_native_cli_build_receipt.py",
    "--root",
    request.repositoryRoot,
    "--expected-source",
    request.sourceManifest.path,
    "--expected-source-sha256",
    request.sourceManifest.sha256,
    "--protected-registry",
    request.protectedRegistry.path,
    "--protected-registry-sha256",
    request.protectedRegistry.sha256,
    "--cargo",
    request.tools.cargo.path,
    "--rustc",
    request.tools.rustc.path,
    "--runtime",
    request.runtime.path,
  ];
  const result = runManagedCommand(
    request.outputRoot,
    "native-cli-build",
    argv,
    request.repositoryRoot,
    { kind: "python3", ...request.tools.python },
  );
  steps.push(stepRecord(result));
  requireCommandSuccess(result);
  const response = parseChildJson<BuildResponse>(result, "managed native build producer");
  const summary = verifyNativeBuildReceipt(
    response.receipt,
    path.join(path.dirname(response.receipt), "frozen/krr"),
    request.runtime.path,
    request.repositoryRoot,
  );
  assertCallerSourceBinding(summary, sourceDigest, sourceFiles);
  return { receiptPath: response.receipt, binaryPath: summary.binaryPath };
}

export function assertCallerSourceBinding(
  summary: Pick<
    ReturnType<typeof verifyNativeBuildReceipt>,
    "sourceManifestSha256" | "sourceDigest" | "sourceFiles"
  >,
  sourceDigest: string,
  sourceFiles: ReturnType<typeof parseSourceManifest>["files"],
): void {
  if (summary.sourceDigest !== sourceDigest || !sameFileRecords(summary.sourceFiles, sourceFiles)) {
    throw new Error("Managed native build receipt does not bind the caller-pinned source");
  }
}

function verifyFinalState(
  request: ManagedFullCompareRequest,
  pins: PinnedInputs,
  requestPath: string,
  requestSha256: string,
  requestIdentity: RasterFileIdentity,
  sourceDigest: string,
  sourceFiles: ReturnType<typeof parseSourceManifest>["files"],
  buildReceiptPath: string,
  binaryPath: string,
): void {
  validateFreshInputState(request, pins);
  if (
    fileSha256(requestPath) !== requestSha256 ||
    !sameRasterFileIdentity(requestIdentity, physicalFileIdentity(requestPath))
  )
    throw new Error("Run request changed during execution");
  const currentSource = captureNativeSourceManifest(request.repositoryRoot);
  const build = verifyNativeBuildReceipt(
    buildReceiptPath,
    binaryPath,
    request.runtime.path,
    request.repositoryRoot,
  );
  assertCallerSourceBinding(build, sourceDigest, sourceFiles);
  if (
    !sameFileRecords(sourceFiles, currentSource.files) ||
    build.sourceDigest !== currentSource.digest
  ) {
    throw new Error("Current source or managed build receipt changed during comparison");
  }
}

function readPinnedRequest(
  requestPath: string,
  expectedSha256: string,
): { bytes: Buffer; identity: RasterFileIdentity } {
  const canonical = path.resolve(requestPath);
  const stat = fs.lstatSync(requestPath);
  const bytes = fs.readFileSync(requestPath);
  if (
    !path.isAbsolute(requestPath) ||
    canonical !== requestPath ||
    stat.isSymbolicLink() ||
    !stat.isFile() ||
    fs.realpathSync(requestPath) !== requestPath ||
    !/^[a-f0-9]{64}$/.test(expectedSha256) ||
    sha256(bytes) !== expectedSha256 ||
    fileSha256(requestPath) !== expectedSha256
  ) {
    throw new Error("Run request must be an externally pinned canonical regular file");
  }
  const identity = physicalFileIdentity(requestPath);
  if (identity.sha256 !== expectedSha256) {
    throw new Error("Run request changed while it was being read");
  }
  return { bytes, identity };
}

function createRunRoot(outputRoot: string): void {
  fs.mkdirSync(outputRoot, { mode: 0o700 });
  fs.mkdirSync(path.join(outputRoot, "logs"), { mode: 0o700 });
}

function initialReceipt(
  requestPath: string,
  requestSha256: string,
  steps: ManagedStep[],
  languages: LanguageResult[],
  request: ManagedFullCompareRequest,
  sourceDigest: string,
  fileCount: number,
): RunReceipt {
  return {
    schema: "krr-mermaid-managed-full-run-v1",
    status: "failed",
    diagnosticOnly: false,
    accepted: false,
    request: { path: requestPath, sha256: requestSha256 },
    receiptPath: path.join(request.outputRoot, "receipt.json"),
    source: {
      path: request.sourceManifest.path,
      sha256: request.sourceManifest.sha256,
      digest: sourceDigest,
      fileCount,
    },
    buildReceipt: null,
    languages,
    steps,
    acceptance: { R01: false, R02: false },
    error: null,
  };
}

function stepRecord(result: ReturnType<typeof runManagedCommand>): ManagedStep {
  return {
    label: result.label,
    argv: result.argv,
    cwd: result.cwd,
    stdoutPath: result.stdoutPath,
    stderrPath: result.stderrPath,
    exitPath: result.exitPath,
    actualExitCode: result.exitCode,
    signal: result.signal,
    spawnError: result.spawnError,
  };
}

function parseCli(argv: string[]): { request: string; requestSha256: string } {
  const { values } = parseArgs({
    args: argv,
    options: { request: { type: "string" }, "request-sha256": { type: "string" } },
    strict: true,
    allowPositionals: false,
  });
  if (typeof values.request !== "string" || typeof values["request-sha256"] !== "string") {
    throw new Error("Usage: managed-full-compare.ts --request PATH --request-sha256 SHA");
  }
  return { request: values.request, requestSha256: values["request-sha256"] };
}

if (import.meta.main) {
  try {
    const args = parseCli(process.argv.slice(2));
    const receipt = await runManagedFullComparison(args.request, args.requestSha256);
    process.stdout.write(
      `${JSON.stringify({
        accepted: receipt.accepted,
        receipt: receipt.receiptPath,
      })}\n`,
    );
    if (!receipt.accepted) process.exitCode = 1;
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  }
}
