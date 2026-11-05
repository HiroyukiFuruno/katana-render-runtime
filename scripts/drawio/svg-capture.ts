import fs from "node:fs";
import path from "node:path";
import type { FontReadyDocument, PageHandle } from "../mermaid/official-renderer-types";
import { OfficialDrawioSourceFonts } from "./official-source-fonts";

export interface DrawioSvgCaptureInput {
  slug: string;
  svg: Buffer;
  sourceFonts: string;
  outputDir: string;
}

export function drawioCaptureHtml(): string {
  return `<!doctype html><html><head><meta charset="utf-8"><style>
html,body{margin:0;background:#1e1e1e;color:#e0e0e0;color-scheme:dark;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}
#capture{display:block;overflow:hidden;padding:12px;box-sizing:border-box;background:#1e1e1e}
#diagram{display:block}
#diagram svg{display:block}
</style></head><body><div id="capture"><div id="diagram"></div></div></body></html>`;
}

export function writeRawDrawioSvg(filePath: string, svg: Buffer) {
  fs.writeFileSync(filePath, svg);
}

export function assertFreshDrawioOutput(outputPath: string) {
  if (fs.existsSync(outputPath)) {
    throw new Error(`Draw.io comparison output already exists: ${outputPath}`);
  }
}

export function assertMatchingDrawioSlugs(
  fixtureSlugs: string[],
  candidateSlugs: string[],
  officialSvgSlugs: string[],
  officialPngSlugs: string[],
) {
  const hasDuplicates = (slugs: string[]) => new Set(slugs).size !== slugs.length;
  const expected = JSON.stringify([...fixtureSlugs].sort());
  const inventories = [candidateSlugs, officialSvgSlugs, officialPngSlugs];
  if (
    fixtureSlugs.length === 0 ||
    [fixtureSlugs, ...inventories].some((slugs) => slugs.length === 0 || hasDuplicates(slugs)) ||
    inventories.some((slugs) => JSON.stringify([...slugs].sort()) !== expected)
  ) {
    throw new Error("candidate and fresh official inventories must match Draw.io fixtures");
  }
}

export async function captureDrawioSvg(page: PageHandle, input: DrawioSvgCaptureInput) {
  const markup = input.svg.toString("utf8");
  if (!Buffer.from(markup, "utf8").equals(input.svg)) {
    throw new Error(`SVG input is not valid UTF-8: ${input.slug}`);
  }
  writeRawDrawioSvg(path.join(input.outputDir, `${input.slug}.svg`), input.svg);
  await page.evaluate((candidate) => {
    (document.getElementById("diagram") as HTMLElement).innerHTML = candidate;
  }, markup);
  const attributesBefore = await svgAttributeSnapshot(page);
  await OfficialDrawioSourceFonts.install(page, input.sourceFonts);
  await waitForSvgImages(page);
  await sizeCaptureToSvg(page);
  await page.evaluate(() => (document as FontReadyDocument).fonts.ready);
  const attributesAfter = await svgAttributeSnapshot(page);
  if (attributesBefore !== attributesAfter) {
    throw new Error(`SVG DOM attributes changed during capture: ${input.slug}`);
  }
  await page.locator("#capture").screenshot({
    path: path.join(input.outputDir, `${input.slug}.png`),
    omitBackground: false,
  });
}

function waitForSvgImages(page: PageHandle): Promise<void> {
  return page.evaluate(() =>
    Promise.all(
      Array.from(document.querySelectorAll("#diagram image")).map(
        (element) =>
          new Promise<void>((resolve) => {
            const href = element.getAttribute("href") ?? element.getAttribute("xlink:href") ?? "";
            const image = new Image();
            image.onload = () => resolve();
            image.onerror = () => resolve();
            image.src = href;
          }),
      ),
    ).then(() => undefined),
  );
}

function sizeCaptureToSvg(page: PageHandle): Promise<void> {
  return page.evaluate(() => {
    const svg = document.querySelector("#diagram svg") as SVGSVGElement;
    const box = svg.getBoundingClientRect();
    if (box.width <= 0 || box.height <= 0) {
      throw new Error("Candidate SVG has no positive rendered dimensions");
    }
    const width = Math.ceil(box.width);
    const height = Math.ceil(box.height);
    const capture = document.getElementById("capture") as HTMLElement;
    capture.style.width = `${width + 24}px`;
    capture.style.height = `${height + 24}px`;
  });
}

function svgAttributeSnapshot(page: PageHandle): Promise<string> {
  return page.evaluate(() => {
    const elements = Array.from(document.querySelectorAll("#diagram svg, #diagram svg *"));
    if (elements.length === 0) {
      throw new Error("Draw.io capture did not load an SVG element");
    }
    return JSON.stringify(
      elements.map((element) => Array.from(element.attributes, ({ name, value }) => [name, value])),
    );
  });
}
