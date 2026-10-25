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
  type NativeFrameEvidence,
  nativeFrameCaptureWrapper,
  scoreNativeFramePaintProjection,
  verifyNativeFrameEvidence,
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
import { assignStrictSvg, resizeSvgCapture } from "./raster-capture-observation";
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

test("low-contrast C4 color changes pass the paint-aligned actual PNG gate", async ({
  browser,
}) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "krr-c4-low-contrast-control-"));
  const page = await browser.newPage({
    viewport: { width: 1520, height: 845 },
    deviceScaleFactor: 1,
  });
  try {
    const pair = await captureLowContrastC4Control(root, page);
    await expectLowContrastC4ProductScore(root, pair);
  } finally {
    await page.close();
    fs.rmSync(root, { recursive: true, force: true });
  }
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

async function expectLowContrastC4ProductScore(root: string, pair: ReferencePair) {
  const projectionDirectory = path.join(root, "projection");
  fs.mkdirSync(projectionDirectory);
  const projectionMagick = new MagickOps(path.join(root, "projection-metrics"), null, "dark");
  projectionMagick.prepare();
  const projected = await scorePaintProjectedReferenceTargets(
    [pair],
    projectionDirectory,
    "dark",
    projectionMagick,
  );
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
    const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), "krr-paint-projection-"));
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
    test(`native row.png forwarding preserves paint shape controls: ${variant.label}`, async () => {
      const currentObservation = requirePaintProjectionObservation(observation);
      const fixtureRoot = path.dirname(currentObservation.projectedDirectory);
      const bridgeRoot = path.join(fixtureRoot, variant.label);
      fs.mkdirSync(bridgeRoot, { recursive: true, mode: 0o700 });
      const bridge = await createNativeShapeBridgeFixture(
        currentObservation.fixture,
        variant.path(),
        bridgeRoot,
      );
      try {
        verifyNativeFrameEvidence(bridge.evidence, "en");
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

async function createNativeShapeBridgeFixture(
  fixture: PaintProjectionFixture,
  variantPngPath: string,
  root: string,
): Promise<{ root: string; evidence: NativeFrameEvidence; pairs: ReferencePair[] }> {
  const svgPath = fixture.input.candidateSvgPath;
  const svg = fs.readFileSync(svgPath, "utf8");
  const sourcePngPath = fixture.input.candidatePngPath;
  const size = fixture.input.candidatePngPath;
  const magick = new MagickOps(path.join(root, "bridge-size-magick"), null, "dark");
  const frameSize = magick.imageSize(size);
  const frame = { width: frameSize.width, height: frameSize.height, scale: 1, scrollY: 0 };
  const pin = (name: string, content: string | Buffer) => {
    const filePath = path.join(root, name);
    fs.writeFileSync(filePath, content, { flag: "wx", mode: 0o600 });
    return { path: filePath, sha256: fileSha256(filePath) };
  };
  // このピン群はrow.png転送だけを検査するcomponent fixtureであり、native capture receiptではない。
  const probeBinary = pin("probe", "probe"),
    runtime = pin("runtime", "runtime"),
    sourceManifest = pin("source-manifest", "manifest"),
    config = pin("config", "config"),
    fontContext = pin("font-context", "font");
  const rows = NATIVE_FRAME_TARGETS.map((slug) => {
    const outputDirectory = path.join(root, slug, "frames");
    fs.mkdirSync(outputDirectory, { recursive: true, mode: 0o700 });
    const wrapper = nativeFrameCaptureWrapper(svg, frame),
      wrapperHtml = pin(`${slug}-wrapper.html`, wrapper),
      outputPng = path.join(outputDirectory, `${frame.width}x${frame.height}-scale-1-scroll-0.png`);
    fs.copyFileSync(variantPngPath, outputPng);
    const png = { path: outputPng, sha256: fileSha256(outputPng) },
      stdout = pin(
        `${slug}-stdout`,
        `source_sha256=${wrapperHtml.sha256}\nwidth=${frame.width} height=${frame.height} scale=1 scroll_y=0\n`,
      ),
      stderr = pin(`${slug}-stderr`, ""),
      exitEvidence = pin(`${slug}-exit.json`, '{"exitCode":0,"signal":null,"error":null}\n');
    return {
      slug,
      inputSvg: { path: svgPath, sha256: fileSha256(svgPath) },
      sourcePng: { path: sourcePngPath, sha256: fileSha256(sourcePngPath) },
      wrapperHtml,
      png,
      stdout,
      stderr,
      exitEvidence,
      outputDirectory,
      frame,
      argv: [
        probeBinary.path,
        "--input",
        wrapperHtml.path,
        "--output",
        outputDirectory,
        "--width",
        String(frame.width),
        "--height",
        String(frame.height),
        "--scale",
        "1",
        "--scroll-y",
        "0",
      ],
      exitCode: 0 as const,
    };
  });
  return {
    root,
    evidence: {
      schema: "krr-mermaid-native-frame-evidence-v1",
      language: "en",
      probeBinary,
      runtime,
      sourceManifest,
      config,
      fontContext,
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
