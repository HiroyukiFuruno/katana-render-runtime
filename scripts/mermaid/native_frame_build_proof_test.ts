import { describe, expect, test } from "bun:test";
import {
  nativeFrameBuildProofArgv,
  requireNativeFrameBuildFacts,
} from "./native-frame-build-proof";

const capture = {
  probeBinary: { path: "/proof/candidate", sha256: "a".repeat(64) },
  buildProof: {
    bundleManifest: { path: "/proof/bundle.json", sha256: "b".repeat(64) },
    artifactPath: "builds/candidate/build-artifact.json",
    artifactSha256: "c".repeat(64),
  },
};
const facts = {
  binarySha256: capture.probeBinary.sha256,
  sourceHead: "d".repeat(40),
  sourceSnapshotSha256: "e".repeat(64),
  lockSha256: "f".repeat(64),
  buildArtifactSha256: capture.buildProof.artifactSha256,
  cargoExecutableSha256: "0".repeat(64),
};

describe("managed native frame build proof", () => {
  test("passes only the build-only verifier route and pinned artifact arguments", () => {
    expect(nativeFrameBuildProofArgv("/tools/python3", "/repo", capture)).toEqual([
      "/tools/python3",
      "scripts/release/verify_html_performance_evidence.py",
      "--evidence-dir",
      "/proof",
      "--bundle-manifest-sha256",
      capture.buildProof.bundleManifest.sha256,
      "--repo",
      "/repo",
      "--candidate-build-proof",
      "builds/candidate/build-artifact.json",
      "--build-artifact-sha256",
      capture.buildProof.artifactSha256,
    ]);
  });

  test("accepts matching facts without interpreting them as rendering acceptance", () => {
    expect(() =>
      requireNativeFrameBuildFacts(facts, capture, facts.cargoExecutableSha256),
    ).not.toThrow();
  });

  test.each(["binarySha256", "buildArtifactSha256", "cargoExecutableSha256"] as const)(
    "rejects a different %s",
    (key) => {
      expect(() =>
        requireNativeFrameBuildFacts(
          { ...facts, [key]: "1".repeat(64) },
          capture,
          facts.cargoExecutableSha256,
        ),
      ).toThrow("managed binary/artifact/tool pins");
    },
  );

  test.each([
    { ...facts, accepted: true },
    { ...facts, sourceHead: "not-a-commit" },
    { ...facts, lockSha256: "short" },
    { ...facts, sourceSnapshotSha256: null },
  ])("rejects non-build or malformed facts %#", (changed) => {
    expect(() =>
      requireNativeFrameBuildFacts(changed, capture, facts.cargoExecutableSha256),
    ).toThrow();
  });
});
