import fs from "node:fs";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";
import { fileSha256, type parseJsonValue } from "./mermaid-pair-manifest";
import {
  canonicalRegularFile,
  digestStringRecord,
  type FileRecord,
  verifyFileRecord,
} from "./native-build-receipt";
import {
  type ManagedObservation,
  type ManagedOwned,
  type ManagedReceipt,
  type ManagedRequest,
  managedLifecycleSchema,
  managedOwnedSchema,
} from "./native-cli-managed-build-schema";
import {
  verifyManagedArtifactRolePins,
  verifyNativeRawFdClosure,
} from "./native-cli-managed-raw-fd";

export { verifyManagedArtifactRolePins } from "./native-cli-managed-raw-fd";

import { parseManagedJson } from "./native-cli-managed-json";

export function requireBound(ok: boolean, label: string): void {
  if (!ok) throw new Error(`Managed native CLI ${label}`);
}
export function same(left: unknown, right: unknown, label: string): void {
  requireBound(isDeepStrictEqual(left, right), `${label} differs`);
}
export function read(pin: FileRecord): ReturnType<typeof parseJsonValue> {
  verifyFileRecord(pin, "managed evidence");
  return parseManagedJson(fs.readFileSync(pin.path, "utf8"), "managed evidence");
}
export function owned(
  record: ManagedOwned,
  lifecyclePin: FileRecord,
  role: "build" | "metadata" | "tool-query",
): void {
  const lifecycle = managedLifecycleSchema.parse(read(lifecyclePin));
  requireBound(
    path.basename(lifecyclePin.path) === "lifecycle.json",
    "lifecycle file role differs",
  );
  same(lifecycle.argv, record.argv, "owner argv");
  requireBound(
    lifecycle.role === role &&
      lifecycle.cwd === record.cwd &&
      lifecycle.identity.pid === record.pid &&
      lifecycle.identity.sessionId === record.pid &&
      lifecycle.identity.processGroupId === record.pid &&
      lifecycle.runLimitSeconds === { build: 1800, metadata: 120, "tool-query": 30 }[role] &&
      BigInt(record.endedNs) >= BigInt(record.startedNs),
    "owner lifecycle binding differs",
  );
  same(
    record.environmentSha256,
    digestStringRecord(record.environment),
    "owner environment digest",
  );
  for (const [index, pin] of [record.stdout, record.stderr].entries()) {
    verifyFileRecord(pin, "managed raw output");
    const raw = lifecycle.raw[index];
    if (raw === undefined) throw new Error("Managed native CLI lifecycle raw missing");
    requireBound(raw.sha256 === pin.sha256, "raw lifecycle hash differs");
    requireBound(
      raw.path ===
        path.join(path.dirname(lifecyclePin.path), index === 0 ? "stdout.raw" : "stderr.raw"),
      "owner raw file role differs",
    );
    canonicalRegularFile(raw.path, "owner raw output");
    requireBound(
      fileSha256(raw.path) === pin.sha256 && fs.statSync(raw.path).size === pin.size,
      "raw copy differs from original owner FD output",
    );
  }
  verifyNativeRawFdClosure(
    lifecycle.nativeRawFdClosure,
    { ...record, parentPid: lifecycle.identity.parentPid },
    lifecycle.raw,
    lifecyclePin.path,
  );
}

export function verifyManagedProcessEnvironment(receipt: ManagedReceipt): string[] {
  const env = receipt.snapshotAfter.environment;
  const empty = [
    "KRR_BUILD_RECEIPT",
    "MERMAID_RASTER_SESSION_CONFIG",
    "MERMAID_RASTER_SESSION_CONFIG_SHA",
    "MERMAID_RASTER_BINDING_CONFIG",
    "MERMAID_RASTER_BINDING_CONFIG_SHA",
  ];
  const allowed = new Set([
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "TERM",
    "TMPDIR",
    "SHELL",
    "RUSTFLAGS",
    "CARGO_CAPACITY_PROTECTED_REFS",
    "CARGO_TARGET_DIR",
    "PWD",
    "SHLVL",
    "_",
    "__CF_USER_TEXT_ENCODING",
    ...empty,
  ]);
  requireBound(
    Object.keys(env).every((name) => allowed.has(name)) && !env.CARGO_TARGET_DIR,
    "unsupported Cargo environment",
  );
  const python = env.PATH?.split(path.delimiter)
    .map((base) => path.join(base, "python3"))
    .find((name) => fs.existsSync(name) && (fs.statSync(name).mode & 0o111) !== 0);
  requireBound(
    python !== undefined &&
      path.isAbsolute(python) &&
      env._ === python &&
      fs.realpathSync(python) === receipt.snapshotAfter.tools.python3.path,
    "Just Python invocation differs",
  );
  const encoding = env.__CF_USER_TEXT_ENCODING;
  requireBound(
    encoding === undefined ||
      (process.platform === "darwin" &&
        /^0x[0-9a-f]+:0x[0-9a-f]+:0x[0-9a-f]+$/i.test(encoding) &&
        Number.parseInt(encoding.split(":")[0] ?? "", 16) === process.getuid?.()),
    "Darwin environment differs",
  );
  const bases: string[] = [];
  for (let base = receipt.cwd; ; base = path.dirname(base)) {
    bases.push(base);
    if (base === path.dirname(base)) break;
  }
  requireBound(env.HOME !== undefined && path.isAbsolute(env.HOME), "HOME absent");
  const inventory = [
    ...new Set([
      ...bases.flatMap((base) => [
        path.join(base, ".cargo/config"),
        path.join(base, ".cargo/config.toml"),
      ]),
      path.join(env.HOME ?? "", ".cargo/config"),
      path.join(env.HOME ?? "", ".cargo/config.toml"),
    ]),
  ];
  requireBound(
    inventory.every((name) => fs.lstatSync(name, { throwIfNoEntry: false }) === undefined),
    "Cargo config appeared",
  );
  return inventory;
}

export function verifyManagedRoles(
  receipt: ManagedReceipt,
  request: ManagedRequest,
  observation: ManagedObservation,
  receiptPath: string,
): void {
  const attempt = path.dirname(receiptPath);
  requireBound(
    request.sourceManifest.path === path.join(attempt, "source-manifest.json") &&
      request.metadata.path === path.join(attempt, "metadata.json") &&
      request.metadataObservation.path === path.join(attempt, "metadata-observation.json"),
    "input artifact role differs",
  );
  const pins = [
    receipt.request,
    receipt.observation,
    request.sourceManifest,
    request.metadata,
    request.metadataObservation,
    receipt.buildObservation.stdout,
    receipt.buildObservation.stderr,
    receipt.metadataLifecycle,
    receipt.hostPrebuild.lifecycle,
    receipt.hostPrebuild.observation.stdout,
    receipt.hostPrebuild.observation.stderr,
    receipt.buildLifecycle,
    observation.ownedCargo.stdout,
    observation.ownedCargo.stderr,
    observation.capacityBefore,
    observation.capacityAfter,
  ];
  const metadata = managedOwnedSchema.parse(read(request.metadataObservation));
  pins.push(metadata.stderr);
  for (const probe of receipt.toolProbes)
    pins.push(probe.lifecycle, probe.observation.stdout, probe.observation.stderr);
  verifyManagedArtifactRolePins(pins, attempt);
  requireBound(
    !receipt.expectedSource.path.startsWith(`${attempt}/`) &&
      !receipt.expectedProtectedRegistry.path.startsWith(`${attempt}/`),
    "independent admission is inside attempt",
  );
}
