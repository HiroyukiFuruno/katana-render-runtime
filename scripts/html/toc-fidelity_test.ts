import { expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  assertCatalogBinaryProof,
  freezeCatalogBinary,
  resumeCatalogBinary,
  validateCatalogBinaryClosure,
} from "./catalog-capture-binary";
import type { JsonValue } from "./dynamic-state";
import { sha256File } from "./reference-capture";
import {
  matchTocEntries,
  tocBinaryFromBuildReport,
  tocBinaryPathFromBuildReport,
  validateTocEntries,
} from "./toc-fidelity";

const entry = {
  destination: "section",
  hash: "#section",
  scrollY: 120,
  capturePath: "/tmp/chrome-section.png",
  targetVisible: true,
};

test("requires every visible TOC destination with its navigation hash", () => {
  expect(validateTocEntries([entry])).toEqual([entry]);
  expect(() => validateTocEntries([entry, entry])).toThrow("Duplicate");
  expect(() => validateTocEntries([{ ...entry, targetVisible: false }])).toThrow("invisible");
  expect(() => validateTocEntries([{ ...entry, hash: "#other" }])).toThrow("Invalid");
  expect(() => validateTocEntries([{ ...entry, scrollY: Number.NaN }])).toThrow("Invalid");
});

test("capture binary proof survives source removal and rejects altered bindings", () => {
  const directory = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "krr-capture-proof-")));
  const source = path.join(directory, "source");
  const binary = path.join(directory, "frozen");
  const report = path.join(directory, "build.jsonl");
  const proofPath = path.join(directory, "proof.json");
  fs.writeFileSync(source, "binary");
  fs.writeFileSync(binary, "binary");
  fs.writeFileSync(
    report,
    JSON.stringify({
      reason: "compiler-artifact",
      executable: source,
      target: { name: "katana_render_runtime", kind: ["lib"] },
      profile: { test: true },
    }),
  );
  const sha = sha256File(binary);
  const proof = {
    schema: "krr-html84-capture-binary-v1",
    buildReport: report,
    buildReportSha256: sha256File(report),
    source,
    path: binary,
    sha256: sha,
  };
  fs.writeFileSync(proofPath, JSON.stringify(proof));
  const ref = { path: proofPath, sha256: sha256File(proofPath) };
  try {
    fs.unlinkSync(source);
    expect(() => tocBinaryFromBuildReport(report)).toThrow();
    expect(() => assertCatalogBinaryProof(ref, binary, sha)).not.toThrow();
    expect(() => assertCatalogBinaryProof(ref, `${binary}-other`, sha)).toThrow();
    fs.writeFileSync(proofPath, JSON.stringify({ ...proof, source: `${source}-other` }));
    expect(() =>
      assertCatalogBinaryProof({ ...ref, sha256: sha256File(proofPath) }, binary, sha),
    ).toThrow();
    fs.writeFileSync(proofPath, JSON.stringify({ ...proof, path: `${binary}-other` }));
    expect(() =>
      assertCatalogBinaryProof({ ...ref, sha256: sha256File(proofPath) }, binary, sha),
    ).toThrow();
    fs.writeFileSync(proofPath, JSON.stringify(proof));
    fs.writeFileSync(binary, "changed");
    expect(() => assertCatalogBinaryProof(ref, binary, sha)).toThrow();
    fs.writeFileSync(binary, "binary");
    fs.appendFileSync(report, " ");
    expect(() => assertCatalogBinaryProof(ref, binary, sha)).toThrow();
    fs.appendFileSync(proofPath, " ");
    expect(() => assertCatalogBinaryProof(ref, binary, sha)).toThrow();
  } finally {
    fs.unlinkSync(binary);
    fs.unlinkSync(report);
    fs.unlinkSync(proofPath);
    fs.rmdirSync(directory);
  }
});

test("catalog freeze binds the selected build executable and refuses a reused destination", () => {
  const directory = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "krr-capture-freeze-")));
  const source = path.join(directory, "source");
  const report = path.join(directory, "build.jsonl");
  const output = path.join(directory, "capture");
  fs.writeFileSync(source, "binary");
  fs.writeFileSync(
    report,
    JSON.stringify({
      reason: "compiler-artifact",
      executable: source,
      target: { name: "katana_render_runtime", kind: ["lib"] },
      profile: { test: true },
    }),
  );
  try {
    const ref = freezeCatalogBinary(report, output);
    const binary = path.join(`${output}-binary`, "runtime-tests");
    expect(() => assertCatalogBinaryProof(ref, binary, sha256File(source))).not.toThrow();
    expect(() => freezeCatalogBinary(report, output)).toThrow();
  } finally {
    fs.unlinkSync(path.join(`${output}-binary`, "proof.json"));
    fs.unlinkSync(path.join(`${output}-binary`, "runtime-tests"));
    fs.rmdirSync(`${output}-binary`);
    fs.unlinkSync(source);
    fs.unlinkSync(report);
    fs.rmdirSync(directory);
  }
});

test("catalog resume reuses prior frozen path and rejects a changed current build", () => {
  const directory = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "krr-capture-resume-")));
  const source = path.join(directory, "source");
  const report = path.join(directory, "build.jsonl");
  const observation = path.join(directory, "observation.json");
  const finalized = path.join(directory, "finalized.json");
  fs.writeFileSync(source, "binary");
  fs.writeFileSync(
    report,
    JSON.stringify({
      reason: "compiler-artifact",
      executable: source,
      target: { name: "katana_render_runtime", kind: ["lib"] },
      profile: { test: true },
    }),
  );
  try {
    const proof = freezeCatalogBinary(report, path.join(directory, "capture"));
    const binary = path.join(directory, "capture-binary", "runtime-tests");
    const sha = sha256File(binary);
    const manifest = {
      schema: "krr-html84-catalog-captures-v2",
      binaryPath: binary,
      binarySha256: sha,
      binaryProof: proof,
      currentBuild: { path: report, sha256: sha256File(report), source, binarySha256: sha },
    };
    fs.writeFileSync(observation, JSON.stringify(manifest));
    fs.writeFileSync(finalized, JSON.stringify(manifest));
    expect(resumeCatalogBinary(report, observation, finalized).binary).toBe(binary);
    expect(resumeCatalogBinary(report, observation, finalized).proof).toEqual(proof);
    const otherProof = path.join(directory, "other-proof.json");
    fs.copyFileSync(proof.path, otherProof);
    fs.writeFileSync(
      finalized,
      JSON.stringify({ ...manifest, binaryProof: { ...proof, path: otherProof } }),
    );
    expect(() => resumeCatalogBinary(report, observation, finalized)).toThrow();
    fs.writeFileSync(finalized, JSON.stringify(manifest));
    fs.writeFileSync(source, "different");
    expect(() => resumeCatalogBinary(report, observation, finalized)).toThrow();
    fs.writeFileSync(source, "binary");
    fs.writeFileSync(finalized, JSON.stringify({ ...manifest, schema: "old" }));
    expect(() => resumeCatalogBinary(report, observation, finalized)).toThrow();
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test("binary closure validates transported files without resolving logical paths on this host", () => {
  const directory = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "krr-closure-")));
  const logical = {
    manifest: "C:\\proof\\manifest.json",
    proof: "C:\\proof\\proof.json",
    report: "C:\\proof\\build.jsonl",
    binary: "C:\\proof\\binary",
    source: "C:\\target\\runtime",
  };
  const payload = Object.fromEntries(
    ["manifest", "proof", "report", "binary"].map((name) => [name, path.join(directory, name)]),
  );
  const file = (name: string): string => {
    const value = payload[name];
    if (!value) throw new Error("missing fixture");
    return value;
  };
  fs.writeFileSync(file("binary"), "binary");
  const sha = sha256File(file("binary"));
  fs.writeFileSync(
    file("report"),
    JSON.stringify({
      reason: "compiler-artifact",
      executable: logical.source,
      target: { name: "katana_render_runtime", kind: ["lib"] },
      profile: { test: true },
    }),
  );
  const reportSha = sha256File(file("report"));
  fs.writeFileSync(
    file("proof"),
    JSON.stringify({
      schema: "krr-html84-capture-binary-v1",
      buildReport: logical.report,
      buildReportSha256: reportSha,
      source: logical.source,
      path: logical.binary,
      sha256: sha,
    }),
  );
  fs.writeFileSync(
    file("manifest"),
    JSON.stringify({
      schema: "krr-html84-catalog-captures-v2",
      binaryPath: logical.binary,
      binarySha256: sha,
      binaryProof: { path: logical.proof, sha256: sha256File(file("proof")) },
      currentBuild: {
        path: logical.report,
        sha256: reportSha,
        source: logical.source,
        binarySha256: sha,
      },
    }),
  );
  const files = (["manifest", "proof", "report", "binary"] as const).map((name) => ({
    path: logical[name],
    sha256: sha256File(file(name)),
    payloadPath: file(name),
  }));
  const request = path.join(directory, "request");
  const write = (entries: typeof files) =>
    fs.writeFileSync(
      request,
      JSON.stringify({
        schema: "krr-html84-binary-closure-request-v1",
        manifest: { path: logical.manifest, sha256: files[0]?.sha256 },
        files: entries,
      }),
    );
  try {
    write(files);
    expect(validateCatalogBinaryClosure(request).requestSha256).toBe(sha256File(request));
    write([
      ...files.slice(0, 3),
      { path: "C:\\proof\\other", sha256: sha, payloadPath: file("binary") },
    ]);
    expect(() => validateCatalogBinaryClosure(request)).toThrow();
    write([...files.slice(0, 3), files[0] as (typeof files)[number]]);
    expect(() => validateCatalogBinaryClosure(request)).toThrow();
    write(files);
    fs.writeFileSync(file("binary"), "changed");
    expect(() => validateCatalogBinaryClosure(request)).toThrow();
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test("rejects missing reordered or substituted TOC coverage", () => {
  const second = { ...entry, destination: "child", hash: "#child" };
  expect(() => matchTocEntries([entry, second], [entry, second])).not.toThrow();
  expect(() => matchTocEntries([entry, second], [entry])).toThrow("coverage differs");
  expect(() => matchTocEntries([entry, second], [second, entry])).toThrow("coverage differs");
  expect(() => matchTocEntries([entry], [second])).toThrow("coverage differs");
});

test("selects exactly one runtime library test executable from Cargo output", () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "krr-toc-build-"));
  const report = path.join(directory, "build.jsonl");
  const executable = path.join(directory, "runtime-test");
  const otherExecutable = path.join(directory, "other-runtime-test");
  fs.writeFileSync(executable, "current artifact");
  fs.writeFileSync(otherExecutable, "second artifact");
  const artifact = {
    reason: "compiler-artifact",
    executable,
    target: { name: "katana_render_runtime", kind: ["lib"] },
    profile: { test: true },
  };
  const write = (items: JsonValue[]) =>
    fs.writeFileSync(report, items.map((item) => JSON.stringify(item)).join("\n"));
  try {
    write([
      { ...artifact, executable: path.join(directory, "removed-runtime-test") },
      { ...artifact, target: { name: "dependency", kind: ["lib"] } },
      artifact,
    ]);
    expect(tocBinaryFromBuildReport(report)).toBe(executable);
    expect(() => tocBinaryPathFromBuildReport(report)).toThrow();
    write([artifact]);
    expect(tocBinaryPathFromBuildReport(report)).toBe(executable);
    write([{ ...artifact, profile: { test: false } }]);
    expect(() => tocBinaryFromBuildReport(report)).toThrow();
    write([artifact, { ...artifact, executable: otherExecutable }]);
    expect(() => tocBinaryFromBuildReport(report)).toThrow();
  } finally {
    fs.unlinkSync(report);
    fs.unlinkSync(executable);
    fs.unlinkSync(otherExecutable);
    fs.rmdirSync(directory);
  }
});
