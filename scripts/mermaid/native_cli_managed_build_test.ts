import { describe, expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { verifyRustFactsHost } from "../runtime-bundles/runtime-rust-facts-host";
import { fileSha256 } from "./mermaid-pair-manifest";
import { verifyNativeBuildReceipt } from "./native-build-receipt";
import { writeFile } from "./native-classic-capture-test-fixture";
import { managedRequestSchema } from "./native-cli-managed-build-schema";
import { createManagedReaderFixture } from "./native-cli-managed-build-test-fixture";
import { validateManagedCliArtifact } from "./native-cli-managed-cargo";
import { managedCapacitySchema, managedFileSchema } from "./native-cli-managed-input-schema";
import { owned, verifyManagedArtifactRolePins } from "./native-cli-managed-process";

const pin = { path: "/unit/file", sha256: "a".repeat(64), size: 1 };
const capacity = {
  state: pin,
  protectedRegistry: pin,
  refs: [{ ...pin, dev: 1, ino: 2, mode: 33188 }],
  capacityPolicy: {
    metric: "unique-inode-allocated-bytes",
    metric_version: 1,
    baseline_bytes: 2363047936,
    factor: 2,
    threshold_bytes: 4726095872,
    target: "/unit/target",
  },
  environmentValue: '["/unit/file"]',
  environmentSha256: "b".repeat(64),
};
const request = {
  schema: "krr-native-cli-guard-request-v1",
  attemptToken: "a".repeat(64),
  cwd: "/unit",
  target: "/unit/target",
  binaryPath: "/unit/target/debug/krr",
  issuedMonotonicNs: "1",
  expiresMonotonicNs: "120000000001",
  cargo: pin,
  sourceManifest: pin,
  sourceDigest: "a".repeat(64),
  metadata: pin,
  metadataObservation: pin,
  expectedPackage: {
    id: "unit#cli",
    name: "katana-render-runtime-cli",
    manifestPath: "/unit/crates/katana-render-runtime-cli/Cargo.toml",
  },
  expectedTarget: {
    name: "krr",
    kind: ["bin"],
    srcPath: "/unit/crates/katana-render-runtime-cli/src/main.rs",
  },
  environment: { PATH: "/unit", RUSTFLAGS: "-D warnings" },
  environmentSha256: "a".repeat(64),
  guardCodePins: [pin],
  capacityAdmission: capacity,
};
const artifact = {
  executable: "/unit/target/debug/krr",
  fresh: true,
  filenames: ["/unit/target/debug/krr"],
  features: [],
  profile: {
    opt_level: "0",
    debuginfo: 2,
    debug_assertions: true,
    overflow_checks: true,
    test: false,
  },
  target: {
    name: "krr",
    kind: ["bin"],
    src_path: "/unit/crates/katana-render-runtime-cli/src/main.rs",
  },
};

describe("managed CLI strict unit boundaries; no actual build evidence", () => {
  test("request keeps exact eighteen fields and nested boundaries", () => {
    expect(Object.keys(managedRequestSchema.parse(request))).toHaveLength(18);
    for (const value of [
      { ...request, extra: true },
      { ...request, cargo: { ...pin, extra: true } },
      { ...request, expectedTarget: { ...request.expectedTarget, kind: ["lib"] } },
      { ...request, capacityAdmission: { ...capacity, extra: true } },
    ]) {
      expect(() => managedRequestSchema.parse(value)).toThrow();
    }
  });
  test("required request bindings cannot be omitted", () => {
    for (const key of [
      "sourceManifest",
      "guardCodePins",
      "metadataObservation",
      "capacityAdmission",
    ] as const) {
      const value: { [key: string]: unknown } = { ...request };
      delete value[key];
      expect(() => managedRequestSchema.parse(value)).toThrow();
    }
  });
  test("fixed capacity and file shape reject threshold weakening and bool sizes", () => {
    expect(() =>
      managedCapacitySchema.parse({
        ...capacity,
        capacityPolicy: { ...capacity.capacityPolicy, threshold_bytes: 4726095873 },
      }),
    ).toThrow();
    expect(() => managedFileSchema.parse({ ...pin, size: true })).toThrow();
  });
  test("fresh and rebuilt artifacts keep boolean distinction", () => {
    expect(validateManagedCliArtifact(artifact).fresh).toBe(true);
    expect(validateManagedCliArtifact({ ...artifact, fresh: false }).fresh).toBe(false);
  });
  test("missing, boolean and unauthorized selected features reject", () => {
    for (const features of [undefined, true, ["unauthorized"], ["x", "x"]]) {
      expect(() => validateManagedCliArtifact({ ...artifact, features })).toThrow();
    }
  });
  test("profile type, domain, test mode and extra fields reject", () => {
    for (const profile of [
      undefined,
      true,
      { ...artifact.profile, test: true },
      { ...artifact.profile, debuginfo: true },
      { ...artifact.profile, debuginfo: 3 },
      { ...artifact.profile, opt_level: "9" },
      { ...artifact.profile, extra: true },
    ]) {
      expect(() => validateManagedCliArtifact({ ...artifact, profile })).toThrow();
    }
  });
  test("external Cargo supported debug domains retain strict managed configuration", () => {
    for (const debuginfo of [null, 0, 1, 2, "line-directives-only", "line-tables-only"] as const) {
      expect(
        validateManagedCliArtifact({ ...artifact, profile: { ...artifact.profile, debuginfo } })
          .profile.debuginfo,
      ).toBe(debuginfo);
    }
  });
  test("owned raw lifecycle hashes and PID bindings reject physical mutation", () => {
    const fixture = createManagedReaderFixture();
    try {
      const observed = fixture.receipt.buildObservation;
      owned(observed, fixture.receipt.buildLifecycle, "build");
      expect(() =>
        owned({ ...observed, pid: 124 }, fixture.receipt.buildLifecycle, "build"),
      ).toThrow(/lifecycle binding/);
      fs.appendFileSync(observed.stdout.path, "tampered");
      expect(() => owned(observed, fixture.receipt.buildLifecycle, "build")).toThrow();
    } finally {
      fs.rmSync(fixture.root, { recursive: true, force: true });
    }
  });
  test("owned raw FD closure rejects leaf inode replacement with unchanged bytes SHA size and mode", () => {
    const fixture = createManagedReaderFixture();
    try {
      const observed = fixture.receipt.buildObservation;
      owned(observed, fixture.receipt.buildLifecycle, "build");
      const raw = observed.stdout.path,
        before = fs.statSync(raw, { bigint: true }),
        bytes = fs.readFileSync(raw),
        sha256 = fileSha256(raw),
        replacement = `${raw}.replacement`;
      fs.writeFileSync(replacement, bytes, { flag: "wx", mode: Number(before.mode) });
      fs.chmodSync(replacement, Number(before.mode));
      fs.renameSync(replacement, raw);
      const after = fs.statSync(raw, { bigint: true });
      expect(fs.readFileSync(raw)).toEqual(bytes);
      expect(fileSha256(raw)).toBe(sha256);
      expect([after.size, after.mode, after.dev]).toEqual([before.size, before.mode, before.dev]);
      expect(after.ino).not.toBe(before.ino);
      expect(() => owned(observed, fixture.receipt.buildLifecycle, "build")).toThrow(
        /raw inode differs/,
      );
    } finally {
      fs.rmSync(fixture.root, { recursive: true, force: true });
    }
  });
  test("owned artifact roles reject distinct paths that alias one inode", () => {
    const directory = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "krr-role-unit-")));
    const first = path.join(directory, "first"),
      second = path.join(directory, "second");
    fs.writeFileSync(first, "same bytes");
    fs.writeFileSync(second, "same bytes");
    const record = (name: string) => ({
      path: name,
      sha256: fileSha256(name),
      size: fs.statSync(name).size,
    });
    verifyManagedArtifactRolePins([record(first), record(second)], directory);
    const alias = path.join(directory, "hard-alias");
    fs.linkSync(first, alias);
    expect(() => verifyManagedArtifactRolePins([record(first), record(alias)], directory)).toThrow(
      /aliases inode/,
    );
    const symbolic = path.join(directory, "symlink");
    fs.symlinkSync(first, symbolic);
    expect(() => verifyManagedArtifactRolePins([record(symbolic)], directory)).toThrow();
    expect(() =>
      verifyManagedArtifactRolePins([record(first)], path.join(directory, "other")),
    ).toThrow(/outside owned attempt/);
  });

  test("host verification uses the admitted child environment", () => {
    const fixture = createManagedReaderFixture();
    try {
      expect(() =>
        verifyRustFactsHost(fixture.root, fixture.hostReceiptPath, fixture.request.environment),
      ).not.toThrow();
      const invalidEnvironment = { ...fixture.request.environment, PATH: "/missing" };
      expect(() =>
        verifyRustFactsHost(fixture.root, fixture.hostReceiptPath, invalidEnvironment),
      ).toThrow(/missing Rust facts host tool|prepared inputs are stale/);
    } finally {
      fs.rmSync(fixture.root, { recursive: true, force: true });
    }
  });
});

test("full reader rejects writable raw holder despite empty declared holders and valid raw pins", () => {
  const f = createManagedReaderFixture();
  try {
    verifyNativeBuildReceipt(f.receiptPath, f.binaryPath, f.runtimePath, f.root);
    const life = JSON.parse(fs.readFileSync(f.receipt.buildLifecycle.path, "utf8"));
    const proof = life.nativeRawFdClosure,
      identity = proof.rawIdentities[0];
    proof.stdout = writeFile(
      proof.stdout.path,
      `p999999\ng999999\nf1\naw\ntREG\nD0x${identity.device.toString(16)}\ni${identity.inode}\n`,
    );
    proof.actualExit = 0;
    f.receipt.buildLifecycle = writeFile(f.receipt.buildLifecycle.path, JSON.stringify(life));
    expect(() => {
      writeFile(f.receiptPath, JSON.stringify(f.receipt));
      return verifyNativeBuildReceipt(f.receiptPath, f.binaryPath, f.runtimePath, f.root);
    }).toThrow(/unknown writable holder/);
  } finally {
    fs.rmSync(f.root, { recursive: true, force: true });
  }
});
