import { expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fixturePath, parseMarkerInventory } from "./svg-marker-browser-capture";

test("marker inventory rejects duplicate paths and altered capture contract", () => {
  const cases = Array.from({ length: 48 }, (_, index) => ({
    input:
      index === 0
        ? "tests/painting/marker/percent-values.svg"
        : `tests/painting/marker/${index}.svg`,
    inputSha256: "a".repeat(64),
    upstreamPngSha256: "b".repeat(64),
    requestedWidth: 300,
    requestedHeight: index === 0 ? 150 : 300,
  }));
  const input = {
    schema: "krr-svg-marker-capture-cases-v1",
    requestedWidth: 300,
    cases,
  };
  expect(parseMarkerInventory(input).cases).toHaveLength(48);
  expect(() => parseMarkerInventory({ ...input, requestedWidth: 200 })).toThrow();
  expect(() =>
    parseMarkerInventory({ ...input, cases: [...cases.slice(0, 47), cases[0]] }),
  ).toThrow();
  expect(() => parseMarkerInventory({ ...input, cases: cases.slice(1) })).toThrow();
  expect(() =>
    parseMarkerInventory({
      ...input,
      cases: cases.map((entry) => ({ ...entry, requestedHeight: 300 })),
    }),
  ).toThrow();
});

test("fixture resolver rejects traversal, links and nonfiles", () => {
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "krr-marker-path-")));
  try {
    fs.writeFileSync(path.join(root, "image.svg"), "fixture");
    fs.symlinkSync(path.join(root, "image.svg"), path.join(root, "alias.svg"));
    expect(fixturePath(root, "image.svg")).toBe(path.join(root, "image.svg"));
    for (const value of ["../image.svg", "/image.svg", "alias.svg", ".", "a//b"]) {
      expect(() => fixturePath(root, value)).toThrow();
    }
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
