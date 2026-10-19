import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { expect, test } from "playwright/test";
import { DiagramTheme } from "./diagram_theme";
import { fileSha256 } from "./mermaid-pair-manifest";
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

  test.beforeAll(async () => {
    const fixtureRoot = fs.mkdtempSync(path.join(os.tmpdir(), "krr-paint-projection-"));
    root = fixtureRoot;
    const fixture = await createPaintProjectionFixture(fixtureRoot),
      projected = await rasterizePaintProjectedReferenceBatch(
        [fixture.input],
        path.join(fixtureRoot, "projected"),
        "dark",
      ),
      neutral = await rasterizeNeutralSvgBatch(
        [fixture.neutralOfficial, fixture.neutralCandidate],
        path.join(fixtureRoot, "neutral"),
        "dark",
      ),
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

  test.afterAll(() => {
    try {
      if (observation !== undefined) {
        expect(observation.fixture.input.candidateSvgSha256).toBe(
          fileSha256(observation.fixture.input.candidateSvgPath),
        );
      }
    } finally {
      if (root !== undefined) fs.rmSync(root, { recursive: true, force: true });
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
});

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

async function createPaintProjectionFixture(root: string): Promise<PaintProjectionFixture> {
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
) {
  const playwright = await new PlaywrightLoader().load(),
    browser = await playwright.chromium.launch({ headless: true });
  try {
    const page = await browser.newPage({
      viewport: { width: 1520, height: 845 },
      deviceScaleFactor: 1,
    });
    try {
      const variants = [
        { svg: officialSvg, outputPath: officialPngPath, removeElement: null },
        { svg: candidateSvg, outputPath: candidatePngPath, removeElement: null },
        { svg: candidateSvg, outputPath: missingLabelPngPath, removeElement: "text" },
        { svg: candidateSvg, outputPath: missingEdgePngPath, removeElement: "line" },
      ] as const;
      for (const variant of variants) {
        await renderProjectionPng(page, variant.svg, variant.outputPath, variant.removeElement);
      }
    } finally {
      await page.close();
    }
  } finally {
    await browser.close();
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
