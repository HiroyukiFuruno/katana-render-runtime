import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import fs from "node:fs";
import path from "node:path";
import { fileSha256 } from "./mermaid-pair-manifest";
import { verifyNativeBuildReceipt } from "./native-build-receipt";
import { createManagedReaderFixture } from "./native-cli-managed-build-test-fixture";

type Fixture = ReturnType<typeof createManagedReaderFixture>;
type OriginState = "available" | "changed" | "missing";

function frozenIdentity(f: Fixture, name: string) {
  const directory = path.join(path.dirname(f.receiptPath), "frozen");
  fs.mkdirSync(directory, { recursive: true });
  const binary = path.join(directory, name);
  fs.copyFileSync(f.binaryPath, binary, fs.constants.COPYFILE_EXCL);
  fs.chmodSync(binary, 0o555);
  const stat = fs.statSync(binary, { bigint: true });
  return {
    path: binary,
    sha256: fileSha256(binary),
    size: Number(stat.size),
    device: Number(stat.dev),
    inode: Number(stat.ino),
    mtimeNs: stat.mtimeNs.toString(),
    ctimeNs: stat.ctimeNs.toString(),
  };
}

function withOrigin(f: Fixture, state: OriginState, check: () => void) {
  if (state === "available") {
    check();
    return;
  }
  const original = `${f.binaryPath}.retained`;
  fs.renameSync(f.binaryPath, original);
  try {
    if (state === "changed") fs.writeFileSync(f.binaryPath, "replaced origin");
    check();
  } finally {
    fs.rmSync(f.binaryPath, { force: true });
    fs.renameSync(original, f.binaryPath);
  }
}

function withFrozenReceipt(
  f: Fixture,
  variant: "bytes" | "path" | "origin" | "valid" | "legacy",
  check: (retainedPath: string) => void,
) {
  const frozen = frozenIdentity(f, variant === "path" ? "other" : "krr");
  try {
    const origin =
      variant === "origin" ? { ...f.receipt.binary, sha256: "0".repeat(64) } : f.receipt.binary;
    const receipt =
      variant === "legacy"
        ? f.receipt
        : {
            ...f.receipt,
            schema: "krr-native-cli-managed-build-v4",
            frozenFrom: origin,
            frozenBinary: frozen,
          };
    fs.writeFileSync(f.receiptPath, JSON.stringify(receipt));
    if (variant === "bytes") {
      fs.chmodSync(frozen.path, 0o755);
      fs.writeFileSync(frozen.path, "changed");
    }
    check(frozen.path);
  } finally {
    fs.unlinkSync(frozen.path);
  }
}

describe("managed native CLI frozen build receipts", () => {
  let f: Fixture;
  beforeAll(() => {
    f = createManagedReaderFixture();
  });
  afterAll(() => {
    if (f) fs.rmSync(f.root, { recursive: true, force: true });
  });

  const verify = (binary: string) =>
    verifyNativeBuildReceipt(f.receiptPath, binary, f.runtimePath, f.root);

  test.each(["available", "changed", "missing"] as const)(
    "v4 retains the selected Cargo proof with the original %s",
    (state) => {
      withFrozenReceipt(f, "valid", (retained) => {
        withOrigin(f, state, () => expect(verify(retained).binaryPath).toBe(retained));
      });
    },
  );

  test("v4 rejects executing the changed original in place of the retained copy", () => {
    withFrozenReceipt(f, "valid", () => {
      withOrigin(f, "changed", () => {
        expect(() => verify(f.binaryPath)).toThrow(/request path\/deadline differs/);
      });
    });
  });

  test.each([
    ["bytes", /managed execution binary changed after its receipt was written/],
    ["path", /frozen binary identity differs/],
    ["origin", /frozen binary origin differs/],
  ] as const)("v4 rejects tampering with %s", (variant, expected) => {
    withFrozenReceipt(f, variant, (retained) => expect(() => verify(retained)).toThrow(expected));
  });

  test("v3 rejects executing a relocated binary", () => {
    withFrozenReceipt(f, "legacy", (retained) => {
      expect(() => verify(retained)).toThrow(/request path\/deadline differs/);
    });
  });
  test("v3 rejects a missing original binary", () => {
    withFrozenReceipt(f, "legacy", () => {
      withOrigin(f, "missing", () => expect(() => verify(f.binaryPath)).toThrow(/ENOENT/));
    });
  });
});
