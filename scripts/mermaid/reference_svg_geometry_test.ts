import { expect, test } from "bun:test";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
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
import {
  assertCompletePaintProjectedTargetSlugs,
  compareSvgGeometry,
  compareSvgGeometryForNeutralRaster,
  isNeutralGeometryTargetSlug,
  NEUTRAL_GEOMETRY_TARGET_SLUGS,
  PAINT_PROJECTED_REFERENCE_TARGET_SLUGS,
  PAINT_PROJECTED_REFERENCE_TARGET_SLUGS_JA,
  prepareNeutralSvg,
} from "./reference_svg_geometry";
import { waitForStrictSvgExternalImages } from "./strict-svg-external-images";

const sourceSvg = (rootId: string, fill: string, shift: string, label = "Box") =>
  `<svg xmlns="http://www.w3.org/2000/svg" id="${rootId}" width="120px" height="80px" viewBox="0 0 120 80"><style>#${rootId} .box{fill:${fill};stroke:#222;stroke-width:2px} #${rootId} .label{fill:${fill};font-size:16px}</style><g transform="translate(0,${shift})"><rect class="box" x="10" y="10" width="50" height="30"/><line x1="10" y1="40" x2="60" y2="40" stroke="#444"/><text class="label" x="12" y="25">${label}<tspan> text</tspan></text></g></svg>`;

test("neutral geometry scope is the fixed current C4 and TreeView fixture family", () => {
  expect(NEUTRAL_GEOMETRY_TARGET_SLUGS).toHaveLength(8);
  expect(NEUTRAL_GEOMETRY_TARGET_SLUGS.every(isNeutralGeometryTargetSlug)).toBe(true);
  expect(PAINT_PROJECTED_REFERENCE_TARGET_SLUGS_JA).toHaveLength(8);
  expect(PAINT_PROJECTED_REFERENCE_TARGET_SLUGS_JA.every(isNeutralGeometryTargetSlug)).toBe(true);
  expect(isNeutralGeometryTargetSlug("11-c4")).toBe(false);
  expect(isNeutralGeometryTargetSlug("08-04-c4x")).toBe(false);
  expect(isNeutralGeometryTargetSlug("custom-tree-view-prototype")).toBe(false);
});

test.each([
  ...PAINT_PROJECTED_REFERENCE_TARGET_SLUGS,
  ...PAINT_PROJECTED_REFERENCE_TARGET_SLUGS_JA,
])("paint projection recognizes the exact language target slug: %s", (slug) => {
  expect(isNeutralGeometryTargetSlug(slug)).toBe(true);
});

test.each([
  { pairCount: 59, slugs: PAINT_PROJECTED_REFERENCE_TARGET_SLUGS },
  { pairCount: 58, slugs: PAINT_PROJECTED_REFERENCE_TARGET_SLUGS_JA },
])(
  "accepts the complete target set for the language pair count: $pairCount",
  ({ pairCount, slugs }) => {
    expect(() => assertCompletePaintProjectedTargetSlugs(pairCount, slugs)).not.toThrow();
  },
);

test.each([
  { name: "missing target", slugs: PAINT_PROJECTED_REFERENCE_TARGET_SLUGS.slice(0, 7) },
  {
    name: "duplicate target",
    slugs: [
      ...PAINT_PROJECTED_REFERENCE_TARGET_SLUGS.slice(0, 7),
      PAINT_PROJECTED_REFERENCE_TARGET_SLUGS[0],
    ],
  },
  { name: "wrong language", slugs: PAINT_PROJECTED_REFERENCE_TARGET_SLUGS_JA },
])("rejects an incomplete or wrong target set: $name", ({ slugs }) => {
  expect(() => assertCompletePaintProjectedTargetSlugs(59, slugs)).toThrow();
});

test("rejects duplicate targets even for a scoped pair count", () => {
  expect(() =>
    assertCompletePaintProjectedTargetSlugs(2, ["scoped-target", "scoped-target"]),
  ).toThrow("Paint projection target slugs are duplicated");
});

test("neutral SVG geometry accepts palette-only changes and equivalent root IDs", () => {
  const left = prepareNeutralSvg(
      sourceSvg("a", "#444444", "0").replace("</svg>", '<metadata data-color="#abcdef"/></svg>'),
    ),
    right = prepareNeutralSvg(
      sourceSvg("b", "#e0e0e0", "0").replace("</svg>", '<metadata data-color="#abcdef"/></svg>'),
    );
  expect(compareSvgGeometry(left, right)).toMatchObject({ accepted: true, maxTransformDelta: 0 });
});

test("neutral SVG geometry preserves paint alpha instead of hiding transparency", () => {
  const opaque = prepareNeutralSvg(sourceSvg("official", "rgba(20, 30, 40, 1)", "0"));
  const transparent = prepareNeutralSvg(sourceSvg("candidate", "rgba(200, 210, 220, 0)", "0"));
  const partial = prepareNeutralSvg(sourceSvg("candidate", "rgba(200, 210, 220, 0.5)", "0"));
  const sameTransparency = prepareNeutralSvg(sourceSvg("candidate", "rgba(200, 210, 220, 0)", "0"));
  expect(compareSvgGeometry(opaque, transparent).accepted).toBe(false);
  expect(compareSvgGeometry(opaque, partial).accepted).toBe(false);
  expect(compareSvgGeometry(transparent, sameTransparency).accepted).toBe(true);
});

test("neutral SVG geometry preserves gradient references and stop alpha", () => {
  const source = (gradientId: string, color: string) =>
    `<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><linearGradient id="${gradientId}"><stop offset="0" stop-color="${color}"/></linearGradient></defs><rect x="1" y="1" width="18" height="18" fill="url(#${gradientId})"/></svg>`;
  const official = prepareNeutralSvg(source("official-gradient", "rgba(10,20,30,0.5)"));
  const recolored = prepareNeutralSvg(source("candidate-gradient", "rgba(220,210,200,0.5)"));
  const transparent = prepareNeutralSvg(source("candidate-gradient", "rgba(220,210,200,0)"));
  expect(compareSvgGeometry(official, recolored).accepted).toBe(true);
  expect(compareSvgGeometry(official, transparent).accepted).toBe(false);
});

test("neutral raster structure ignores paint declarations but distinguishes reference resolution", () => {
  const withoutDirectPaint = prepareNeutralSvg(
      '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><rect x="1" y="1" width="18" height="18"/></svg>',
    ),
    withDirectPaint = prepareNeutralSvg(
      '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><rect x="1" y="1" width="18" height="18" fill="#222" stroke="none"/></svg>',
    );
  expect(compareSvgGeometry(withoutDirectPaint, withDirectPaint).accepted).toBe(false);
  expect(compareSvgGeometryForNeutralRaster(withoutDirectPaint, withDirectPaint).accepted).toBe(
    true,
  );
  const unresolved = prepareNeutralSvg(
      '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><rect x="1" y="1" width="18" height="18" fill="url(#missing)"/></svg>',
    ),
    unresolvedWithDifferentId = prepareNeutralSvg(
      '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><rect x="1" y="1" width="18" height="18" fill="url(#also-missing)"/></svg>',
    ),
    resolved = prepareNeutralSvg(
      '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><linearGradient id="present"><stop offset="0" stop-color="#000"/></linearGradient></defs><rect x="1" y="1" width="18" height="18" fill="url(#present)"/></svg>',
    );
  expect(compareSvgGeometry(unresolved, unresolvedWithDifferentId).accepted).toBe(true);
  expect(compareSvgGeometry(unresolved, resolved).accepted).toBe(false);
});

test("neutral SVG geometry rejects missing boxes, connectors, labels, and text changes", () => {
  const original = prepareNeutralSvg(sourceSvg("official", "#444444", "0"));
  const removedBox = prepareNeutralSvg(
    sourceSvg("official", "#444444", "0").replace(/<rect[^>]*\/>/, ""),
  );
  const removedConnector = prepareNeutralSvg(
    sourceSvg("official", "#444444", "0").replace(/<line[^>]*\/>/, ""),
  );
  const removedLabel = prepareNeutralSvg(
    sourceSvg("official", "#444444", "0").replace(/<text[\s\S]*?<\/text>/, ""),
  );
  const changedText = prepareNeutralSvg(sourceSvg("official", "#444444", "0", "Box shifted"));
  for (const changed of [removedBox, removedConnector, removedLabel, changedText]) {
    expect(compareSvgGeometry(original, changed).accepted).toBe(false);
  }
});

test("neutral SVG geometry bounds transform tolerance and preserves viewport dimensions", () => {
  const original = prepareNeutralSvg(sourceSvg("official", "#444444", "0"));
  const tinyDelta = prepareNeutralSvg(sourceSvg("candidate", "#444444", "0.000009"));
  const shifted = prepareNeutralSvg(sourceSvg("candidate", "#444444", "0.00002"));
  const scaled = prepareNeutralSvg(
    sourceSvg("candidate", "#444444", "0").replace('viewBox="0 0 120 80"', 'viewBox="0 0 121 80"'),
  );
  expect(compareSvgGeometry(original, tinyDelta).accepted).toBe(true);
  expect(compareSvgGeometry(original, shifted).accepted).toBe(false);
  expect(compareSvgGeometry(original, scaled).accepted).toBe(false);
});

test("neutral SVG geometry fails closed for malformed and unsupported SVG content", () => {
  expect(() => prepareNeutralSvg("<svg><g></svg>")).toThrow();
  expect(() =>
    prepareNeutralSvg('<!DOCTYPE svg [<!ENTITY x SYSTEM "file:///etc/passwd">]><svg>&x;</svg>'),
  ).toThrow();
  expect(() =>
    prepareNeutralSvg('<svg xmlns="http://www.w3.org/2000/svg"><image href="x.png"/></svg>'),
  ).toThrow();
});

test("paint projection scores actual candidate pixels and rejects PNG-only label loss", async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "krr-paint-projection-"));
  try {
    const fixture = await createPaintProjectionFixture(root),
      projected = await rasterizePaintProjectedReferenceBatch(
        [fixture.input],
        path.join(root, "projected"),
        "dark",
      ),
      neutral = await rasterizeNeutralSvgBatch(
        [fixture.neutralOfficial, fixture.neutralCandidate],
        path.join(root, "neutral"),
        "dark",
      ),
      magick = new MagickOps(path.join(root, "magick"), null, "dark"),
      projectedRecord = projected.records[0],
      neutralOfficial = neutral.records.find((record) => record.role === "official"),
      neutralCandidate = neutral.records.find((record) => record.role === "candidate");
    expect(projectedRecord).toBeDefined();
    expect(neutralOfficial).toBeDefined();
    expect(neutralCandidate).toBeDefined();
    if (
      projectedRecord === undefined ||
      neutralOfficial === undefined ||
      neutralCandidate === undefined
    ) {
      throw new Error("Paint projection fixture did not produce every expected raster");
    }
    const projectionScore = magick.comparePaintProjectedImages(
        projectedRecord.projectedReferencePngPath,
        fixture.input.candidatePngPath,
        path.join(root, "projected"),
        fixture.input.slug,
      ),
      missingLabelScore = magick.comparePaintProjectedImages(
        projectedRecord.projectedReferencePngPath,
        fixture.missingLabelPngPath,
        path.join(root, "projected"),
        "missing-label",
      ),
      neutralScore = magick.compareGeometryImages(
        neutralOfficial.neutralPngPath,
        neutralCandidate.neutralPngPath,
        path.join(root, "neutral"),
        fixture.input.slug,
      );
    expect(projectionScore.score).toBe(100);
    expect(missingLabelScore.score).toBeLessThan(99);
    expect(neutralScore.fullCanvasScore).toBe(100);
    expect(fixture.input.candidateSvgSha256).toBe(fileSha256(fixture.input.candidateSvgPath));
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

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

test.each(paintProjectionIdMapCases)(
  "paint projection allows repeated IDs only for one-to-one repeated mappings: $name",
  async (item) => {
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
  },
);

interface PaintProjectionFixture {
  input: PaintProjectedReferenceInput;
  neutralOfficial: NeutralRasterInput;
  neutralCandidate: NeutralRasterInput;
  missingLabelPngPath: string;
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
  return `<svg xmlns="http://www.w3.org/2000/svg" width="160" height="80" viewBox="0 0 160 80"><rect x="4" y="4" width="152" height="72" rx="4" fill="${fill}" stroke="${fill}"/><text x="10" y="45" fill="${text}" font-family="Arial, sans-serif" font-size="18">DELETE LABEL TEXT</text></svg>`;
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
  };
}

async function renderPaintProjectionFixture(
  officialSvg: string,
  candidateSvg: string,
  officialPngPath: string,
  candidatePngPath: string,
  missingLabelPngPath: string,
) {
  const playwright = await new PlaywrightLoader().load(),
    browser = await playwright.chromium.launch({ headless: true });
  try {
    const page = await browser.newPage({
      viewport: { width: 1520, height: 845 },
      deviceScaleFactor: 1,
    });
    try {
      await renderProjectionPng(page, officialSvg, officialPngPath, false);
      await renderProjectionPng(page, candidateSvg, candidatePngPath, false);
      await renderProjectionPng(page, candidateSvg, missingLabelPngPath, true);
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
  removeLabel: boolean,
) {
  await page.setContent(rasterCaptureHtml(DiagramTheme.parse("dark")), { waitUntil: "load" });
  await assignStrictSvg(page, svg);
  await resizeSvgCapture(page, true, "strictViewBox");
  await waitForStrictSvgExternalImages(page);
  await page.evaluate(() => (document as FontReadyDocument).fonts.ready);
  if (removeLabel) {
    await page.evaluate(() => {
      const text = document.querySelector("#diagram svg text");
      if (text === null) throw new Error("Fixture SVG text node is missing");
      text.textContent = "";
    });
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
