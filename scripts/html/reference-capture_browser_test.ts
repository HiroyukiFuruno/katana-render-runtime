import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { expect, test } from "playwright/test";
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
