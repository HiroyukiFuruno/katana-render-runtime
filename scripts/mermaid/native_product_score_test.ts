import { expect, test } from "bun:test";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { makeOnePixelPng } from "./mermaid_pair_test_support";
import { scoreNativeProductRows } from "./native-product-score";
import { MagickOps } from "./reference_image_ops";
import type { ReferencePair } from "./reference_pair_repository";

interface ScoreFixture {
  root: string;
  officialPath: string;
  browserPath: string;
  nativePath: string;
  pair: ReferencePair;
}

function writePng(directory: string, name: string, seed: string, rectangleX: number): string {
  const output = path.join(directory, name);
  execFileSync("magick", [
    seed,
    "-resize",
    "8x8!",
    "-fill",
    "#2d2d2d",
    "-draw",
    "rectangle 0,0 7,7",
    "-fill",
    "#e0e0e0",
    "-draw",
    `rectangle ${rectangleX},1 ${rectangleX + 4},5`,
    output,
  ]);
  return output;
}

function createScoreFixture(): ScoreFixture {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "krr-native-product-score-")),
    seed = path.join(root, "seed.png");
  fs.writeFileSync(seed, makeOnePixelPng(), { flag: "wx" });
  const officialPath = writePng(root, "official.png", seed, 1),
    browserPath = path.join(root, "browser.png"),
    nativePath = writePng(root, "native.png", seed, 2),
    slug = "native-product-score-fixture";
  fs.copyFileSync(officialPath, browserPath);
  return {
    root,
    officialPath,
    browserPath,
    nativePath,
    pair: {
      slug,
      officialPath,
      katanaPath: browserPath,
      officialSvgPath: path.join(root, "official.svg"),
      katanaSvgPath: path.join(root, "candidate.svg"),
    },
  };
}

test("native product RGB score reads actual frame PNG, not the browser comparison PNG", () => {
  const fixture = createScoreFixture();
  try {
    expectBrowserPairIsIdentical(fixture);
    expectNativeFrameMismatchFails(fixture);
    expectMatchingNativeFramePasses(fixture);
    expectMissingNativeFrameIsRejected(fixture);
  } finally {
    fs.rmSync(fixture.root, { recursive: true, force: true });
  }
});

function expectBrowserPairIsIdentical(fixture: ScoreFixture): void {
  const directory = path.join(fixture.root, "browser-magick"),
    magick = new MagickOps(directory, null, "dark");
  magick.prepare();
  expect(
    magick.comparePaintProjectedImages(
      fixture.officialPath,
      fixture.browserPath,
      path.join(directory, "work"),
      fixture.pair.slug,
    ).score,
  ).toBe(100);
}

function expectNativeFrameMismatchFails(fixture: ScoreFixture): void {
  const rows = scoreFixture(fixture, fixture.nativePath, "native-magick");
  expect(rows[0]?.rgbScore).toBeLessThan(99);
  expect(rows[0]?.passed).toBe(false);
}

function expectMatchingNativeFramePasses(fixture: ScoreFixture): void {
  const rows = scoreFixture(fixture, fixture.officialPath, "matching-magick");
  expect(rows[0]?.rgbScore).toBe(100);
  expect(rows[0]?.passed).toBe(true);
}

function expectMissingNativeFrameIsRejected(fixture: ScoreFixture): void {
  const directory = path.join(fixture.root, "missing-magick");
  expect(() =>
    scoreNativeProductRows(
      [fixture.pair],
      [],
      new Map(),
      new MagickOps(directory, null, "dark"),
      directory,
    ),
  ).toThrow("Native product frame inventory differs");
}

function scoreFixture(fixture: ScoreFixture, pngPath: string, name: string) {
  const directory = path.join(fixture.root, name),
    frame = { slug: fixture.pair.slug, png: { path: pngPath } };
  return scoreNativeProductRows(
    [fixture.pair],
    [frame],
    new Map(),
    new MagickOps(directory, null, "dark"),
    directory,
  );
}
