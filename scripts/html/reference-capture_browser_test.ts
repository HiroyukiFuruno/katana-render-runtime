import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { expect, test } from "playwright/test";
import { DiagramTheme } from "../mermaid/diagram_theme";
import { nativeFrameCaptureWrapper } from "../mermaid/native-frame-evidence";
import type { PageHandle } from "../mermaid/official-renderer-types";
import { assignStrictSvg, resizeSvgCapture } from "../mermaid/raster-capture-observation";
import { rasterCaptureHtml } from "../mermaid/rasterize-svg-dir";
import {
  type CaptureOptions,
  createCaptureBrowser,
  runWithCaptureBrowser,
  sha256File,
} from "./reference-capture";

test("static capture isolates contexts, scores real PNGs, and survives a failed case", async () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "krr-browser-reuse-"));
  const htmlPath = path.join(directory, "fixture.html");
  const brokenHtmlPath = path.join(directory, "broken.html");
  const missingHtmlPath = path.join(directory, "missing.html");
  const krrPath = path.join(directory, "reference.png");
  const html =
    "<!doctype html><style>body{margin:0;background:white;color:black;font:24px sans-serif}</style>" +
    "<body><script>localStorage.count = String(Number(localStorage.count || 0) + 1);" +
    "document.body.append(localStorage.count)</script></body>";
  fs.writeFileSync(htmlPath, html);
  fs.writeFileSync(
    brokenHtmlPath,
    html.replace("</body>", '<img src="/missing-resource.png"></body>'),
  );
  const options = (outputDir: string, htmlPathValue = htmlPath): CaptureOptions => ({
    htmlPath: htmlPathValue,
    krrPath,
    outputDir,
    width: 320,
    height: 200,
    scale: 1,
    mode: "viewport",
    scrollY: 0,
    minScore: 99,
  });
  const captureBrowser = createCaptureBrowser();
  try {
    const first = await captureBrowser.capture(options(path.join(directory, "first")));
    fs.copyFileSync(first.capturePath, krrPath);
    await expect(
      runWithCaptureBrowser(
        options(path.join(directory, "broken"), brokenHtmlPath),
        captureBrowser,
      ),
    ).rejects.toThrow("asset loading failed");
    const report = await runWithCaptureBrowser(
      options(path.join(directory, "scored")),
      captureBrowser,
    );
    const scores = JSON.parse(
      fs.readFileSync(path.join(directory, "scored", "scores.json"), "utf8"),
    ) as { score: number; browser: { actualScrollY: number }; input: { htmlSha256: string } };
    expect(report.passed).toBe(true);
    expect(scores.score).toBe(100);
    expect(scores.browser.actualScrollY).toBe(0);
    expect(scores.input.htmlSha256).toBe(sha256File(htmlPath));
    await expect(
      runWithCaptureBrowser(
        options(path.join(directory, "missing"), missingHtmlPath),
        captureBrowser,
      ),
    ).rejects.toThrow("HTML input does not exist");
  } finally {
    await captureBrowser.close();
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

const nativeFrameSizingCases = [
  {
    relativePath: "tests/fixtures/mermaid/en/official-dark/08-01-c4-context-simple.svg",
    viewBox: [0, -70, 1148, 441],
  },
  {
    relativePath: "tests/fixtures/mermaid/en/official-dark/22-01-tree-view-simple.svg",
    viewBox: [-0.5, 0, 124.828125, 232],
  },
] as const;

test("native wrapper uses the same natural viewBox as formal Chrome capture", async ({ page }) => {
  await page.setViewportSize({ width: 1520, height: 845 });
  for (const target of nativeFrameSizingCases) await compareNativeAndFormalSizing(page, target);
  for (const viewBox of ["0 0 20", "0 0 0 20", "0 0 -1 20", "0 0 20 10 5"]) {
    await expectInvalidBrowserViewBox(page, viewBox);
  }
});

async function compareNativeAndFormalSizing(
  page: PageHandle,
  fixture: (typeof nativeFrameSizingCases)[number],
) {
  const svg = fs.readFileSync(path.join(process.cwd(), fixture.relativePath), "utf8"),
    frame = {
      width: Math.ceil(fixture.viewBox[2]) + 24,
      height: Math.ceil(fixture.viewBox[3]) + 24,
    };
  await page.setContent(nativeFrameCaptureWrapper(svg, frame), { waitUntil: "load" });
  const native = await readBrowserGeometry(page);
  await page.setContent(rasterCaptureHtml(DiagramTheme.parse("dark")), { waitUntil: "load" });
  const loaded = await assignStrictSvg(page, svg);
  expect(loaded.assignedSvgSerialization).toBe(native.svgHtml);
  await resizeSvgCapture(page, true, "strictViewBox");
  const formal = await readBrowserGeometry(page);
  expect(native.capture).toEqual(frame);
  expect(native.diagram).toEqual(formal.diagram);
  expect(native.svgBox).toEqual(formal.svgBox);
  expect(native.capture).toEqual(formal.capture);
  expect(native.svgHtml).toBe(loaded.expectedSvgSerialization);
  expect(native.rootFontSize).toBe("16px");
}

function readBrowserGeometry(page: PageHandle) {
  return page.evaluate(() => {
    const capture = document.getElementById("capture") as HTMLElement,
      diagram = document.getElementById("diagram") as HTMLElement,
      svg = document.querySelector("#diagram svg") as SVGSVGElement,
      captureBox = capture.getBoundingClientRect(),
      diagramBox = diagram.getBoundingClientRect(),
      svgBox = svg.getBoundingClientRect();
    return {
      capture: { width: captureBox.width, height: captureBox.height },
      diagram: { width: diagramBox.width, height: diagramBox.height },
      svgBox: { width: svgBox.width, height: svgBox.height },
      svgHtml: svg.outerHTML,
      rootFontSize: getComputedStyle(svg).fontSize,
    };
  });
}

async function expectInvalidBrowserViewBox(page: PageHandle, viewBox: string) {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${viewBox}"/>`;
  expect(() => nativeFrameCaptureWrapper(svg, { width: 44, height: 44 })).toThrow(
    "invalid SVG viewBox",
  );
  await page.setContent(rasterCaptureHtml(DiagramTheme.parse("dark")), { waitUntil: "load" });
  await assignStrictSvg(page, svg);
  await expect(resizeSvgCapture(page, true, "strictViewBox")).rejects.toThrow(
    "SVG viewBox is invalid",
  );
}
