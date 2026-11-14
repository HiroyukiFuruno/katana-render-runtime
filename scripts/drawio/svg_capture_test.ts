import { expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { assertMatchingDrawioSlugs, writeRawDrawioSvg } from "./svg-capture";

test("candidate SVG transport preserves the exact input bytes", () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "drawio-svg-bytes-"));
  const source = Buffer.from('<svg viewBox="0 0 2 2">日本語&#xA;</svg>', "utf8");
  const target = path.join(directory, "candidate.svg");
  try {
    writeRawDrawioSvg(target, source);
    expect(fs.readFileSync(target)).toEqual(source);
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test("candidate and fresh official SVG/PNG inventories must match", () => {
  expect(() =>
    assertMatchingDrawioSlugs(["a", "b"], ["a", "b"], ["a", "b"], ["a", "b"]),
  ).not.toThrow();
  expect(() => assertMatchingDrawioSlugs(["a", "b"], ["a"], ["a", "b"], ["a", "b"])).toThrow(
    "candidate and fresh official inventories must match Draw.io fixtures",
  );
  expect(() => assertMatchingDrawioSlugs(["a"], ["a"], ["a"], [])).toThrow(
    "candidate and fresh official inventories must match Draw.io fixtures",
  );
  expect(() => assertMatchingDrawioSlugs([], [], [], [])).toThrow(
    "candidate and fresh official inventories must match Draw.io fixtures",
  );
  expect(() => assertMatchingDrawioSlugs(["a", "a"], ["a", "a"], ["a", "a"], ["a", "a"])).toThrow(
    "candidate and fresh official inventories must match Draw.io fixtures",
  );
});
