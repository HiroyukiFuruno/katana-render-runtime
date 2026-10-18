import { expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import type { JsonValue } from "./dynamic-state";
import { matchTocEntries, tocBinaryFromBuildReport, validateTocEntries } from "./toc-fidelity";

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
