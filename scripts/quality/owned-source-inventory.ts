import { execFileSync } from "node:child_process";
import { lstatSync } from "node:fs";
import path from "node:path";

import type { Format, Policy } from "./duplication-schema";

export const OWNED_SOURCE_FORMATS: Readonly<Record<string, Format>> = {
  ".rs": "rust",
  ".ts": "typescript",
  ".tsx": "typescript",
  ".mts": "typescript",
  ".cts": "typescript",
  ".js": "javascript",
  ".jsx": "javascript",
  ".mjs": "javascript",
  ".cjs": "javascript",
  ".py": "python",
  ".pyi": "python",
};

export function readOwnedRepositoryIndex(repo: string): { tracked: string[]; untracked: string[] } {
  const read = (flags: string[]): string[] =>
    execFileSync("git", ["ls-files", ...flags, "-z"], { cwd: repo, encoding: "utf8" })
      .split("\0")
      .filter(Boolean)
      .sort();
  return {
    tracked: read(["--cached"]),
    untracked: read(["--others", "--exclude-standard"]),
  };
}

export function collectOwnedSourcePaths(
  repo: string,
  policy: Pick<Policy, "roots" | "excludes" | "formats">,
): string[] {
  const entries = execFileSync(
    "git",
    ["ls-files", "--cached", "--others", "--exclude-standard", "-z"],
    { cwd: repo, encoding: "utf8" },
  );
  const result: string[] = [];
  const excluded = new Set(policy.excludes);
  for (const entry of entries.split("\0")) {
    if (entry === "") continue;
    const normalized = entry.replaceAll("\\", "/");
    const segments = normalized.split("/");
    if (!policy.roots.includes(segments[0] ?? "")) continue;
    if (segments.slice(1).some((segment) => excluded.has(segment))) continue;
    const format = OWNED_SOURCE_FORMATS[path.extname(normalized)];
    if (format === undefined || !policy.formats.includes(format)) continue;
    const absolute = path.resolve(repo, normalized);
    if (lstatSync(absolute).isFile()) result.push(absolute);
  }
  return result.sort();
}
