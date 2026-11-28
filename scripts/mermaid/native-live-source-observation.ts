import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { isDeepStrictEqual, parseArgs } from "node:util";
import { type JsonValue, parseJsonValue, sha256 } from "./mermaid-pair-manifest";
import { parseSourceManifestValue } from "./native-build-receipt";
import { captureNativeSourceManifest } from "./native-source-manifest";

type GitState = { head: string; indexSha256: string; statusBase64: string };

export type LiveSourceObservation = GitState & {
  schema: "krr-html-performance-live-source-observation-v1";
  root: string;
  manifest: ReturnType<typeof captureNativeSourceManifest>;
};

function gitState(root: string): GitState {
  const options = { cwd: root, env: { ...process.env, GIT_OPTIONAL_LOCKS: "0" } };
  const git = (args: string[]) => execFileSync("git", args, options);
  const head = git(["rev-parse", "--verify", "HEAD"]).toString("utf8").trim();
  if (!/^(?:[0-9a-f]{40}|[0-9a-f]{64})$/.test(head)) throw new Error("Invalid source HEAD");
  const indexName = git(["rev-parse", "--git-path", "index"]).toString("utf8").trim();
  const index = path.resolve(root, indexName);
  const indexSha256 = sha256(fs.readFileSync(index));
  const statusBase64 = git(["status", "--porcelain=v1", "-z", "--untracked-files=all"]).toString(
    "base64",
  );
  return { head, indexSha256, statusBase64 };
}

function requireCanonicalRoot(root: string): void {
  if (!path.isAbsolute(root) || path.resolve(root) !== root || fs.realpathSync(root) !== root) {
    throw new Error("Absolute canonical source root is required");
  }
}

/** 観測だけを保存する。source 合格や SDK の同一性を証明しない。 */
export function captureLiveSourceObservation(root: string): LiveSourceObservation {
  requireCanonicalRoot(root);
  const before = gitState(root);
  const first = captureNativeSourceManifest(root);
  const second = captureNativeSourceManifest(root);
  const after = gitState(root);
  if (!isDeepStrictEqual(before, after) || !isDeepStrictEqual(first, second)) {
    throw new Error("Live source changed during observation");
  }
  return {
    schema: "krr-html-performance-live-source-observation-v1",
    root,
    ...after,
    manifest: second,
  };
}

/** 保存済み観測の構造のみを検査する。現在の checkout や SDK は検証しない。 */
export function parseLiveSourceObservation(value: JsonValue): LiveSourceObservation {
  if (value === null || Array.isArray(value) || typeof value !== "object") {
    throw new Error("Invalid live source observation");
  }
  const keys = Object.keys(value).sort();
  if (
    !isDeepStrictEqual(keys, ["head", "indexSha256", "manifest", "root", "schema", "statusBase64"])
  ) {
    throw new Error("Invalid live source observation fields");
  }
  if (
    value.schema !== "krr-html-performance-live-source-observation-v1" ||
    typeof value.root !== "string" ||
    !path.isAbsolute(value.root) ||
    path.resolve(value.root) !== value.root ||
    typeof value.head !== "string" ||
    !/^(?:[0-9a-f]{40}|[0-9a-f]{64})$/.test(value.head) ||
    typeof value.indexSha256 !== "string" ||
    !/^[0-9a-f]{64}$/.test(value.indexSha256) ||
    typeof value.statusBase64 !== "string" ||
    Buffer.from(value.statusBase64, "base64").toString("base64") !== value.statusBase64
  ) {
    throw new Error("Invalid live source observation identity");
  }
  if (value.manifest === undefined) throw new Error("Live source manifest is missing");
  const manifest = parseSourceManifestValue(value.manifest);
  if (!isDeepStrictEqual(value.manifest, manifest)) {
    throw new Error("Invalid live source manifest fields");
  }
  for (const file of manifest.files) {
    const relative = path.relative(value.root, file.path);
    if (
      !relative ||
      relative === ".." ||
      relative.startsWith(`..${path.sep}`) ||
      path.isAbsolute(relative)
    ) {
      throw new Error("Live source manifest path escapes root");
    }
  }
  return {
    schema: "krr-html-performance-live-source-observation-v1",
    root: value.root,
    head: value.head,
    indexSha256: value.indexSha256,
    statusBase64: value.statusBase64,
    manifest,
  };
}

/** 現在の checkout と比較するため、過去 endpoint の構造検証には使わない。 */
export function verifyLiveSourceObservation(root: string, value: JsonValue): LiveSourceObservation {
  requireCanonicalRoot(root);
  const parsed = parseLiveSourceObservation(value);
  const current = captureLiveSourceObservation(root);
  if (!isDeepStrictEqual(parsed, current)) {
    throw new Error("Live source observation differs");
  }
  return current;
}

if (import.meta.main) {
  try {
    const { values } = parseArgs({
      args: process.argv.slice(2),
      options: { root: { type: "string" }, verify: { type: "string" } },
      strict: true,
      allowPositionals: false,
    });
    if (typeof values.root !== "string") throw new Error("Absolute canonical --root is required");
    const observation = values.verify
      ? verifyLiveSourceObservation(
          values.root,
          parseJsonValue(fs.readFileSync(values.verify, "utf8"), "live source observation"),
        )
      : captureLiveSourceObservation(values.root);
    process.stdout.write(`${JSON.stringify(observation)}\n`);
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  }
}
