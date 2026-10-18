import { afterEach, expect, test } from "bun:test";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, symlinkSync, unlinkSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";

import type { Format } from "./duplication-schema";
import { collectOwnedSourcePaths } from "./owned-source-inventory";

const fixtures: string[] = [];
const formats: Format[] = ["rust", "typescript", "javascript", "python"];

afterEach(() => {
  for (const repo of fixtures.splice(0)) rmSync(repo, { recursive: true, force: true });
});

function fixture(): string {
  const repo = mkdtempSync(path.join(os.tmpdir(), "krr-owned-inventory-"));
  fixtures.push(repo);
  execFileSync("git", ["init", "-q", repo]);
  return repo;
}

function write(repo: string, file: string, value = "export const value = 1;\n"): void {
  const destination = path.join(repo, file);
  mkdirSync(path.dirname(destination), { recursive: true });
  writeFileSync(destination, value);
}

function inventory(repo: string, selectedFormats = formats): string[] {
  return collectOwnedSourcePaths(repo, {
    roots: ["scripts", "crates"],
    excludes: ["generated", "vendor"],
    formats: selectedFormats,
  }).map((file) => path.relative(repo, file).replaceAll("\\", "/"));
}

test("cached and nonignored untracked source including tests share the same sorted inventory", () => {
  const repo = fixture();
  const files = [
    "scripts/unit test.py",
    "crates/demo/src/lib.rs",
    "scripts/helper.mts",
    "scripts/host.js",
  ];
  for (const file of files) write(repo, file);
  execFileSync("git", ["add", "--", files[1] ?? "", files[3] ?? ""], { cwd: repo });
  expect(inventory(repo)).toEqual([...files].sort());
});

test.each([
  ["scripts/ignored.ts", "/scripts/ignored.ts\n"],
  ["documents/reference.ts", ""],
  ["scripts/generated/output.ts", ""],
  ["scripts/vendor/upstream.js", ""],
  ["scripts/fixture.html", ""],
])("existing selection boundary excludes %s", (file, ignored) => {
  const repo = fixture();
  write(repo, file);
  writeFileSync(path.join(repo, ".gitignore"), ignored);
  expect(inventory(repo)).toEqual([]);
});

test("format selection remains explicit without removing test source from that language", () => {
  const repo = fixture();
  for (const file of ["scripts/helper.ts", "scripts/helper_test.py"]) write(repo, file);
  expect(inventory(repo, ["python"])).toEqual(["scripts/helper_test.py"]);
});

test("cached sources remain visible even if a later ignore pattern matches", () => {
  const repo = fixture();
  write(repo, "scripts/helper.ts");
  execFileSync("git", ["add", "--", "scripts/helper.ts"], { cwd: repo });
  writeFileSync(path.join(repo, ".gitignore"), "/scripts/helper.ts\n");
  expect(inventory(repo)).toEqual(["scripts/helper.ts"]);
});

test("original regular-file selection skips links and missing cached files still fail", () => {
  const repo = fixture();
  write(repo, "scripts/helper.ts");
  symlinkSync(path.join(repo, "scripts/helper.ts"), path.join(repo, "scripts/link.ts"));
  expect(inventory(repo)).toEqual(["scripts/helper.ts"]);
  execFileSync("git", ["add", "--", "scripts/helper.ts"], { cwd: repo });
  unlinkSync(path.join(repo, "scripts/helper.ts"));
  expect(() => inventory(repo)).toThrow();
});
