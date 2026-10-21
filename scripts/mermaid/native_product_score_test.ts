import { expect, test } from "bun:test";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileSha256 } from "./mermaid-pair-manifest";
import { scoreNativeProductRows } from "./native-product-score";
import { MagickOps } from "./reference_image_ops";
import type { ReferencePair } from "./reference_pair_repository";

const GEOMETRY_SLUG = "08-01-c4-context-simple";
const WIDTH = 128;
const HEIGHT = 100;

interface Fixture {
  root: string;
  officialPath: string;
  pair: ReferencePair;
}

interface DiagramOptions {
  inverted?: boolean;
  text?: boolean;
  connector?: boolean;
  shiftX?: number;
  splitPalette?: "red-green" | "green-red" | "red-red";
  width?: number;
}

function drawDiagram(output: string, options: DiagramOptions = {}): void {
  const shiftX = options.shiftX ?? 0,
    background = options.inverted ? "#e1e1e1" : "#1e1e1e",
    panel = options.inverted ? "#1e1e1e" : "#e1e1e1",
    glyph = background,
    args = [
      "-size",
      `${options.width ?? WIDTH}x${HEIGHT}`,
      `xc:${background}`,
      "-fill",
      panel,
      "-draw",
      `rectangle ${16 + shiftX},20 ${112 + shiftX},68`,
    ];
  if (options.splitPalette !== undefined) {
    const left = options.splitPalette === "green-red" ? "#004b00" : "#ff0000",
      right = options.splitPalette === "red-green" ? "#004b00" : "#ff0000";
    args.push(
      "-fill",
      left,
      "-draw",
      `rectangle ${80 + shiftX},24 ${94 + shiftX},64`,
      "-fill",
      right,
      "-draw",
      `rectangle ${94 + shiftX},24 ${108 + shiftX},64`,
    );
  }
  if (options.text !== false) {
    args.push(
      "-stroke",
      glyph,
      "-strokewidth",
      "3",
      "-draw",
      [
        `line ${31 + shiftX},32 ${31 + shiftX},58`,
        `line ${31 + shiftX},45 ${45 + shiftX},32`,
        `line ${31 + shiftX},45 ${45 + shiftX},58`,
        `line ${55 + shiftX},32 ${69 + shiftX},32`,
        `line ${55 + shiftX},32 ${55 + shiftX},58`,
        `line ${69 + shiftX},32 ${69 + shiftX},44`,
        `line ${55 + shiftX},45 ${69 + shiftX},45`,
        `line ${55 + shiftX},45 ${69 + shiftX},58`,
      ].join(" "),
    );
  }
  if (options.connector !== false) {
    args.push(
      "-stroke",
      options.inverted ? "#101010" : "#f0f0f0",
      "-strokewidth",
      "3",
      "-draw",
      `line ${20 + shiftX},84 ${108 + shiftX},84`,
    );
  }
  args.push(output);
  execFileSync("magick", args);
}

function createFixture(slug = GEOMETRY_SLUG): Fixture {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "krr-native-product-edge-score-")),
    officialPath = path.join(root, "official.png");
  drawDiagram(officialPath);
  return {
    root,
    officialPath,
    pair: {
      slug,
      officialPath,
      katanaPath: officialPath,
      officialSvgPath: path.join(root, "official.svg"),
      katanaSvgPath: path.join(root, "candidate.svg"),
    },
  };
}

function withFixture<T>(action: (fixture: Fixture) => T, slug = GEOMETRY_SLUG): T {
  const fixture = createFixture(slug);
  try {
    return action(fixture);
  } finally {
    fs.rmSync(fixture.root, { recursive: true, force: true });
  }
}

function scoreFixture(fixture: Fixture, actualPng: string, runName: string) {
  const directory = path.join(fixture.root, runName),
    frame = { slug: fixture.pair.slug, png: { path: actualPng, sha256: fileSha256(actualPng) } },
    projected = {
      policy: "raw-svg-geometry-candidate-computed-paint-v1" as const,
      score: 100,
      structuralIntegrityAccepted: true,
      reasons: [],
      maxTransformDelta: 0,
      fullCanvasRmseScore: 100,
      fullCanvasMaeScore: 100,
      contentRmseScore: 100,
      contentMaeScore: 100,
      width: WIDTH,
      height: HEIGHT,
      evidenceSha256: "a".repeat(64),
    };
  return scoreNativeProductRows(
    [fixture.pair],
    [frame],
    new Map([[fixture.pair.slug, projected]]),
    new MagickOps(directory, null, "dark"),
    directory,
  )[0];
}

test("native C4 shape score uses actual PNG RGB edge support and keeps RGB separate", () =>
  withFixture((fixture) => {
    const recolored = path.join(fixture.root, "recolored.png");
    drawDiagram(recolored, { inverted: true });
    const row = scoreFixture(fixture, recolored, "palette-inversion");
    expect(row?.actualPngShape?.score).toBe(100);
    expect(row?.rgbScore).toBeLessThan(99);
    expect(row?.passed).toBe(true);
    expect(row?.actualPngShape?.policy).toBe("rgb-channel-edge-support-v1");
    expect(row?.scoreSemantics).toBe(
      "actual-png-rgb-channel-edge-support-v1+paint-projected-reference-v1",
    );
  }));

test("native C4 shape score rejects missing interior text despite projected score 100", () =>
  withFixture((fixture) => {
    const actual = path.join(fixture.root, "missing-text.png");
    drawDiagram(actual, { text: false });
    const row = scoreFixture(fixture, actual, "missing-text");
    expect(row?.paintProjectedReference?.score).toBe(100);
    expect(row?.actualPngShape?.score).toBeLessThan(99);
    expect(row?.score).toBeLessThan(99);
    expect(row?.passed).toBe(false);
  }));

test("native C4 shape score rejects missing connector and shifted geometry", () =>
  withFixture((fixture) => {
    const missingLine = path.join(fixture.root, "missing-line.png"),
      shifted = path.join(fixture.root, "shifted.png");
    drawDiagram(missingLine, { connector: false });
    drawDiagram(shifted, { shiftX: 3 });
    for (const row of [
      scoreFixture(fixture, missingLine, "missing-line"),
      scoreFixture(fixture, shifted, "shifted"),
    ]) {
      expect(row?.actualPngShape?.score).toBeLessThan(99);
      expect(row?.passed).toBe(false);
    }
  }));

test("native C4 edge support retains a quantized equal-luminance RGB boundary", () =>
  withFixture((fixture) => {
    const swapped = path.join(fixture.root, "equal-luminance-swapped.png"),
      removed = path.join(fixture.root, "equal-luminance-boundary-removed.png");
    const gray8 = (color: string) =>
      execFileSync(
        "magick",
        [`xc:${color}`, "-colorspace", "Gray", "-format", "%[fx:round(255*intensity)]", "info:"],
        { encoding: "utf8" },
      );
    expect(gray8("#ff0000")).toBe(gray8("#004b00"));
    drawDiagram(fixture.officialPath, { splitPalette: "red-green" });
    drawDiagram(swapped, { splitPalette: "green-red" });
    drawDiagram(removed, { splitPalette: "red-red" });
    expect(scoreFixture(fixture, swapped, "equal-luminance-swapped")?.actualPngShape?.score).toBe(
      100,
    );
    expect(
      scoreFixture(fixture, removed, "equal-luminance-boundary-removed")?.actualPngShape?.score,
    ).toBeLessThan(99);
  }));

test("native C4 edge support rejects actual PNG dimension mismatch", () =>
  withFixture((fixture) => {
    const wrongSize = path.join(fixture.root, "wrong-size.png");
    drawDiagram(wrongSize, { width: WIDTH - 1 });
    expect(() =>
      new MagickOps(
        path.join(fixture.root, "dimension-check"),
        null,
        "dark",
      ).compareRgbChannelEdgeSupportImages(
        fixture.officialPath,
        wrongSize,
        path.join(fixture.root, "dimension-check", "work"),
        fixture.pair.slug,
      ),
    ).toThrow("Actual PNG dimensions differ");
  }));

test("native product RGB score reads actual frame PNG and rejects missing frames", () =>
  withFixture((fixture) => {
    const browserPath = path.join(fixture.root, "browser.png"),
      nativePath = path.join(fixture.root, "native.png");
    drawDiagram(browserPath);
    drawDiagram(nativePath, { shiftX: 3 });
    const browserScore = new MagickOps(path.join(fixture.root, "browser-magick"), null, "dark");
    browserScore.prepare();
    expect(
      browserScore.comparePaintProjectedImages(
        fixture.officialPath,
        browserPath,
        path.join(fixture.root, "browser-magick", "work"),
        fixture.pair.slug,
      ).score,
    ).toBe(100);
    expect(scoreFixture(fixture, nativePath, "rgb-mismatch")?.passed).toBe(false);
    expect(scoreFixture(fixture, fixture.officialPath, "rgb-match")?.passed).toBe(true);
    expect(() =>
      scoreNativeProductRows(
        [fixture.pair],
        [],
        new Map(),
        new MagickOps(path.join(fixture.root, "missing"), null, "dark"),
        path.join(fixture.root, "missing"),
      ),
    ).toThrow("Native product frame inventory differs");
  }, "native-product-score-fixture"));
