import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { parseArgs } from "node:util";
import { sha256 } from "./mermaid-pair-manifest";
import {
  assertCurrentGitSourceSet,
  canonicalRegularFile,
  digestFileRecords,
  type FileRecord,
  type parseSourceManifest,
  verifyFileRecord,
} from "./native-build-receipt";

export function captureNativeSourceManifest(
  repositoryRoot: string,
): ReturnType<typeof parseSourceManifest> {
  const root = canonicalDirectory(repositoryRoot);
  const tracked = gitPaths(root, ["ls-files", "-z"]);
  const untracked = gitPaths(root, ["ls-files", "--others", "--exclude-standard", "-z"]);
  const names = [...new Set([...tracked, ...untracked])].sort((left, right) =>
    Buffer.compare(Buffer.from(left, "utf8"), Buffer.from(right, "utf8")),
  );
  for (const name of names) assertSourceRootPath(name);

  const files = names.map((name) => captureFile(root, name));
  const manifest = {
    schema: "krr-native-build-source-manifest-v1" as const,
    digest: digestFileRecords(files),
    files,
  };

  for (const file of files) verifyFileRecord(file, "captured native source");
  assertCurrentGitSourceSet(root, files);
  return manifest;
}

function canonicalDirectory(directory: string): string {
  if (!path.isAbsolute(directory) || path.resolve(directory) !== directory) {
    throw new Error("Native source repository root must be absolute and canonical");
  }
  const stats = fs.lstatSync(directory);
  if (stats.isSymbolicLink() || !stats.isDirectory() || fs.realpathSync(directory) !== directory) {
    throw new Error("Native source repository root must be a canonical directory");
  }
  const gitRootOutput = execFileSync("git", ["rev-parse", "--show-toplevel"], { cwd: directory });
  const gitRootText = gitRootOutput.toString("utf8");
  const gitRoot = gitRootText.endsWith("\r\n")
    ? gitRootText.slice(0, -2)
    : gitRootText.endsWith("\n")
      ? gitRootText.slice(0, -1)
      : gitRootText;
  if (!gitRoot || fs.realpathSync(path.resolve(gitRoot)) !== directory) {
    throw new Error("Native source repository root must be the Git top-level directory");
  }
  return directory;
}

function gitPaths(root: string, args: string[]): string[] {
  const output = execFileSync("git", args, { cwd: root });
  const names: string[] = [];
  let start = 0;
  for (let end = 0; end < output.length; end += 1) {
    if (output[end] !== 0) continue;
    if (end > start) {
      const bytes = output.subarray(start, end);
      const name = bytes.toString("utf8");
      if (!Buffer.from(name, "utf8").equals(bytes)) {
        throw new Error("Git source path is not valid UTF-8");
      }
      names.push(name);
    }
    start = end + 1;
  }
  if (start !== output.length) throw new Error("Git source path output is not NUL terminated");
  return names;
}

function captureFile(root: string, name: string): FileRecord {
  if (
    path.posix.isAbsolute(name) ||
    path.posix.normalize(name) !== name ||
    name === ".." ||
    name.startsWith("../")
  ) {
    throw new Error(`Git source path escapes repository: ${name}`);
  }
  const filePath = path.join(root, name);
  const relativePath = path.relative(root, filePath);
  if (
    relativePath.length === 0 ||
    relativePath === ".." ||
    relativePath.startsWith(`..${path.sep}`) ||
    path.isAbsolute(relativePath)
  ) {
    throw new Error(`Git source path escapes repository: ${name}`);
  }
  const canonicalPath = canonicalRegularFile(filePath, "native source");
  if (canonicalPath !== filePath) throw new Error("Git source path is not canonical");

  const before = fs.lstatSync(filePath, { bigint: true });
  if (!before.isFile() || before.isSymbolicLink())
    throw new Error(`Native source is not a regular file: ${name}`);
  const noFollow = fs.constants.O_NOFOLLOW ?? 0;
  const descriptor = fs.openSync(filePath, fs.constants.O_RDONLY | noFollow);
  try {
    const openedBefore = fs.fstatSync(descriptor, { bigint: true });
    if (!sameFileState(before, openedBefore))
      throw new Error(`Native source changed before read: ${name}`);
    const sha = sha256(fs.readFileSync(descriptor));
    const openedAfter = fs.fstatSync(descriptor, { bigint: true });
    const after = fs.lstatSync(filePath, { bigint: true });
    if (
      !sameFileState(openedBefore, openedAfter) ||
      !sameFileState(openedAfter, after) ||
      fs.realpathSync(filePath) !== canonicalPath
    ) {
      throw new Error(`Native source changed while being read: ${name}`);
    }
    return { path: canonicalPath, sha256: sha, size: Number(openedAfter.size) };
  } finally {
    fs.closeSync(descriptor);
  }
}

function assertSourceRootPath(name: string): void {
  const rootSegment = name.split("/")[0];
  if (rootSegment === "tmp" || rootSegment === "target") {
    throw new Error(`Git-visible source path is outside the native source set: ${name}`);
  }
}

function sameFileState(
  left: Pick<fs.BigIntStats, "dev" | "ino" | "size" | "mtimeNs" | "ctimeNs" | "mode">,
  right: Pick<fs.BigIntStats, "dev" | "ino" | "size" | "mtimeNs" | "ctimeNs" | "mode">,
): boolean {
  return (
    left.dev === right.dev &&
    left.ino === right.ino &&
    left.size === right.size &&
    left.mtimeNs === right.mtimeNs &&
    left.ctimeNs === right.ctimeNs &&
    left.mode === right.mode
  );
}

if (import.meta.main) {
  try {
    const { values, positionals } = parseArgs({
      args: process.argv.slice(2),
      options: { root: { type: "string" } },
      strict: true,
      allowPositionals: false,
    });
    if (positionals.length !== 0 || typeof values.root !== "string") {
      throw new Error(
        "Usage: bun run scripts/mermaid/native-source-manifest.ts --root ABSOLUTE_CANONICAL_ROOT",
      );
    }
    process.stdout.write(`${JSON.stringify(captureNativeSourceManifest(values.root))}\n`);
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    process.stderr.write(`${detail}\n`);
    process.exitCode = 1;
  }
}
