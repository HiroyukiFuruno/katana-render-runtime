import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { expect, test } from "playwright/test";
import { FixtureRepository } from "./diagram_update_fixtures";
import { OfficialMermaidRenderer } from "./official-renderer";
import { resolveOfficialRendererAssetInputs } from "./official-renderer-asset-inputs";
import type { FontReadyDocument } from "./official-renderer-types";
import {
  assignStrictSvg,
  prepareStrictSvgCapture,
  resizeSvgCapture,
} from "./raster-capture-observation";
import { RasterProductionSession } from "./raster-production-session";
import { waitForStrictSvgExternalImages } from "./strict-svg-external-images";

test("official raw producer captures on a freshly prepared viewport", async ({ browser }) => {
  const root = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), "krr-official-viewport-")));
  const outputDir = path.join(root, "output"),
    attempt = path.join(root, "attempt");
  fs.mkdirSync(outputDir);
  fs.mkdirSync(attempt);
  const fixture = new FixtureRepository(path.resolve("tests/fixtures/mermaid/en")).read(
    "08-02-c4-context-full.md",
  );
  const assets = resolveOfficialRendererAssetInputs([], process.env, process.cwd(), os.homedir());
  const endpointPaths = [
    ["producer", "scripts/mermaid/official-renderer.ts"],
    ["playwright", "node_modules/playwright/package.json"],
    ["lock", "bun.lock"],
    ["runtime", assets.mermaidJs],
    ["font", "scripts/mermaid/diagram_theme.ts"],
    ["config", "package.json"],
  ] as const;
  const session = new RasterProductionSession({
    role: "official",
    attempt,
    nonce: "official-viewport-component",
    inventory: [fixture.slug],
    outputDir,
    observerPython: fs.realpathSync("/usr/bin/python3"),
    endpoints: endpointPaths.map(([kind, file]) => ({
      name: kind,
      kind,
      version: "test-physical-input",
      path: fs.realpathSync(file),
    })),
  });
  const renderer = new OfficialMermaidRenderer({
    outputDir,
    ...assets,
    theme: "dark",
    referenceMode: "strictVendorOriginal",
    referenceScope: "diagnostic",
    rasterSession: session,
  });
  try {
    renderer.prepareRasterInputs([fixture]);
    await renderer.start();
    try {
      await renderer.render(fixture);
    } finally {
      await renderer.stop();
    }
    const receiptPath = renderer.finishRasterReceipt();
    if (receiptPath === undefined) throw new Error("Official raster receipt missing");
    const receipt = JSON.parse(fs.readFileSync(receiptPath, "utf8"));
    const manifest = JSON.parse(
      fs.readFileSync(path.join(outputDir, `${fixture.slug}.pair.json`), "utf8"),
    );
    const provenanceRoot = path.join(outputDir, ".provenance");
    const provenanceDirectory = fs.readdirSync(provenanceRoot)[0];
    if (provenanceDirectory === undefined) throw new Error("Official raw stage missing");
    const raw = fs.readFileSync(path.join(provenanceRoot, provenanceDirectory, "raw-mermaid.svg"));
    expect(fs.readFileSync(path.join(outputDir, `${fixture.slug}.svg`))).toEqual(raw);
    expect(fs.readFileSync(path.join(outputDir, `${fixture.slug}.rendered.svg`))).toEqual(raw);
    expect(manifest.config.captureResize).toBe("raw-svg-viewbox-viewport-before-assignment-v1");
    expect(manifest.config.initialViewport).toEqual({
      width: 1520,
      height: 845,
      deviceScaleFactor: 1,
    });
    const page = await browser.newPage({
      viewport: { width: 1520, height: 845 },
      deviceScaleFactor: 1,
    });
    try {
      await page.setContent(renderer.baseHtml(), { waitUntil: "load" });
      const viewport = await prepareStrictSvgCapture(page, raw.toString("utf8"));
      expect(manifest.config.viewport).toEqual({ ...viewport, deviceScaleFactor: 1 });
      expect(receipt.rows[0].capture.viewport).toEqual(viewport);
      await assignStrictSvg(page, raw.toString("utf8"));
      await resizeSvgCapture(page, true, "strictViewBox");
      await waitForStrictSvgExternalImages(page);
      await page.evaluate(() => (document as FontReadyDocument).fonts.ready);
      const expected = await page.locator("#capture").screenshot({ omitBackground: false });
      expect(fs.readFileSync(path.join(outputDir, `${fixture.slug}.png`))).toEqual(expected);
    } finally {
      await page.close();
    }
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
