import fs from "node:fs";
import path from "node:path";
import { sha256File } from "./reference-capture";
import { tocBinaryFromBuildReport, tocBinaryPathFromBuildReport } from "./toc-fidelity";

export interface CatalogBinaryProofRef {
  path: string;
  sha256: string;
}

export interface CatalogCurrentBuild {
  path: string;
  sha256: string;
  source: string;
  binarySha256: string;
}

interface ClosureFile {
  path: string;
  sha256: string;
  payloadPath: string;
}

function closureRecord(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value))
    throw new Error("catalog closure record is invalid");
  return value as Record<string, unknown>;
}

function closureFiles(value: unknown): Map<string, ClosureFile> {
  if (!Array.isArray(value) || value.length < 4 || value.length > 5)
    throw new Error("catalog closure file count differs");
  const files = new Map<string, ClosureFile>();
  for (const item of value) {
    const row = closureRecord(item);
    if (
      Object.keys(row).sort().join(",") !== "path,payloadPath,sha256" ||
      typeof row.path !== "string" ||
      !(path.posix.isAbsolute(row.path) || path.win32.isAbsolute(row.path)) ||
      typeof row.sha256 !== "string" ||
      !/^[a-f0-9]{64}$/u.test(row.sha256) ||
      typeof row.payloadPath !== "string" ||
      !path.isAbsolute(row.payloadPath) ||
      files.has(row.path)
    )
      throw new Error("catalog closure file pin is invalid");
    files.set(row.path, { path: row.path, sha256: row.sha256, payloadPath: row.payloadPath });
  }
  return files;
}

export function validateCatalogBinaryClosure(requestPath: string): Record<string, unknown> {
  if (!path.isAbsolute(requestPath) || fs.realpathSync(requestPath) !== requestPath)
    throw new Error("catalog closure request must be canonical");
  const requestSha256 = sha256File(requestPath);
  const request = closureRecord(JSON.parse(fs.readFileSync(requestPath, "utf8")));
  if (
    Object.keys(request).sort().join(",") !== "files,manifest,schema" ||
    request.schema !== "krr-html84-binary-closure-request-v1"
  )
    throw new Error("catalog closure request differs");
  const manifest = closureRecord(request.manifest);
  if (
    Object.keys(manifest).sort().join(",") !== "path,sha256" ||
    typeof manifest.path !== "string" ||
    typeof manifest.sha256 !== "string"
  )
    throw new Error("catalog closure manifest pin differs");
  const files = closureFiles(request.files);
  const used = new Set<string>();
  const resolve: BinaryFileResolver = (logical, sha) => {
    const file = files.get(logical);
    if (!file || file.sha256 !== sha) throw new Error("catalog closure file missing or differs");
    used.add(logical);
    return liveBinaryFile(file.payloadPath, sha);
  };
  const captured = closureRecord(
    JSON.parse(fs.readFileSync(resolve(manifest.path, manifest.sha256), "utf8")),
  );
  assertCatalogManifestBinaryProof(captured, resolve);
  if (used.size !== files.size || sha256File(requestPath) !== requestSha256)
    throw new Error("catalog closure contains unused files or changed request");
  return { schema: "krr-html84-binary-closure-result-v1", requestSha256, manifest };
}

export const CATALOG_CAPTURE_SCHEMA = "krr-html84-catalog-captures-v2";

type BinaryFileResolver = (logicalPath: string, sha256: string) => string;

function liveBinaryFile(logicalPath: string, sha256: string): string {
  if (fs.realpathSync(logicalPath) !== logicalPath || sha256File(logicalPath) !== sha256)
    throw new Error("catalog binary file differs");
  return logicalPath;
}

export function assertCatalogManifestBinaryProof(
  manifest: Record<string, unknown>,
  resolve: BinaryFileResolver = liveBinaryFile,
): void {
  const ref = manifest.binaryProof;
  if (
    manifest.schema !== CATALOG_CAPTURE_SCHEMA ||
    typeof manifest.binaryPath !== "string" ||
    typeof manifest.binarySha256 !== "string" ||
    !/^[a-f0-9]{64}$/u.test(manifest.binarySha256) ||
    typeof ref !== "object" ||
    ref === null ||
    Array.isArray(ref)
  )
    throw new Error("catalog capture v2 binary proof is required");
  const value = ref as Record<string, unknown>;
  if (
    Object.keys(value).sort().join(",") !== "path,sha256" ||
    typeof value.path !== "string" ||
    typeof value.sha256 !== "string" ||
    !/^[a-f0-9]{64}$/u.test(value.sha256)
  )
    throw new Error("catalog capture binary proof reference differs");
  assertCatalogBinaryProof(
    { path: value.path, sha256: value.sha256 },
    manifest.binaryPath,
    manifest.binarySha256,
    resolve,
  );
  assertCatalogCurrentBuild(manifest.currentBuild, manifest.binarySha256, resolve);
}

function assertCatalogCurrentBuild(
  value: unknown,
  binarySha256: string,
  resolve: BinaryFileResolver,
): void {
  if (typeof value !== "object" || value === null || Array.isArray(value))
    throw new Error("catalog current build is invalid");
  const build = value as Record<string, unknown>;
  if (
    Object.keys(build).sort().join(",") !== "binarySha256,path,sha256,source" ||
    typeof build.path !== "string" ||
    typeof build.sha256 !== "string" ||
    typeof build.source !== "string" ||
    build.binarySha256 !== binarySha256 ||
    tocBinaryPathFromBuildReport(resolve(build.path, build.sha256)) !== build.source
  )
    throw new Error("catalog current build binding differs");
}

export function currentCatalogBuild(buildReport: string): CatalogCurrentBuild {
  const report = fs.realpathSync(buildReport);
  const reportSha = sha256File(report);
  const source = tocBinaryFromBuildReport(report);
  if (fs.realpathSync(source) !== source || tocBinaryPathFromBuildReport(report) !== source)
    throw new Error("catalog current build selection differs");
  const binarySha256 = sha256File(source);
  if (sha256File(report) !== reportSha) throw new Error("catalog current build report changed");
  return { path: report, sha256: reportSha, source, binarySha256 };
}

export function resumeCatalogBinary(
  buildReport: string,
  observationPath: string,
  finalizedPath: string,
): {
  binary: string;
  proof: CatalogBinaryProofRef;
  currentBuild: CatalogCurrentBuild;
} {
  const observation = JSON.parse(
    fs.readFileSync(fs.realpathSync(observationPath), "utf8"),
  ) as Record<string, unknown>;
  const finalized = JSON.parse(fs.readFileSync(fs.realpathSync(finalizedPath), "utf8")) as Record<
    string,
    unknown
  >;
  assertCatalogManifestBinaryProof(observation);
  assertCatalogManifestBinaryProof(finalized);
  const observationProof = observation.binaryProof as CatalogBinaryProofRef;
  const finalizedProof = finalized.binaryProof as CatalogBinaryProofRef;
  if (
    observation.binaryPath !== finalized.binaryPath ||
    observation.binarySha256 !== finalized.binarySha256 ||
    observationProof.path !== finalizedProof.path ||
    observationProof.sha256 !== finalizedProof.sha256
  )
    throw new Error("catalog resume binary identity differs");
  const currentBuild = currentCatalogBuild(buildReport);
  if (currentBuild.binarySha256 !== finalized.binarySha256)
    throw new Error("catalog resume current build differs");
  return { binary: finalized.binaryPath as string, proof: finalizedProof, currentBuild };
}

export function freezeCatalogBinary(buildReport: string, output: string): CatalogBinaryProofRef {
  const report = fs.realpathSync(buildReport);
  const reportSha256 = sha256File(report);
  const source = tocBinaryFromBuildReport(report);
  if (tocBinaryPathFromBuildReport(report) !== source)
    throw new Error("catalog build selection differs");
  if (fs.realpathSync(source) !== source)
    throw new Error("catalog build executable is not canonical");
  const sourceSha256 = sha256File(source);
  const frozen = path.resolve(`${output}-binary`, "runtime-tests");
  const result = Bun.spawnSync([
    "python3",
    path.join(import.meta.dir, "../release/freeze_html_capture_binary.py"),
    "--source",
    source,
    "--destination",
    frozen,
  ]);
  if (result.exitCode !== 0)
    throw new Error(`binary freeze failed: ${new TextDecoder().decode(result.stderr)}`);
  const value = JSON.parse(new TextDecoder().decode(result.stdout)) as Record<string, unknown>;
  if (
    value.source !== source ||
    value.path !== frozen ||
    value.sha256 !== sourceSha256 ||
    fs.realpathSync(frozen) !== frozen ||
    sha256File(frozen) !== sourceSha256 ||
    sha256File(source) !== sourceSha256 ||
    sha256File(report) !== reportSha256
  )
    throw new Error("catalog binary freeze identity differs");
  const proof = {
    schema: "krr-html84-capture-binary-v1",
    buildReport: report,
    buildReportSha256: reportSha256,
    source,
    path: frozen,
    sha256: sourceSha256,
  };
  const proofPath = path.join(path.dirname(frozen), "proof.json");
  fs.writeFileSync(proofPath, `${JSON.stringify(proof)}\n`, { flag: "wx" });
  return { path: proofPath, sha256: sha256File(proofPath) };
}

export function assertCatalogBinaryProof(
  ref: CatalogBinaryProofRef,
  binary: string,
  sha: string,
  resolve: BinaryFileResolver = liveBinaryFile,
): void {
  const proof = JSON.parse(fs.readFileSync(resolve(ref.path, ref.sha256), "utf8")) as Record<
    string,
    unknown
  >;
  if (
    Object.keys(proof).sort().join(",") !==
      "buildReport,buildReportSha256,path,schema,sha256,source" ||
    proof.schema !== "krr-html84-capture-binary-v1" ||
    typeof proof.buildReport !== "string" ||
    typeof proof.buildReportSha256 !== "string" ||
    typeof proof.source !== "string" ||
    proof.path !== binary ||
    proof.sha256 !== sha ||
    tocBinaryPathFromBuildReport(resolve(proof.buildReport, proof.buildReportSha256)) !==
      proof.source
  )
    throw new Error("catalog binary build binding differs");
  resolve(binary, sha);
}
