import { expect, test } from "bun:test";
import path from "node:path";
import {
  countRgbaDifferences,
  parseCaptureManifest,
  parseNativeDiagnostics,
  pixelsDiffer,
} from "./svg-marker-browser-diagnostic";
import { markerInventoryFixture } from "./svg-marker-inventory-test-fixture";

const crateRoot = path.resolve("/repo/crates/katana-render-runtime");
const repoRoot = path.resolve(crateRoot, "../..");
const fixtureRoot = path.join(repoRoot, "tests/fixtures/svg-engine/upstream/resvg/tests");
const inventorySha256 = "f".repeat(64);

function nativeLog(entries: readonly { input: string }[]): string {
  const lines: string[] = [];
  for (const { input } of entries) {
    for (let x = 0; x < 8; x += 1) {
      lines.push(
        `pixel (${x},0) differs: actual=Some([12, 20, 30, 255]) expected=Some([10, 20, 30, 255])`,
      );
    }
    lines.push(
      `assertion \`left == right\` failed: upstream fixture ${path.join(fixtureRoot, input)}`,
    );
  }
  return lines.join("\n");
}

function captureManifest(): Record<string, unknown> {
  return {
    schema: "krr-svg-marker-browser-capture-v1",
    diagnosticOnly: true,
    requestedWidth: 300,
    inventorySha256,
    browserVersion: "Chrome fixture",
    tool: { executable: "/fixture/chrome", sha256: "1".repeat(64) },
    captures: markerInventoryFixture().cases.map((entry, index) => ({
      ...entry,
      png: { path: `${String(index).padStart(2, "0")}.png`, sha256: "2".repeat(64) },
      resources: [],
      font: { path: "fonts/NotoSans-Regular.ttf", sha256: "3".repeat(64) },
    })),
  };
}

test("joins all 48 logged native groups to their fixed upstream paths", () => {
  const entries = markerInventoryFixture().cases;
  const cases = parseNativeDiagnostics(nativeLog(entries), crateRoot, entries);
  expect(cases).toHaveLength(48);
  expect(cases.every((entry) => entry.samples.length === 8)).toBe(true);
  expect(cases.map((entry) => entry.input)).toEqual(entries.map((entry) => entry.input));
});

test.each([
  "duplicate path",
  "outside path",
  "missing pixels",
  "None pixel",
  "out-of-range pixel",
  "case height overflow",
] as const)("rejects malformed native diagnostic: %s", (mutation) => {
  const entries = markerInventoryFixture().cases;
  let text = nativeLog(entries);
  if (mutation === "duplicate path") {
    const second = path.join(fixtureRoot, entries[1]?.input ?? "");
    text = text.replace(second, path.join(fixtureRoot, entries[0]?.input ?? ""));
  }
  if (mutation === "outside path") {
    text = text.replace(path.join(fixtureRoot, entries[0]?.input ?? ""), "/outside/fixture.svg");
  }
  if (mutation === "missing pixels") {
    text = text.replace(
      /pixel \(\d+,0\) differs: actual=Some\(\[12, 20, 30, 255\]\) expected=Some\(\[10, 20, 30, 255\]\)\n/g,
      "",
    );
  }
  if (mutation === "None pixel")
    text = text.replace("actual=Some([12, 20, 30, 255])", "actual=None");
  if (mutation === "out-of-range pixel") text = text.replace("pixel (0,0)", "pixel (300,0)");
  if (mutation === "case height overflow") {
    const shortCase = entries.find((entry) => entry.requestedHeight === 150);
    if (!shortCase) throw new Error("short-height inventory case is missing");
    const assertion = `assertion \`left == right\` failed: upstream fixture ${path.join(fixtureRoot, shortCase.input)}`;
    const assertionOffset = text.indexOf(assertion);
    const pixelOffset = text.lastIndexOf("pixel (0,0)", assertionOffset);
    if (assertionOffset < 0 || pixelOffset < 0)
      throw new Error("short-height native group is missing");
    text = `${text.slice(0, pixelOffset)}${text.slice(pixelOffset).replace("pixel (0,0)", "pixel (0,150)")}`;
  }
  expect(() => parseNativeDiagnostics(text, crateRoot, entries)).toThrow();
});

test("capture manifest preserves every inventory hash and canonical PNG path", () => {
  const expected = markerInventoryFixture();
  expect(parseCaptureManifest(captureManifest(), expected, inventorySha256).captures).toHaveLength(
    48,
  );
  const missing = captureManifest();
  missing.captures = (missing.captures as unknown[]).slice(1);
  expect(() => parseCaptureManifest(missing, expected, inventorySha256)).toThrow();
  const altered = captureManifest();
  const captures = altered.captures as Array<Record<string, unknown>>;
  const first = captures[0];
  if (!first) throw new Error("capture fixture is empty");
  captures[0] = { ...first, inputSha256: "9".repeat(64) };
  expect(() => parseCaptureManifest(altered, expected, inventorySha256)).toThrow();
  const duplicate = captureManifest();
  const duplicateCaptures = duplicate.captures as Array<Record<string, unknown>>;
  const firstDuplicate = duplicateCaptures[0];
  if (!firstDuplicate) throw new Error("capture fixture is empty");
  duplicateCaptures[1] = firstDuplicate;
  expect(() => parseCaptureManifest(duplicate, expected, inventorySha256)).toThrow();
});

test("rejects wrong capture inventory digest and unsafe browser PNG paths", () => {
  expect(() =>
    parseCaptureManifest(captureManifest(), markerInventoryFixture(), "0".repeat(64)),
  ).toThrow();
  const altered = captureManifest();
  const captures = altered.captures as Array<Record<string, unknown>>;
  const first = captures[0];
  if (!first) throw new Error("capture fixture is empty");
  captures[0] = { ...first, png: { path: "../capture.png", sha256: "2".repeat(64) } };
  expect(() => parseCaptureManifest(altered, markerInventoryFixture(), inventorySha256)).toThrow();
  const wrongHeight = captureManifest();
  const wrongHeightCaptures = wrongHeight.captures as Array<Record<string, unknown>>;
  const shortCapture = wrongHeightCaptures.find(
    (entry) => entry.input === "tests/painting/marker/percent-values.svg",
  );
  if (!shortCapture) throw new Error("short-height browser capture is missing");
  shortCapture.requestedHeight = 300;
  expect(() =>
    parseCaptureManifest(wrongHeight, markerInventoryFixture(), inventorySha256),
  ).toThrow();
});

test("uses one-channel tolerance and ignores RGB under fully transparent pixels", () => {
  const expected = new Uint8Array(300 * 150 * 4);
  const actual = new Uint8Array(expected.length);
  for (let offset = 0; offset < expected.length; offset += 4) {
    expected.set([10, 20, 30, 255], offset);
    actual.set([10, 20, 30, 255], offset);
  }
  actual[0] = 11;
  actual[4] = 12;
  expected.set([0, 0, 0, 0], 8);
  actual.set([255, 200, 100, 0], 8);
  expect(pixelsDiffer([10, 20, 30, 255], [11, 20, 30, 255])).toBe(false);
  expect(pixelsDiffer([10, 20, 30, 255], [12, 20, 30, 255])).toBe(true);
  expect(countRgbaDifferences(actual, expected, 300, 150)).toBe(1);
  expect(() => countRgbaDifferences(actual, expected, 300, 300)).toThrow();
});
