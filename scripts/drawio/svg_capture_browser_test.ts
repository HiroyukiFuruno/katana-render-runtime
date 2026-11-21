import { expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { PlaywrightLoader } from "../mermaid/official-renderer";
import type { PageHandle } from "../mermaid/official-renderer-types";
import { captureDrawioSvg, drawioCaptureHtml } from "./svg-capture";

function pngWidth(filePath: string): number {
  const png = fs.readFileSync(filePath);
  expect(png.toString("ascii", 1, 4)).toBe("PNG");
  return png.readUInt32BE(16);
}

async function captureWidth(page: PageHandle, outputDir: string, width: number): Promise<number> {
  await page.setContent(drawioCaptureHtml(), { waitUntil: "load" });
  const svg = Buffer.from(
    `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="100" viewBox="0 0 200 200"><rect width="200" height="200" fill="#fff"/></svg>`,
    "utf8",
  );
  await captureDrawioSvg(page, {
    slug: `declared-${width}`,
    svg,
    sourceFonts: "<mxGraphModel/>",
    outputDir,
  });
  return pngWidth(path.join(outputDir, `declared-${width}.png`));
}

test("Chrome preserves SVG geometry attributes and uses declared width without viewBox correction", async () => {
  const outputDir = fs.mkdtempSync(path.join(os.tmpdir(), "drawio-svg-capture-"));
  const { chromium } = await new PlaywrightLoader().load();
  const browser = await chromium.launch({ headless: true });
  try {
    const page = await browser.newPage({
      viewport: { width: 1520, height: 845 },
      deviceScaleFactor: 1,
    });
    try {
      expect(await captureWidth(page, outputDir, 100)).toBe(124);
      expect(await captureWidth(page, outputDir, 200)).toBe(224);
    } finally {
      await page.close();
    }
  } finally {
    await browser.close();
    fs.rmSync(outputDir, { recursive: true, force: true });
  }
});
