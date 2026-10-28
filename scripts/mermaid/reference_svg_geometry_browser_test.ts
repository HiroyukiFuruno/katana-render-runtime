import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { chromium } from "playwright";
import { expect, test } from "playwright/test";
import { DiagramTheme } from "./diagram_theme";
import { parsePaintProjectedReferenceEvidence } from "./managed-full-compare-results";
import { fileSha256 } from "./mermaid-pair-manifest";
import {
  NATIVE_FRAME_TARGETS,
  type NativeFrameScoreInput,
  scoreNativeFramePaintProjection,
} from "./native-frame-evidence";
import { scoreNativeProductRows } from "./native-product-score";
import {
  type NeutralRasterInput,
  type PaintProjectedReferenceInput,
  rasterizeNeutralSvgBatch,
  rasterizePaintProjectedReferenceBatch,
} from "./neutral-svg-browser-rasterizer";
import { PlaywrightLoader } from "./official-renderer";
import type { FontReadyDocument, PageHandle } from "./official-renderer-types";
import {
  assignStrictSvg,
  prepareStrictSvgCapture,
  resizeSvgCapture,
} from "./raster-capture-observation";
import { rasterCaptureHtml } from "./rasterize-svg-dir";
import { MagickOps } from "./reference_image_ops";
import type { ReferencePair } from "./reference_pair_repository";
import { scorePaintProjectedReferenceTargets } from "./reference_svg_geometry_scoring";
import { waitForStrictSvgExternalImages } from "./strict-svg-external-images";

const naturalViewBoxSizingCases = [
  {
    relativePath: "tests/fixtures/mermaid/en/official-dark/08-01-c4-context-simple.svg",
    viewBox: [0, -70, 1148, 441],
  },
  {
    relativePath: "tests/fixtures/mermaid/en/official-dark/22-01-tree-view-simple.svg",
    viewBox: [-0.5, 0, 124.828125, 232],
  },
] as const;

test("SVG tspan boundaries preserve within-span kerning with a bundled font", async ({
  browser,
}) => {
  const fontFamily = "KrrBundledNotoKerningRegression";
  const fontBytes = fs.readFileSync(
    path.resolve("crates/katana-render-runtime/assets/fonts/NotoSans-Regular.ttf"),
  );
  const page = await browser.newPage({
    viewport: { width: 600, height: 200 },
    deviceScaleFactor: 1,
  });
  try {
    await page.setContent(`<!doctype html>
      <style>
        @font-face {
          font-family: "${fontFamily}";
          src: url(data:font/ttf;base64,${fontBytes.toString("base64")}) format("truetype");
        }
        svg text { font-family: "${fontFamily}"; font-size: 48px; font-kerning: normal; }
      </style>
      <svg xmlns="http://www.w3.org/2000/svg" width="600" height="200">
        <text id="full" y="55">AV</text>
        <text id="split" y="105"><tspan id="split-left">A</tspan><tspan id="split-right">V</tspan></text>
        <text id="single-a" y="155">A</text>
        <text id="single-v" y="155">V</text>
        <text id="preserved-full" y="55" xml:space="preserve"> AV </text>
        <text id="preserved-split" y="105" xml:space="preserve"><tspan id="preserved-left" xml:space="preserve"> A</tspan><tspan id="preserved-right" xml:space="preserve">V </tspan></text>
        <text id="preserved-a" y="155" xml:space="preserve"> A</text>
        <text id="preserved-v" y="155" xml:space="preserve">V </text>
      </svg>`);

    const measurements = await page.evaluate(async (family) => {
      const loadedFaces = await document.fonts.load(`48px "${family}"`, "AV");
      await document.fonts.ready;
      const measure = (id: string) => {
        const element = document.getElementById(id);
        if (!(element instanceof SVGTextContentElement)) {
          throw new Error(`Missing SVG text measurement element: ${id}`);
        }
        return element.getComputedTextLength();
      };
      return {
        fontLoaded: loadedFaces.some((face) => face.family === family && face.status === "loaded"),
        full: measure("full"),
        split: measure("split"),
        splitLeft: measure("split-left"),
        splitRight: measure("split-right"),
        singleA: measure("single-a"),
        singleV: measure("single-v"),
        preservedFull: measure("preserved-full"),
        preservedSplit: measure("preserved-split"),
        preservedLeft: measure("preserved-left"),
        preservedRight: measure("preserved-right"),
        preservedA: measure("preserved-a"),
        preservedV: measure("preserved-v"),
      };
    }, fontFamily);

    expect(measurements.fontLoaded).toBe(true);
    expect(measurements.split).toBeGreaterThan(measurements.full);
    expect(measurements.full).toBeLessThan(measurements.singleA + measurements.singleV);
    expect(
      Math.abs(measurements.split - measurements.splitLeft - measurements.splitRight),
    ).toBeLessThanOrEqual(0.02);
    expect(
      Math.abs(
        measurements.preservedSplit - measurements.preservedLeft - measurements.preservedRight,
      ),
    ).toBeLessThanOrEqual(0.02);
    expect(measurements.preservedLeft).toBeGreaterThan(measurements.singleA);
    expect(measurements.preservedRight).toBeGreaterThan(measurements.singleV);
    expect(measurements.preservedFull).toBeGreaterThan(measurements.full);
  } finally {
    await page.close();
  }
});

test.describe("low-contrast C4 color changes pass the paint-aligned actual PNG gate", () => {
  let root: string | undefined;
  let pair: ReferencePair | undefined;
  let projected: Awaited<ReturnType<typeof scorePaintProjectedReferenceTargets>> | undefined;

  test.beforeAll(async ({ browser }) => {
    const fixtureRoot = fs.realpathSync(
      fs.mkdtempSync(path.join(os.tmpdir(), "krr-c4-low-contrast-control-")),
    );
    root = fixtureRoot;
    const page = await browser.newPage({
      viewport: { width: 1520, height: 845 },
      deviceScaleFactor: 1,
    });
    let fixturePair: ReferencePair;
    try {
      fixturePair = await captureLowContrastC4Control(fixtureRoot, page);
      pair = fixturePair;
    } finally {
      await page.close();
    }
    const projectionDirectory = path.join(fixtureRoot, "projection");
    fs.mkdirSync(projectionDirectory);
    const projectionMagick = new MagickOps(
      path.join(fixtureRoot, "projection-metrics"),
      null,
      "dark",
    );
    projectionMagick.prepare();
    projected = await scorePaintProjectedReferenceTargets(
      [fixturePair],
      projectionDirectory,
      "dark",
      projectionMagick,
    );
  });

  test.afterAll(() => {
    if (root !== undefined) fs.rmSync(root, { recursive: true, force: true });
  });

  test("keeps RGB below 99 while actual PNG shape passes", () => {
    const fixtureRoot = root,
      fixturePair = pair,
      projection = projected;
    if (fixtureRoot === undefined || fixturePair === undefined || projection === undefined) {
      throw new Error("Low-contrast C4 fixture was not initialized");
    }
    expectLowContrastC4ProductScore(fixtureRoot, fixturePair, projection);
  });
});

async function captureLowContrastC4Control(root: string, page: PageHandle): Promise<ReferencePair> {
  const sourcePath = path.resolve("tests/fixtures/mermaid/en/official-dark/08-04-c4-component.svg");
  const storedSource = fs.readFileSync(sourcePath, "utf8");
  const source = storedSource.replaceAll("#e0e0e0", "#444444");
  const colored = source.replaceAll("#444444", "#e0e0e0");
  expect(colored).not.toBe(source);
  const pair = {
    slug: "08-04-c4-component",
    officialSvgPath: path.join(root, "official.svg"),
    officialPath: path.join(root, "official.png"),
    katanaSvgPath: path.join(root, "candidate.svg"),
    katanaPath: path.join(root, "actual.png"),
  };
  fs.writeFileSync(pair.officialSvgPath, source, { flag: "wx" });
  fs.writeFileSync(pair.katanaSvgPath, colored, { flag: "wx" });
  await renderAaColorControl(page, source, pair.officialPath);
  const originalGeometry = await readColorControlGeometry(page);
  await renderAaColorControl(page, colored, pair.katanaPath);
  expect(await readColorControlGeometry(page)).toEqual(originalGeometry);
  expect(fs.readFileSync(sourcePath, "utf8")).toBe(storedSource);
  return pair;
}

function readColorControlGeometry(page: PageHandle) {
  return page.evaluate(() =>
    Array.from(document.querySelectorAll("#diagram svg text"), (node) => {
      const text = node as SVGTextElement;
      const box = text.getBBox();
      const matrix = text.getScreenCTM();
      if (matrix === null) throw new Error("C4 control text has no screen transform");
      const style = getComputedStyle(text);
      return {
        text: text.textContent,
        box: [box.x, box.y, box.width, box.height],
        matrix: [matrix.a, matrix.b, matrix.c, matrix.d, matrix.e, matrix.f],
        font: [style.fontFamily, style.fontSize, style.fontWeight, style.fontStyle],
      };
    }),
  );
}

function expectLowContrastC4ProductScore(
  root: string,
  pair: ReferencePair,
  projected: Awaited<ReturnType<typeof scorePaintProjectedReferenceTargets>>,
) {
  const evidence = parsePaintProjectedReferenceEvidence(
    JSON.parse(fs.readFileSync(projected.evidencePath, "utf8")),
  );
  const expected = evidence.pairs[0];
  if (expected === undefined) throw new Error("C4 control produced no paint projection");
  const frame = {
    slug: pair.slug,
    png: { path: pair.katanaPath, sha256: fileSha256(pair.katanaPath) },
  };
  const bindings = new Map([
    [
      pair.slug,
      {
        projectedPng: {
          path: expected.projectedReference.pngPath,
          sha256: expected.projectedReference.pngSha256,
        },
        officialPngSha256: fileSha256(pair.officialPath),
        actualPngSha256: frame.png.sha256,
        evidenceSha256: projected.evidenceSha256,
      },
    ],
  ]);
  const directory = path.join(root, "native-score");
  const row = scoreNativeProductRows(
    [pair],
    [frame],
    projected.scores,
    bindings,
    new MagickOps(directory, null, "dark"),
    directory,
  ).scores[0];
  expect(row?.rgbScore).toBeLessThan(99);
  expect(row?.actualPngShape?.score).toBeGreaterThanOrEqual(99);
  expect(row?.passed).toBe(true);
}

test("strict raster sizing preserves SVG markup and resolves percentage width from the viewBox", async () => {
  const playwright = await new PlaywrightLoader().load(),
    browser = await playwright.chromium.launch({ headless: true }),
    page = await browser.newPage({
      viewport: { width: 1520, height: 845 },
      deviceScaleFactor: 1,
    });
  try {
    for (const fixture of naturalViewBoxSizingCases) {
      for (const mode of ["strictViewBox", "officialFallback"] as const) {
        await expectNaturalViewBoxSizing(page, fixture, mode);
      }
    }
    await expectOfficialFallbackWithoutViewBox(page);
  } finally {
    await page.close();
    await browser.close();
  }
});

test("strict capture prepares raw dimensions before layout across initial viewports", async ({
  browser,
}) => {
  const svg = fs.readFileSync(
    "tests/fixtures/mermaid/en/official-dark/08-02-c4-context-full.svg",
    "utf8",
  );
  const results = [];
  for (const viewport of [
    { width: 600, height: 300 },
    { width: 3200, height: 3200 },
  ]) {
    const page = await browser.newPage({ viewport, deviceScaleFactor: 1 });
    try {
      await page.setContent(rasterCaptureHtml(DiagramTheme.parse("dark")), { waitUntil: "load" });
      const preparedViewport = await prepareStrictSvgCapture(page, svg);
      expect(preparedViewport).toEqual({ width: 1724, height: 1863 });
      expect(page.viewportSize()).toEqual(preparedViewport);
      const initial = await page.evaluate(() => {
        const capture = document.getElementById("capture") as HTMLElement;
        const diagram = document.getElementById("diagram") as HTMLElement;
        return [
          capture.style.width,
          capture.style.height,
          diagram.style.width,
          diagram.style.height,
        ];
      });
      expect(initial).toEqual(["1724px", "1863px", "1700px", "1839px"]);
      await assignStrictSvg(page, svg);
      const initialText = await page.evaluate(async () => {
        await document.fonts.ready;
        const leaf = document.querySelector<SVGGraphicsElement>(
          "#diagram svg .c4-name text tspan.text-inner-tspan",
        );
        if (leaf === null) throw new Error("C4 text leaf is missing");
        leaf.getBBox();
        return leaf.textContent?.trim() ?? "";
      });
      expect(initialText.length).toBeGreaterThan(0);
      const cdp = await page.context().newCDPSession(page);
      await cdp.send("DOM.enable");
      await cdp.send("CSS.enable");
      const domDocument = await cdp.send("DOM.getDocument");
      const node = await cdp.send("DOM.querySelector", {
        nodeId: domDocument.root.nodeId,
        selector: "#diagram svg .c4-name text tspan.text-inner-tspan",
      });
      expect(node.nodeId).not.toBe(0);
      const first = await cdp.send("CSS.getPlatformFontsForNode", { nodeId: node.nodeId });
      await resizeSvgCapture(page, true, "strictViewBox");
      await page.evaluate(() => document.fonts.ready);
      const final = await cdp.send("CSS.getPlatformFontsForNode", { nodeId: node.nodeId });
      expect(final.fonts).toEqual(first.fonts);
      expect(final.fonts.length).toBeGreaterThan(0);
      const png = await page.locator("#capture").screenshot();
      results.push({ fonts: final.fonts, sha256: createHash("sha256").update(png).digest("hex") });
      await cdp.detach();
    } finally {
      await page.close();
    }
  }
  expect(results[0]).toEqual(results[1]);
});

test("strict capture rejects invalid viewBox before changing the capture", async ({ browser }) => {
  const page = await browser.newPage({
    viewport: { width: 600, height: 300 },
    deviceScaleFactor: 1,
  });
  try {
    await page.setContent(rasterCaptureHtml(DiagramTheme.parse("dark")), { waitUntil: "load" });
    const before = await page.evaluate(() => document.body.innerHTML);
    await expect(
      prepareStrictSvgCapture(page, '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 0 5"/>'),
    ).rejects.toThrow("SVG viewBox is invalid");
    expect(await page.evaluate(() => document.body.innerHTML)).toBe(before);
  } finally {
    await page.close();
  }
});

test("Chrome AA color-only changes preserve edge support while geometry shifts do not", async ({
  browser,
}) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "krr-chrome-aa-color-control-"));
  const page = await browser.newPage({
    viewport: { width: 160, height: 80 },
    deviceScaleFactor: 1,
  });
  try {
    await runAaColorControl(root, page);
  } finally {
    await page.close();
    fs.rmSync(root, { recursive: true, force: true });
  }
});

async function runAaColorControl(root: string, page: PageHandle) {
  const images = await captureAaColorControl(root, page),
    comparisonRoot = path.join(root, "comparison"),
    magick = new MagickOps(comparisonRoot, null, "dark");
  magick.prepare();
  const colorOnly = magick.compareRgbChannelEdgeSupportImages(
      images.official,
      images.recolored,
      comparisonRoot,
      "chrome-aa-color-only",
    ),
    geometryShift = magick.compareRgbChannelEdgeSupportImages(
      images.official,
      images.shifted,
      comparisonRoot,
      "chrome-aa-geometry-shift",
    );
  expect(colorOnly.score).toBeGreaterThanOrEqual(99);
  expect(
    magick.comparePaintProjectedImages(
      images.official,
      images.recolored,
      comparisonRoot,
      "chrome-aa-color-rgb",
    ).score,
  ).toBeLessThan(99);
  expect(geometryShift.score).toBeLessThan(99);
}

async function captureAaColorControl(root: string, page: PageHandle) {
  const images = {
    official: path.join(root, "official.png"),
    recolored: path.join(root, "recolored.png"),
    shifted: path.join(root, "shifted.png"),
  };
  await renderAaColorControl(page, aaColorControlSvg("#29415f", "#f2f4f8", 0), images.official);
  await renderAaColorControl(page, aaColorControlSvg("#8d3b5a", "#ffe29a", 0), images.recolored);
  await renderAaColorControl(page, aaColorControlSvg("#8d3b5a", "#ffe29a", 1), images.shifted);
  return images;
}

async function renderAaColorControl(page: PageHandle, svg: string, output: string) {
  await page.setContent(rasterCaptureHtml(DiagramTheme.parse("dark")), { waitUntil: "load" });
  await assignStrictSvg(page, svg);
  await resizeSvgCapture(page, true, "strictViewBox");
  await page.evaluate(() => (document as FontReadyDocument).fonts.ready);
  await page.locator("#capture").screenshot({ path: output, omitBackground: false });
}

function aaColorControlSvg(panel: string, text: string, shift: number): string {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="160" height="80" viewBox="0 0 160 80"><defs><filter id="shadow" x="-20%" y="-20%" width="140%" height="140%"><feDropShadow dx="2" dy="2" stdDeviation="1.5" flood-color="${text}" flood-opacity="0.45"/></filter></defs><rect width="160" height="80" fill="#111827"/><g transform="translate(${shift},0)" filter="url(#shadow)"><rect x="12" y="12" width="112" height="42" rx="6" fill="${panel}" stroke="${text}" stroke-width="2"/><line x1="18" y1="62" x2="118" y2="62" stroke="${text}" stroke-width="3"/><text x="20" y="38" fill="${text}" font-family="Arial, sans-serif" font-size="16">日本語 AA</text></g></svg>`;
}

async function expectNaturalViewBoxSizing(
  page: PageHandle,
  fixture: (typeof naturalViewBoxSizingCases)[number],
  mode: "strictViewBox" | "officialFallback",
) {
  const svg = fs.readFileSync(path.join(process.cwd(), fixture.relativePath), "utf8");
  await page.setContent(rasterCaptureHtml(DiagramTheme.parse("dark")), { waitUntil: "load" });
  const loaded = await assignStrictSvg(page, svg),
    before = await page.evaluate(() => {
      const element = document.querySelector("#diagram svg") as SVGSVGElement;
      return {
        outerHTML: element.outerHTML,
        width: element.getAttribute("width"),
        height: element.getAttribute("height"),
        viewBox: element.getAttribute("viewBox"),
      };
    });
  expect(loaded.assignedSvgSerialization).toBe(before.outerHTML);
  expect(before.width).toBe("100%");
  expect(before.height).toBeNull();
  expect(before.viewBox?.trim().split(/[ ,]+/).map(Number)).toEqual(Array.from(fixture.viewBox));

  await resizeSvgCapture(page, true, mode);
  await page.evaluate(() => (document as FontReadyDocument).fonts.ready);
  const geometry = await page.evaluate(() => {
    const capture = document.getElementById("capture") as HTMLElement,
      diagram = document.getElementById("diagram") as HTMLElement,
      svgElement = document.querySelector("#diagram svg") as SVGSVGElement,
      captureBox = capture.getBoundingClientRect(),
      diagramBox = diagram.getBoundingClientRect(),
      svgBox = svgElement.getBoundingClientRect();
    return {
      capture: { width: captureBox.width, height: captureBox.height },
      diagram: {
        x: diagramBox.x,
        y: diagramBox.y,
        width: diagramBox.width,
        height: diagramBox.height,
      },
      svg: { width: svgBox.width, height: svgBox.height },
      rootFontSize: getComputedStyle(svgElement).fontSize,
      width: svgElement.getAttribute("width"),
      height: svgElement.getAttribute("height"),
      viewBox: (svgElement.getAttribute("viewBox") ?? "").trim().split(/[ ,]+/).map(Number),
      outerHTML: svgElement.outerHTML,
    };
  });
  expect(geometry.capture).toEqual({
    width: Math.ceil(fixture.viewBox[2]) + 24,
    height: Math.ceil(fixture.viewBox[3]) + 24,
  });
  expect(geometry.diagram.width).toBe(fixture.viewBox[2]);
  expect(geometry.diagram.height).toBe(fixture.viewBox[3]);
  expect(geometry.svg).toEqual({ width: fixture.viewBox[2], height: fixture.viewBox[3] });
  expect(geometry.rootFontSize).toBe("16px");
  expect(geometry.width).toBe(before.width);
  expect(geometry.height).toBe(before.height);
  expect(geometry.viewBox).toEqual(Array.from(fixture.viewBox));
  expect(geometry.outerHTML).toBe(before.outerHTML);
}

async function expectOfficialFallbackWithoutViewBox(page: PageHandle) {
  const svg =
    '<svg xmlns="http://www.w3.org/2000/svg" width="160" height="80"><rect width="160" height="80"/></svg>';
  await page.setContent(rasterCaptureHtml(DiagramTheme.parse("dark")), { waitUntil: "load" });
  const loaded = await assignStrictSvg(page, svg);
  await resizeSvgCapture(page, true, "officialFallback");
  const result = await page.evaluate(() => {
    const element = document.querySelector("#diagram svg") as SVGSVGElement,
      diagram = document.getElementById("diagram") as HTMLElement;
    return {
      outerHTML: element.outerHTML,
      width: element.getAttribute("width"),
      height: element.getAttribute("height"),
      diagramStyleWidth: diagram.style.width,
      diagramStyleHeight: diagram.style.height,
    };
  });
  expect(loaded.assignedSvgSerialization).toBe(result.outerHTML);
  expect(result.width).toBe("160");
  expect(result.height).toBe("80");
  expect(result.diagramStyleWidth).toBe("");
  expect(result.diagramStyleHeight).toBe("");
}

interface PaintProjectionScoreCase {
  title: string;
  label: string;
  pngPath: (fixture: PaintProjectionFixture) => string;
  passes: boolean;
}

const paintProjectionScoreCases: readonly PaintProjectionScoreCase[] = [
  {
    title: "keeps the color-only candidate exact while changing RGB and preserving SVG bytes",
    label: "color-only",
    pngPath: (fixture) => fixture.input.candidatePngPath,
    passes: true,
  },
  {
    title: "rejects a candidate with a missing label",
    label: "missing-label",
    pngPath: (fixture) => fixture.missingLabelPngPath,
    passes: false,
  },
  {
    title: "rejects a candidate with a missing edge",
    label: "missing-edge",
    pngPath: (fixture) => fixture.missingEdgePngPath,
    passes: false,
  },
];

test.describe("paint projection evaluates actual candidate pixels", () => {
  let root: string | undefined;
  let observation: PaintProjectionObservation | undefined;
  let fixturePage: PageHandle | undefined;

  test.beforeAll(async ({ browser }) => {
    const fixtureRoot = fs.realpathSync(
      fs.mkdtempSync(path.join(os.tmpdir(), "krr-paint-projection-")),
    );
    root = fixtureRoot;
    fixturePage = await browser.newPage({
      viewport: { width: 1520, height: 845 },
      deviceScaleFactor: 1,
    });
    const fixture = await createPaintProjectionFixture(fixtureRoot, fixturePage),
      [projectedResult, neutralResult] = await Promise.allSettled([
        rasterizePaintProjectedReferenceBatch(
          [fixture.input],
          path.join(fixtureRoot, "projected"),
          "dark",
        ),
        rasterizeNeutralSvgBatch(
          [fixture.neutralOfficial, fixture.neutralCandidate],
          path.join(fixtureRoot, "neutral"),
          "dark",
        ),
      ] as const);
    if (projectedResult.status === "rejected" && neutralResult.status === "rejected") {
      throw new AggregateError(
        [projectedResult.reason, neutralResult.reason],
        "Paint projection raster batches failed",
      );
    }
    if (projectedResult.status === "rejected") throw projectedResult.reason;
    if (neutralResult.status === "rejected") throw neutralResult.reason;
    const projected = projectedResult.value,
      neutral = neutralResult.value,
      magick = new MagickOps(path.join(fixtureRoot, "magick"), null, "dark"),
      projectedRecord = projected.records[0],
      neutralOfficial = neutral.records.find((record) => record.role === "official"),
      neutralCandidate = neutral.records.find((record) => record.role === "candidate");
    if (
      projectedRecord === undefined ||
      neutralOfficial === undefined ||
      neutralCandidate === undefined
    ) {
      throw new Error("Paint projection fixture did not produce every expected raster");
    }
    const identityServer = await chromium.launchServer({ headless: true });
    try {
      const spawnedExecutable = identityServer.process().spawnfile;
      if (spawnedExecutable === undefined || !path.isAbsolute(spawnedExecutable)) {
        throw new Error("Launched Chromium process executable path is unavailable");
      }
      const actualExecutable = fs.realpathSync(spawnedExecutable);
      expect(projected.browserExecutablePath).toBe(actualExecutable);
      expect(neutral.browserExecutablePath).toBe(actualExecutable);
      expect(projected.browserExecutableSha256).toBe(fileSha256(actualExecutable));
      expect(neutral.browserExecutableSha256).toBe(fileSha256(actualExecutable));
    } finally {
      await identityServer.close();
    }
    observation = {
      fixture,
      projectedReferencePngPath: projectedRecord.projectedReferencePngPath,
      neutralOfficialPngPath: neutralOfficial.neutralPngPath,
      neutralCandidatePngPath: neutralCandidate.neutralPngPath,
      magick,
      projectedDirectory: path.join(fixtureRoot, "projected"),
      neutralDirectory: path.join(fixtureRoot, "neutral"),
    };
  });

  test.afterAll(async () => {
    try {
      if (observation !== undefined) {
        expect(observation.fixture.input.candidateSvgSha256).toBe(
          fileSha256(observation.fixture.input.candidateSvgPath),
        );
      }
    } finally {
      try {
        await fixturePage?.close();
      } finally {
        if (root !== undefined) fs.rmSync(root, { recursive: true, force: true });
      }
    }
  });

  for (const item of paintProjectionScoreCases) {
    test(item.title, () => {
      const result = requirePaintProjectionObservation(observation);
      const score = result.magick.comparePaintProjectedImages(
        result.projectedReferencePngPath,
        item.pngPath(result.fixture),
        result.projectedDirectory,
        item.label,
      ).score;
      if (item.passes) {
        expect(score).toBe(100);
        expect(
          result.magick.compareNormalizedError(
            "RMSE",
            result.fixture.input.officialPngPath,
            result.fixture.input.candidatePngPath,
          ),
        ).toBeGreaterThan(0);
      } else {
        expect(score).toBeLessThan(99);
      }
    });
  }

  test("keeps the neutral geometry raster exact", () => {
    const result = requirePaintProjectionObservation(observation);
    expect(
      result.magick.compareGeometryImages(
        result.neutralOfficialPngPath,
        result.neutralCandidatePngPath,
        result.neutralDirectory,
        result.fixture.input.slug,
      ).fullCanvasScore,
    ).toBe(100);
  });
  for (const variant of [
    {
      label: "candidate",
      path: () => requirePaintProjectionObservation(observation).fixture.input.candidatePngPath,
      passes: true,
    },
    {
      label: "missing-label",
      path: () => requirePaintProjectionObservation(observation).fixture.missingLabelPngPath,
      passes: false,
    },
    {
      label: "missing-edge",
      path: () => requirePaintProjectionObservation(observation).fixture.missingEdgePngPath,
      passes: false,
    },
  ] as const) {
    test(`component scorer forwards frame PNG and preserves paint shape controls: ${variant.label}`, async () => {
      const currentObservation = requirePaintProjectionObservation(observation);
      const fixtureRoot = path.dirname(currentObservation.projectedDirectory);
      const bridgeRoot = path.join(fixtureRoot, variant.label);
      fs.mkdirSync(bridgeRoot, { recursive: true, mode: 0o700 });
      const bridge = createNativeShapeBridgeFixture(currentObservation.fixture, variant.path());
      try {
        const result = await scoreNativeFramePaintProjection(
          bridge.pairs,
          bridge.evidence,
          bridgeRoot,
          "dark",
          new MagickOps(path.join(fixtureRoot, "bridge-magick"), null, "dark"),
        );
        expect(result.scores.size).toBe(NATIVE_FRAME_TARGETS.length);
        for (const score of result.scores.values()) {
          if (variant.passes) expect(score.score).toBe(100);
          else expect(score.score).toBeLessThan(99);
        }
      } finally {
        fs.rmSync(bridgeRoot, { recursive: true, force: true });
      }
    });
  }
});

function createNativeShapeBridgeFixture(
  fixture: PaintProjectionFixture,
  variantPngPath: string,
): { evidence: NativeFrameScoreInput; pairs: ReferencePair[] } {
  const svgPath = fixture.input.candidateSvgPath;
  const sourcePngPath = fixture.input.candidatePngPath;
  const rows = NATIVE_FRAME_TARGETS.map((slug) => ({
    slug,
    inputSvg: { path: svgPath, sha256: fileSha256(svgPath) },
    sourcePng: { path: sourcePngPath, sha256: fileSha256(sourcePngPath) },
    png: { path: variantPngPath, sha256: fileSha256(variantPngPath) },
  }));
  return {
    evidence: {
      language: "en",
      rows,
    },
    pairs: NATIVE_FRAME_TARGETS.map((slug) => ({
      slug,
      officialPath: fixture.input.officialPngPath,
      katanaPath: fixture.input.candidatePngPath,
      officialSvgPath: fixture.input.officialSvgPath,
      katanaSvgPath: svgPath,
    })),
  };
}

interface PaintProjectionObservation {
  fixture: PaintProjectionFixture;
  projectedReferencePngPath: string;
  neutralOfficialPngPath: string;
  neutralCandidatePngPath: string;
  magick: MagickOps;
  projectedDirectory: string;
  neutralDirectory: string;
}

function requirePaintProjectionObservation(
  observation: PaintProjectionObservation | undefined,
): PaintProjectionObservation {
  if (observation === undefined) throw new Error("Paint projection fixture was not initialized");
  return observation;
}

const paintProjectionIdMapCases = [
  {
    name: "same mapping",
    candidateIds: ["candidate-root", "shared", "shared"] as const,
    officialIds: ["official-root", "target-shared", "target-shared"] as const,
    rejected: false,
  },
  {
    name: "source maps to different targets",
    candidateIds: ["candidate-root", "shared", "shared"] as const,
    officialIds: ["official-root", "target-a", "target-b"] as const,
    rejected: true,
  },
  {
    name: "targets map from different sources",
    candidateIds: ["candidate-root", "source-a", "source-b"] as const,
    officialIds: ["official-root", "target-shared", "target-shared"] as const,
    rejected: true,
  },
];

for (const item of paintProjectionIdMapCases) {
  test(`paint projection allows repeated IDs only for one-to-one repeated mappings: ${item.name}`, async () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "krr-paint-id-map-"));
    try {
      const input = createPaintProjectionInput(
        root,
        paintProjectionSvgWithIds(item.officialIds, "#444444"),
        paintProjectionSvgWithIds(item.candidateIds, "#4682d8"),
      );
      const run = rasterizePaintProjectedReferenceBatch(
        [input],
        path.join(root, "projected"),
        "dark",
      );
      if (item.rejected) {
        await expect(run).rejects.toThrow("Paint projection cannot map an SVG reference ID");
      } else {
        await expect(run).resolves.toMatchObject({ records: [{ slug: input.slug }] });
      }
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });
}

interface PaintProjectionFixture {
  input: PaintProjectedReferenceInput;
  neutralOfficial: NeutralRasterInput;
  neutralCandidate: NeutralRasterInput;
  missingLabelPngPath: string;
  missingEdgePngPath: string;
}

async function createPaintProjectionFixture(
  root: string,
  page: PageHandle,
): Promise<PaintProjectionFixture> {
  const officialSvg = paintProjectionSvg("#444444", "#222222"),
    candidateSvg = paintProjectionSvg("#4682d8", "#e8edf4"),
    paths = paintProjectionPaths(root),
    screenshots = await renderPaintProjectionFixture(
      officialSvg,
      candidateSvg,
      paths.officialPngPath,
      paths.candidatePngPath,
      paths.missingLabelPngPath,
      paths.missingEdgePngPath,
      page,
    );
  fs.writeFileSync(paths.officialSvgPath, officialSvg, { flag: "wx" });
  fs.writeFileSync(paths.candidateSvgPath, candidateSvg, { flag: "wx" });
  return projectionFixture(paths, officialSvg, candidateSvg, screenshots);
}

function createPaintProjectionInput(
  root: string,
  officialSvg: string,
  candidateSvg: string,
): PaintProjectedReferenceInput {
  const paths = paintProjectionPaths(root),
    png = Buffer.from(
      "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=",
      "base64",
    );
  for (const outputPath of [
    paths.officialPngPath,
    paths.candidatePngPath,
    paths.missingLabelPngPath,
    paths.missingEdgePngPath,
  ])
    fs.writeFileSync(outputPath, png, { flag: "wx" });
  fs.writeFileSync(paths.officialSvgPath, officialSvg, { flag: "wx" });
  fs.writeFileSync(paths.candidateSvgPath, candidateSvg, { flag: "wx" });
  return projectionFixture(paths, officialSvg, candidateSvg, {
    officialPngSha256: fileSha256(paths.officialPngPath),
    candidatePngSha256: fileSha256(paths.candidatePngPath),
  }).input;
}

function paintProjectionSvg(fill: string, text: string): string {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="160" height="80" viewBox="0 0 160 80"><rect x="4" y="4" width="152" height="72" rx="4" fill="${fill}" stroke="${fill}"/><line x1="20" y1="20" x2="140" y2="60" stroke="${text}" stroke-width="3"/><text x="10" y="45" fill="${text}" font-family="Arial, sans-serif" font-size="18">DELETE LABEL TEXT</text></svg>`;
}

function paintProjectionSvgWithIds(ids: readonly [string, string, string], fill: string): string {
  return `<svg xmlns="http://www.w3.org/2000/svg" id="${ids[0]}" width="160" height="80" viewBox="0 0 160 80"><g id="${ids[1]}" fill="${fill}"><rect id="${ids[2]}" x="4" y="4" width="152" height="72" fill="${fill}"/><text x="10" y="45" fill="#222" font-family="Arial, sans-serif" font-size="18">ID MAP</text></g></svg>`;
}

function paintProjectionPaths(root: string) {
  const slug = "08-04-c4-component";
  return {
    slug,
    officialSvgPath: path.join(root, "official.svg"),
    candidateSvgPath: path.join(root, "candidate.svg"),
    officialPngPath: path.join(root, "official.png"),
    candidatePngPath: path.join(root, "candidate.png"),
    missingLabelPngPath: path.join(root, "candidate-missing-label.png"),
    missingEdgePngPath: path.join(root, "candidate-missing-edge.png"),
  };
}

async function renderPaintProjectionFixture(
  officialSvg: string,
  candidateSvg: string,
  officialPngPath: string,
  candidatePngPath: string,
  missingLabelPngPath: string,
  missingEdgePngPath: string,
  page: PageHandle,
) {
  const variants = [
    { svg: officialSvg, outputPath: officialPngPath, removeElement: null },
    { svg: candidateSvg, outputPath: candidatePngPath, removeElement: null },
    { svg: candidateSvg, outputPath: missingLabelPngPath, removeElement: "text" },
    { svg: candidateSvg, outputPath: missingEdgePngPath, removeElement: "line" },
  ] as const;
  for (const variant of variants) {
    await renderProjectionPng(page, variant.svg, variant.outputPath, variant.removeElement);
  }
  return {
    officialPngSha256: fileSha256(officialPngPath),
    candidatePngSha256: fileSha256(candidatePngPath),
  };
}

async function renderProjectionPng(
  page: PageHandle,
  svg: string,
  outputPath: string,
  removeElement: "text" | "line" | null,
) {
  await page.setContent(rasterCaptureHtml(DiagramTheme.parse("dark")), { waitUntil: "load" });
  await assignStrictSvg(page, svg);
  await resizeSvgCapture(page, true, "strictViewBox");
  await waitForStrictSvgExternalImages(page);
  await page.evaluate(() => (document as FontReadyDocument).fonts.ready);
  if (removeElement !== null) {
    await page.evaluate((selector: "text" | "line") => {
      const element = document.querySelector(`#diagram svg ${selector}`);
      if (element === null) throw new Error(`Fixture SVG ${selector} is missing`);
      element.remove();
    }, removeElement);
  }
  await page.locator("#capture").screenshot({ path: outputPath, omitBackground: false });
}

function projectionFixture(
  paths: ReturnType<typeof paintProjectionPaths>,
  officialSvg: string,
  candidateSvg: string,
  screenshots: { officialPngSha256: string; candidatePngSha256: string },
): PaintProjectionFixture {
  const officialSvgSha256 = hashText(officialSvg),
    candidateSvgSha256 = hashText(candidateSvg),
    input: PaintProjectedReferenceInput = {
      slug: paths.slug,
      officialSvgPath: paths.officialSvgPath,
      officialSvg,
      officialSvgSha256,
      officialPngPath: paths.officialPngPath,
      officialPngSha256: screenshots.officialPngSha256,
      candidateSvgPath: paths.candidateSvgPath,
      candidateSvg,
      candidateSvgSha256,
      candidatePngPath: paths.candidatePngPath,
      candidatePngSha256: screenshots.candidatePngSha256,
      outputSvgPath: path.join(
        path.dirname(paths.officialSvgPath),
        "projected",
        `${paths.slug}-reference.svg`,
      ),
      outputPngPath: path.join(
        path.dirname(paths.officialSvgPath),
        "projected",
        `${paths.slug}-reference.png`,
      ),
    };
  return {
    input,
    neutralOfficial: neutralFixture(input, "official", paths.officialSvgPath, officialSvg),
    neutralCandidate: neutralFixture(input, "candidate", paths.candidateSvgPath, candidateSvg),
    missingLabelPngPath: paths.missingLabelPngPath,
    missingEdgePngPath: paths.missingEdgePngPath,
  };
}

function neutralFixture(
  input: PaintProjectedReferenceInput,
  role: "official" | "candidate",
  svgPath: string,
  svg: string,
) {
  const sourcePngPath = role === "official" ? input.officialPngPath : input.candidatePngPath,
    sourcePngSha256 = role === "official" ? input.officialPngSha256 : input.candidatePngSha256;
  return {
    slug: input.slug,
    role,
    svgPath,
    svg,
    svgSha256: role === "official" ? input.officialSvgSha256 : input.candidateSvgSha256,
    sourcePngPath,
    sourcePngSha256,
    outputSvgPath: path.join(
      path.dirname(input.officialSvgPath),
      "neutral",
      `${role}-${input.slug}.svg`,
    ),
    outputPngPath: path.join(
      path.dirname(input.officialSvgPath),
      "neutral",
      `${role}-${input.slug}.png`,
    ),
  };
}

function hashText(value: string): string {
  return createHash("sha256").update(value).digest("hex");
}

test("paint projection isolates candidate font context from official rasterization", async ({
  browser,
}) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "krr-paint-font-isolation-")),
    candidatePage = await browser.newPage({
      viewport: { width: 1520, height: 845 },
      deviceScaleFactor: 1,
    });
  try {
    const input = await createFontIsolationInput(root, candidatePage, browser),
      projected = await rasterizePaintProjectedReferenceBatch(
        [input],
        input.outputDirectory,
        "dark",
      ),
      record = projected.records[0];
    if (record === undefined) throw new Error("Font isolation raster was not produced");
    const projectedSvg = fs.readFileSync(record.projectedReferenceSvgPath, "utf8");
    expect(input.candidateSvg).toContain('font-family="Courier New, monospace"');
    expect(projectedSvg).toContain('font-family="Arial, sans-serif"');
    expect(projectedSvg).not.toContain("Courier New, monospace");
    expect(record.officialCaptureViewport).toEqual({
      width: record.width,
      height: record.height,
      deviceScaleFactor: 1,
    });
    expect(record.candidateCaptureViewport.deviceScaleFactor).toBe(1);
    expect(record.officialPaintVisibilitySha256).toBe(record.paintVisibilitySha256);
    expect(record.projectedReferencePngSha256).toBe(input.officialPngSha256);
    expect(fs.readFileSync(record.projectedReferencePngPath)).toEqual(
      fs.readFileSync(input.officialPngPath),
    );
  } finally {
    await candidatePage.close();
    fs.rmSync(root, { recursive: true, force: true });
  }
});

interface FontIsolationInput extends PaintProjectedReferenceInput {
  outputDirectory: string;
}

async function createFontIsolationInput(
  root: string,
  candidatePage: PageHandle,
  browser: import("playwright").Browser,
): Promise<FontIsolationInput> {
  const slug = "paint-font-isolation",
    officialSvg = paintProjectionSvg("#444444", "#222222").replace(
      "DELETE LABEL TEXT",
      "OFFICIAL FONT TEXT",
    ),
    candidateSvg = paintProjectionSvg("#444444", "#222222")
      .replace("Arial, sans-serif", "Courier New, monospace")
      .replace("DELETE LABEL TEXT", "CANDIDATE FONT TEXT"),
    officialSvgPath = path.join(root, "official.svg"),
    candidateSvgPath = path.join(root, "candidate.svg"),
    officialPngPath = path.join(root, "official.png"),
    candidatePngPath = path.join(root, "candidate.png"),
    outputDirectory = path.join(root, "projected");
  fs.writeFileSync(officialSvgPath, officialSvg, { flag: "wx" });
  fs.writeFileSync(candidateSvgPath, candidateSvg, { flag: "wx" });
  await renderProjectionPng(candidatePage, candidateSvg, candidatePngPath, null);
  await renderFreshOfficialPng(browser, officialSvg, officialPngPath);
  return {
    slug,
    officialSvgPath,
    officialSvg,
    officialSvgSha256: fileSha256(officialSvgPath),
    officialPngPath,
    officialPngSha256: fileSha256(officialPngPath),
    candidateSvgPath,
    candidateSvg,
    candidateSvgSha256: fileSha256(candidateSvgPath),
    candidatePngPath,
    candidatePngSha256: fileSha256(candidatePngPath),
    outputSvgPath: path.join(outputDirectory, `${slug}-reference.svg`),
    outputPngPath: path.join(outputDirectory, `${slug}-reference.png`),
    outputDirectory,
  };
}

async function renderFreshOfficialPng(
  browser: import("playwright").Browser,
  officialSvg: string,
  outputPath: string,
) {
  const page = await browser.newPage({
    viewport: { width: 1520, height: 845 },
    deviceScaleFactor: 1,
  });
  try {
    await page.setContent(rasterCaptureHtml(DiagramTheme.parse("dark")), { waitUntil: "load" });
    await prepareStrictSvgCapture(page, officialSvg);
    await assignStrictSvg(page, officialSvg);
    await resizeSvgCapture(page, true, "strictViewBox");
    await waitForStrictSvgExternalImages(page);
    await page.evaluate(() => (document as FontReadyDocument).fonts.ready);
    await page.locator("#capture").screenshot({ path: outputPath, omitBackground: false });
  } finally {
    await page.close();
  }
}
