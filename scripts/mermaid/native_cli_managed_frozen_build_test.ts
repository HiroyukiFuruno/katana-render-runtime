import { expect, test } from "bun:test";
import fs from "node:fs";
import path from "node:path";
import { fileSha256 } from "./mermaid-pair-manifest";
import { verifyNativeBuildReceipt } from "./native-build-receipt";
import { createManagedReaderFixture } from "./native-cli-managed-build-test-fixture";

function frozenIdentity(f: ReturnType<typeof createManagedReaderFixture>, name = "krr") {
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

test("v4 keeps the selected Cargo proof while executing the retained copy", () => {
  const f = createManagedReaderFixture();
  try {
    const frozen = frozenIdentity(f);
    const receipt = {
      ...f.receipt,
      schema: "krr-native-cli-managed-build-v4",
      frozenFrom: f.receipt.binary,
      frozenBinary: frozen,
    };
    fs.writeFileSync(f.receiptPath, JSON.stringify(receipt));
    expect(
      verifyNativeBuildReceipt(f.receiptPath, frozen.path, f.runtimePath, f.root).binaryPath,
    ).toBe(frozen.path);
    fs.writeFileSync(f.binaryPath, "replaced origin");
    expect(() =>
      verifyNativeBuildReceipt(f.receiptPath, f.binaryPath, f.runtimePath, f.root),
    ).toThrow();
    expect(
      verifyNativeBuildReceipt(f.receiptPath, frozen.path, f.runtimePath, f.root).binaryPath,
    ).toBe(frozen.path);
    fs.unlinkSync(f.binaryPath);
    expect(
      verifyNativeBuildReceipt(f.receiptPath, frozen.path, f.runtimePath, f.root).binaryPath,
    ).toBe(frozen.path);
  } finally {
    fs.rmSync(f.root, { recursive: true, force: true });
  }
});

test("v4 rejects changed copy, unrelated path and altered origin", () => {
  for (const variant of ["bytes", "path", "origin"] as const) {
    const f = createManagedReaderFixture();
    try {
      const frozen = frozenIdentity(f, variant === "path" ? "other" : "krr");
      const origin =
        variant === "origin" ? { ...f.receipt.binary, sha256: "0".repeat(64) } : f.receipt.binary;
      const receipt = {
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
      expect(() =>
        verifyNativeBuildReceipt(f.receiptPath, frozen.path, f.runtimePath, f.root),
      ).toThrow();
    } finally {
      fs.rmSync(f.root, { recursive: true, force: true });
    }
  }
});

test("v3 still rejects a relocated binary and loss of its target path", () => {
  const f = createManagedReaderFixture();
  try {
    const frozen = frozenIdentity(f);
    expect(() =>
      verifyNativeBuildReceipt(f.receiptPath, frozen.path, f.runtimePath, f.root),
    ).toThrow();
    fs.unlinkSync(f.binaryPath);
    expect(() =>
      verifyNativeBuildReceipt(f.receiptPath, f.binaryPath, f.runtimePath, f.root),
    ).toThrow();
  } finally {
    fs.rmSync(f.root, { recursive: true, force: true });
  }
});
