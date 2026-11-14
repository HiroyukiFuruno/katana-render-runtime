import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { type Browser, chromium, type Page } from "playwright";

export type CaptureMode = "viewport" | "full-page" | "scroll";
export type ImageFailurePolicy = "complete" | "natural-width";

export interface CaptureOptions {
  htmlPath: string;
  krrPath: string;
  outputDir: string;
  width: number;
  height: number;
  scale: number;
  mode: CaptureMode;
  scrollY: number;
  minScore: number;
  expectedHtmlSha256?: string;
}

export interface ImageSize {
  width: number;
  height: number;
}

export interface ScoreReport {
  score: number;
  rmseScore: number;
  maeScore: number;
  minScore: number;
  passed: boolean;
  dimensions: ImageSize;
}

interface BrowserMetadata {
  name: string;
  version: string;
  actualScrollY: number;
}

export function parseOptions(argv: string[]): CaptureOptions {
  const knownOptions = new Set([
    "--html",
    "--krr",
    "--output",
    "--width",
    "--height",
    "--scale",
    "--mode",
    "--scroll-y",
    "--min-score",
  ]);
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv.at(index);
    if (argument === undefined || !knownOptions.has(argument)) {
      throw new Error(`Invalid CLI argument at index ${index}: ${argument}`);
    }
    const value = argv.at(index + 1);
    if (value === undefined || value.startsWith("--")) {
      throw new Error(`Missing value for CLI argument: ${argument}`);
    }
    index += 1;
  }
  const get = (name: string, fallback?: string): string => {
    const index = argv.indexOf(name);
    const value = index >= 0 ? argv.at(index + 1) : undefined;
    if (value !== undefined) return value;
    if (fallback !== undefined) return fallback;
    throw new Error(`Missing required option: ${name}`);
  };
  const number = (name: string, fallback: number, minimum = 0, integer = false): number => {
    const value = Number(get(name, String(fallback)));
    if (!Number.isFinite(value) || value < minimum || (integer && !Number.isInteger(value)))
      throw new Error(`Invalid number option: ${name}`);
    return value;
  };
  const mode = get("--mode", "viewport");
  if (mode !== "viewport" && mode !== "full-page" && mode !== "scroll") {
    throw new Error(`Invalid capture mode: ${mode}`);
  }
  const minScore = number("--min-score", 99, 0);
  if (minScore > 100) throw new Error("Invalid number option: --min-score");
  return {
    htmlPath: path.resolve(get("--html")),
    krrPath: path.resolve(get("--krr")),
    outputDir: path.resolve(get("--output", "tmp/html-chrome-fidelity")),
    width: number("--width", 1280, 1, true),
    height: number("--height", 720, 1, true),
    scale: number("--scale", 1, 0.1),
    mode,
    scrollY: number("--scroll-y", 0, 0, true),
    minScore,
  };
}

export function parseMetricOutput(value: string): number {
  const match = value.match(/\(([-+]?\d*\.?\d+(?:e[-+]?\d+)?)\)/i);
  if (!match?.[1]) throw new Error(`ImageMagick metric parse failed: ${value.trim()}`);
  const parsed = Number(match[1]);
  if (!Number.isFinite(parsed)) throw new Error(`Invalid image metric: ${value.trim()}`);
  return parsed;
}

export function scoreFromErrors(
  rmseError: number,
  maeError: number,
  minScore: number,
): ScoreReport {
  const rmseScore = Math.max(0, 100 * (1 - rmseError));
  const maeScore = Math.max(0, 100 * (1 - maeError));
  const score = Math.min(rmseScore, maeScore);
  return {
    score,
    rmseScore,
    maeScore,
    minScore,
    passed: score >= minScore,
    dimensions: { width: 0, height: 0 },
  };
}

export function sha256File(filePath: string): string {
  return createHash("sha256").update(fs.readFileSync(filePath)).digest("hex");
}

export function assertInputUnchanged(before: string, after: string): void {
  if (before !== after) throw new Error("HTML input changed while capture was running");
}

export function assertExpectedInputSha(expected: string | undefined, actual: string): void {
  if (expected !== undefined && expected !== actual) {
    throw new Error(`HTML input SHA changed: expected ${expected}, got ${actual}`);
  }
}

export function assertExactlyOneReportedInputSha(output: string, expected: string): string {
  const matches = [...output.matchAll(/source_sha256=([0-9a-f]{64})\b/gi)];
  const actual = matches[0]?.[1]?.toLowerCase();
  if (matches.length !== 1 || actual === undefined) {
    throw new Error("KRR probe did not report exactly one source SHA");
  }
  assertExpectedInputSha(expected, actual);
  return actual;
}

export function assertNoFailedAssets(failedAssets: string[]): void {
  if (failedAssets.length > 0) {
    throw new Error(`HTML asset loading failed: ${failedAssets.join(", ")}`);
  }
}

export function collectFailedAssetResponses(page: Page, failedResponses: string[]): void {
  page.on("response", (response) => {
    if (
      response.status() >= 400 &&
      /\.(?:css|gif|ico|jpe?g|png|svg|ttf|otf|woff2?)(?:\?|$)/i.test(response.url())
    ) {
      failedResponses.push(`${response.status()} ${response.url()}`);
    }
  });
}

export function failedAssetUrls(
  assets: {
    images: Array<{ currentSrc: string; complete: boolean; naturalWidth: number }>;
    fonts: string[];
  },
  imagePolicy: ImageFailurePolicy = "complete",
): string[] {
  return [
    ...assets.images
      .filter(
        (image) =>
          image.currentSrc.length > 0 &&
          (image.naturalWidth === 0 || (imagePolicy === "complete" && !image.complete)),
      )
      .map((image) => image.currentSrc),
    ...assets.fonts,
  ];
}

export async function waitForAssets(
  page: Page,
  failedResponses: string[],
  imagePolicy: ImageFailurePolicy = "complete",
): Promise<void> {
  await page.evaluate(async () => {
    await document.fonts.ready;
    await Promise.all(
      Array.from(document.images).map(async (image) => {
        if (image.complete) return;
        await new Promise<void>((resolve) => {
          image.addEventListener("load", () => resolve(), { once: true });
          image.addEventListener("error", () => resolve(), { once: true });
        });
      }),
    );
  });
  const failedAssets = await page.evaluate(() => {
    const images = Array.from(document.images).map((image) => ({
      currentSrc: image.currentSrc,
      complete: image.complete,
      naturalWidth: image.naturalWidth,
    }));
    const failedFonts = Array.from(document.fonts)
      .filter((font) => font.status === "error")
      .map((font) => font.family);
    return { images, fonts: failedFonts };
  });
  failedResponses.push(...failedAssetUrls(failedAssets, imagePolicy));
  assertNoFailedAssets(failedResponses);
}

function identify(pathname: string): ImageSize {
  const result = spawnSync("magick", ["identify", "-format", "%w %h", pathname]);
  if (result.status !== 0) throw new Error(`ImageMagick identify failed for ${pathname}`);
  const dimensions = new TextDecoder()
    .decode(result.stdout ?? new Uint8Array())
    .trim()
    .split(/\s+/)
    .map(Number);
  const width = dimensions.at(0);
  const height = dimensions.at(1);
  if (
    width === undefined ||
    height === undefined ||
    !Number.isFinite(width) ||
    !Number.isFinite(height)
  ) {
    throw new Error(`Invalid image dimensions for ${pathname}`);
  }
  return { width, height };
}

function compare(metric: string, left: string, right: string): number {
  const result = spawnSync("magick", ["compare", "-metric", metric, left, right, "null:"]);
  if (result.status !== 0 && result.status !== 1) {
    throw new Error(
      `ImageMagick compare failed (${metric}): ${new TextDecoder().decode(
        result.stderr ?? new Uint8Array(),
      )}`,
    );
  }
  return parseMetricOutput(
    `${new TextDecoder().decode(result.stderr ?? new Uint8Array())}${new TextDecoder().decode(
      result.stdout ?? new Uint8Array(),
    )}`,
  );
}

function writeDiff(left: string, right: string, output: string): void {
  const result = spawnSync("magick", ["compare", "-compose", "difference", left, right, output]);
  if (result.status !== 0 && result.status !== 1) {
    throw new Error(
      `ImageMagick diff failed: ${new TextDecoder().decode(result.stderr ?? new Uint8Array())}`,
    );
  }
}

export interface CaptureBrowser {
  capture(options: CaptureOptions): Promise<BrowserMetadata & { capturePath: string }>;
  close(): Promise<void>;
}

async function captureWithBrowser(
  browser: Browser,
  options: CaptureOptions,
): Promise<BrowserMetadata & { capturePath: string }> {
  if (options.mode === "full-page") {
    throw new Error(
      "Full-page capture requires a KRR full-page reference route; use viewport or scroll mode",
    );
  }
  fs.mkdirSync(options.outputDir, { recursive: true });
  const capturePath = path.join(options.outputDir, "chrome.png");
  const context = await browser.newContext({
    viewport: { width: options.width, height: options.height },
    deviceScaleFactor: options.scale,
    colorScheme: "light",
  });
  try {
    const page = await context.newPage();
    const failedResponses: string[] = [];
    collectFailedAssetResponses(page, failedResponses);
    await page.goto(pathToFileURL(options.htmlPath).href, { waitUntil: "networkidle" });
    await waitForAssets(page, failedResponses);
    if (options.mode === "scroll") {
      await page.evaluate((y) => window.scrollTo(0, y), options.scrollY);
      await page.waitForFunction((requested) => {
        const scrollingElement = document.scrollingElement;
        const maxScrollY = Math.max(
          0,
          (scrollingElement?.scrollHeight ?? document.documentElement.scrollHeight) -
            window.innerHeight,
        );
        const target = Math.min(Math.max(requested, 0), maxScrollY);
        return Math.abs(window.scrollY - target) <= 1;
      }, options.scrollY);
    }
    await page.screenshot({ path: capturePath, fullPage: false });
    const version = browser.version();
    const actualScrollY = await page.evaluate(() => window.scrollY);
    return { name: "Google Chrome", version, actualScrollY, capturePath };
  } finally {
    await context.close();
  }
}

export function createCaptureBrowser(): CaptureBrowser {
  let browser: Browser | undefined;
  return {
    async capture(options) {
      browser ??= await chromium.launch({ channel: "chrome", headless: true });
      return captureWithBrowser(browser, options);
    },
    async close() {
      if (browser === undefined) return;
      const current = browser;
      browser = undefined;
      await current.close();
    },
  };
}

function validateCaptureInputs(options: CaptureOptions): string {
  if (!fs.existsSync(options.htmlPath))
    throw new Error(`HTML input does not exist: ${options.htmlPath}`);
  if (!fs.existsSync(options.krrPath))
    throw new Error(`KRR PNG does not exist: ${options.krrPath}`);
  const htmlSha256Before = sha256File(options.htmlPath);
  assertExpectedInputSha(options.expectedHtmlSha256, htmlSha256Before);
  return htmlSha256Before;
}

export async function run(options: CaptureOptions): Promise<ScoreReport> {
  const captureBrowser = createCaptureBrowser();
  try {
    return await runWithCaptureBrowser(options, captureBrowser);
  } finally {
    await captureBrowser.close();
  }
}

export async function runWithCaptureBrowser(
  options: CaptureOptions,
  captureBrowser: CaptureBrowser,
): Promise<ScoreReport> {
  const htmlSha256Before = validateCaptureInputs(options);
  const browser = await captureBrowser.capture(options);
  const htmlSha256After = sha256File(options.htmlPath);
  assertInputUnchanged(htmlSha256Before, htmlSha256After);
  assertExpectedInputSha(options.expectedHtmlSha256, htmlSha256After);
  const report = compareImages(browser.capturePath, options.krrPath, options.minScore);
  writeDiff(browser.capturePath, options.krrPath, path.join(options.outputDir, "diff.png"));
  fs.writeFileSync(
    path.join(options.outputDir, "scores.json"),
    `${JSON.stringify(
      {
        ...report,
        input: {
          htmlPath: options.htmlPath,
          htmlSha256: htmlSha256After,
          krrPath: options.krrPath,
          krrSha256: sha256File(options.krrPath),
        },
        browser,
        viewport: { width: options.width, height: options.height, scale: options.scale },
        colorScheme: "light",
        mode: options.mode,
        scrollY: options.scrollY,
      },
      null,
      2,
    )}\n`,
  );
  console.log(`score: ${report.score.toFixed(2)} (minimum ${options.minScore.toFixed(2)})`);
  console.log(`capture: ${browser.capturePath}`);
  console.log(`diff: ${path.join(options.outputDir, "diff.png")}`);
  return report;
}

export function compareImages(chromePath: string, krrPath: string, minScore: number): ScoreReport {
  const capturedSize = identify(chromePath);
  const krrSize = identify(krrPath);
  if (capturedSize.width !== krrSize.width || capturedSize.height !== krrSize.height) {
    throw new Error(
      `PNG dimensions differ: Chrome ${capturedSize.width}x${capturedSize.height}, KRR ${krrSize.width}x${krrSize.height}`,
    );
  }
  const rmseError = compare("RMSE", chromePath, krrPath);
  const maeError = compare("MAE", chromePath, krrPath);
  const report = scoreFromErrors(rmseError, maeError, minScore);
  report.dimensions = capturedSize;
  return report;
}

if (import.meta.main) {
  run(parseOptions(process.argv.slice(2)))
    .then((report) => {
      process.exitCode = report.passed ? 0 : 1;
    })
    .catch((error: Error) => {
      console.error(error instanceof Error ? error.message : error);
      process.exitCode = 1;
    });
}
