import fs from "node:fs";
import path from "node:path";
import { z } from "zod";
import { digestStringRecord, type FileRecord, verifyFileRecord } from "./native-build-receipt";
import {
  digest,
  environment,
  managedFileSchema,
  natural,
  ns,
  positive,
  strings,
} from "./native-cli-managed-input-schema";

export const nativeRawFdSchema = z
  .object({
    schema: z.literal("krr-native-raw-fd-closure-v1"),
    toolBefore: managedFileSchema,
    toolAfter: managedFileSchema,
    argv: strings,
    cwd: z.string(),
    attemptToken: digest,
    environment,
    environmentSha256: digest,
    identity: z
      .object({ pid: positive, parentPid: positive, sessionId: positive, processGroupId: positive })
      .strict(),
    actualExit: z.union([z.literal(0), z.literal(1)]),
    startedNs: ns,
    endedNs: ns,
    reaped: z.literal(true),
    groupClosed: z.literal(true),
    rawFdsClosed: z.literal(true),
    runLimitSeconds: z.number().positive().max(10),
    cleanupLimitSeconds: z.literal(10),
    stdout: managedFileSchema,
    stderr: managedFileSchema,
    rawIdentities: z.tuple([
      z.object({ device: natural, inode: positive }).strict(),
      z.object({ device: natural, inode: positive }).strict(),
    ]),
    holders: z.tuple([]),
    descendantClosureProved: z.literal(false),
  })
  .strict();

function bound(ok: boolean, label: string): void {
  if (!ok) throw new Error(`Native raw FD closure ${label}`);
}
function inspect(raw: string, pairs: Array<{ device: number; inode: number }>): void {
  let record: { [key: string]: string } = {};
  const flush = () => {
    if (Object.keys(record).length === 0) return;
    bound(
      record.D !== undefined &&
        record.i !== undefined &&
        record.f !== undefined &&
        record.t === "REG",
      "fields missing",
    );
    bound(
      pairs.some(
        (pair) =>
          pair.device === Number.parseInt(record.D ?? "", 16) && pair.inode === Number(record.i),
      ),
      "identity differs",
    );
    bound(record.a === "r", "unknown writable holder remains");
  };
  for (const line of raw.trimEnd().split("\n").filter(Boolean)) {
    const key = line[0] ?? "",
      value = line.slice(1);
    if (key === "p" || key === "f") {
      flush();
      record = key === "f" ? { f: value } : {};
    } else if (key !== "g") {
      bound(
        ["a", "t", "D", "i"].includes(key) && record[key] === undefined,
        "unknown/duplicate field",
      );
      record[key] = value;
    }
  }
  flush();
}

export function verifyNativeRawFdClosure(
  value: unknown,
  owner: {
    cwd: string;
    attemptToken: string;
    environment: { [key: string]: string };
    pid: number;
    parentPid: number;
    startedNs: string;
    endedNs: string;
  },
  raw: Array<{ path: string; sha256: string }>,
  lifecyclePath: string,
): void {
  const proof = nativeRawFdSchema.parse(value);
  const fixed = process.platform === "darwin" ? "/usr/sbin/lsof" : "/usr/bin/lsof";
  bound(["darwin", "linux"].includes(process.platform), "platform unsupported");
  bound(
    proof.toolBefore.path === fs.realpathSync(fixed) &&
      proof.toolBefore.path === proof.toolAfter.path &&
      proof.toolBefore.sha256 === proof.toolAfter.sha256 &&
      proof.toolBefore.size === proof.toolAfter.size,
    "tool pin differs",
  );
  verifyFileRecord(proof.toolBefore, "native FD observer tool");
  bound(
    JSON.stringify(proof.argv) ===
      JSON.stringify([
        proof.toolBefore.path,
        "-nP",
        "-FpgfatiD",
        "--",
        ...raw.map((pin) => pin.path),
      ]),
    "argv differs",
  );
  bound(
    proof.cwd === owner.cwd &&
      proof.attemptToken === owner.attemptToken &&
      proof.environmentSha256 === digestStringRecord(owner.environment) &&
      proof.environmentSha256 === digestStringRecord(proof.environment),
    "environment/attempt differs",
  );
  bound(
    proof.identity.parentPid === owner.parentPid &&
      proof.identity.pid !== owner.pid &&
      proof.identity.sessionId === proof.identity.pid &&
      proof.identity.processGroupId === proof.identity.pid &&
      BigInt(proof.startedNs) >= BigInt(owner.startedNs) &&
      BigInt(proof.endedNs) <= BigInt(owner.endedNs) &&
      BigInt(proof.endedNs) >= BigInt(proof.startedNs),
    "process/time differs",
  );
  for (const [index, pin] of raw.entries()) {
    const stat = fs.statSync(pin.path),
      identity = proof.rawIdentities[index];
    bound(
      identity !== undefined && stat.dev === identity.device && stat.ino === identity.inode,
      "raw inode differs",
    );
  }
  for (const [pin, name] of [
    [proof.stdout, "native-fd-stdout.raw"],
    [proof.stderr, "native-fd-stderr.raw"],
  ] as const) {
    bound(pin.path === path.join(path.dirname(lifecyclePath), name), "observer raw role differs");
    verifyFileRecord(pin, "native FD observer raw");
  }
  bound(proof.stderr.size === 0, "observer coverage unknown");
  verifyManagedArtifactRolePins(
    [
      proof.stdout,
      proof.stderr,
      ...raw.map((pin) => ({ ...pin, size: fs.statSync(pin.path).size })),
    ],
    path.dirname(path.dirname(path.dirname(lifecyclePath))),
  );
  inspect(fs.readFileSync(proof.stdout.path, "utf8"), proof.rawIdentities);
}

export function verifyManagedArtifactRolePins(pins: FileRecord[], attempt: string): void {
  const identities = new Set<string>();
  for (const pin of pins) {
    bound(pin.path.startsWith(`${attempt}/`), "artifact outside owned attempt");
    verifyFileRecord(pin, "managed artifact role");
    const stat = fs.statSync(pin.path),
      identity = `${stat.dev}:${stat.ino}`;
    bound(!identities.has(identity), "artifact role aliases inode");
    identities.add(identity);
  }
}
