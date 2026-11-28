import path from "node:path";
import { z } from "zod";
import type { ManagedFullCompareRequest } from "./managed-full-compare-input";

const sha256 = z.string().regex(/^[a-f0-9]{64}$/);
const buildFacts = z
  .object({
    binarySha256: sha256,
    sourceHead: z.string().regex(/^[a-f0-9]{40}$/),
    sourceSnapshotSha256: sha256,
    lockSha256: sha256,
    buildArtifactSha256: sha256,
    cargoExecutableSha256: sha256,
  })
  .strict();

type Capture = Pick<
  NonNullable<ManagedFullCompareRequest["languages"]["en"]["nativeFrameCapture"]>,
  "probeBinary" | "buildProof"
>;

export function nativeFrameBuildProofArgv(
  pythonPath: string,
  root: string,
  capture: Capture,
): string[] {
  return [
    pythonPath,
    "scripts/release/verify_html_performance_evidence.py",
    "--evidence-dir",
    path.dirname(capture.buildProof.bundleManifest.path),
    "--bundle-manifest-sha256",
    capture.buildProof.bundleManifest.sha256,
    "--repo",
    root,
    "--candidate-build-proof",
    capture.buildProof.artifactPath,
    "--build-artifact-sha256",
    capture.buildProof.artifactSha256,
  ];
}

export function requireNativeFrameBuildFacts(
  value: unknown,
  capture: Capture,
  cargoSha256: string,
): void {
  const facts = buildFacts.parse(value);
  if (
    facts.binarySha256 !== capture.probeBinary.sha256 ||
    facts.buildArtifactSha256 !== capture.buildProof.artifactSha256 ||
    facts.cargoExecutableSha256 !== cargoSha256
  ) {
    throw new Error("Native frame build facts differ from the managed binary/artifact/tool pins");
  }
}
