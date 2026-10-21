import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { chromium } from "playwright";
import { waitForAssets } from "./reference-capture";

export type InteractionName = "link" | "scroll" | "form" | "js";

export interface InteractionState {
  link: { url: string };
  scroll: { scrollY: number; contentHeight: number; viewportHeight: number };
  form: { value: string; result: string };
  js: { expanded: boolean | null; result: string };
}

export interface InteractionReport {
  inputSha256: string;
  browser: {
    name: string;
    version: string;
    viewport: { width: number; height: number };
    scale: number;
  };
  states: Partial<InteractionState>;
  performance?: { firstFrameMs: number; closeMs: number; contractMaxMs: number };
}

export interface InteractionSuiteOptions {
  htmlPath: string;
  outputDir: string;
  width: number;
  height: number;
  scale: number;
  krrReport?: string;
  maxPerformanceMs: number;
}

export interface InteractionSuiteResult {
  evidenceKind: "supplemental-interaction-regression";
  matchedInteractions: number;
  totalInteractions: number;
  passed: boolean;
  comparisons: Record<InteractionName, { passed: boolean; mismatches: string[] }>;
  performance: InteractionReport["performance"] | null;
  performancePassed: boolean | null;
}

const INTERACTIONS: InteractionName[] = ["link", "scroll", "form", "js"];

export function parseInteractionOptions(argv: string[]): InteractionSuiteOptions {
  const allowed = new Set([
    "--html",
    "--output",
    "--width",
    "--height",
    "--scale",
    "--krr-report",
    "--max-performance-ms",
  ]);
  const values = new Map<string, string>();
  for (let index = 0; index < argv.length; index += 2) {
    const name = argv.at(index);
    const value = argv.at(index + 1);
    if (!name || !allowed.has(name) || value === undefined || value.startsWith("--")) {
      throw new Error(`Invalid interaction argument near index ${index}`);
    }
    values.set(name, value);
  }
  const html = values.get("--html");
  if (!html) throw new Error("Missing required option: --html");
  const integer = (name: string, fallback: number): number => {
    const value = Number(values.get(name) ?? fallback);
    if (!Number.isInteger(value) || value < 1) throw new Error(`Invalid integer option: ${name}`);
    return value;
  };
  const scale = Number(values.get("--scale") ?? "1");
  const budget = Number(values.get("--max-performance-ms") ?? "60000");
  if (!Number.isFinite(scale) || scale <= 0 || !Number.isFinite(budget) || budget <= 0) {
    throw new Error("Invalid numeric interaction option");
  }
  const options: InteractionSuiteOptions = {
    htmlPath: path.resolve(html),
    outputDir: path.resolve(values.get("--output") ?? "tmp/html-interaction-suite"),
    width: integer("--width", 1280),
    height: integer("--height", 900),
    scale,
    maxPerformanceMs: budget,
  };
  const krrReport = values.get("--krr-report");
  if (krrReport !== undefined) options.krrReport = path.resolve(krrReport);
  return options;
}

function sha256(filePath: string): string {
  return createHash("sha256").update(fs.readFileSync(filePath)).digest("hex");
}

async function captureChrome(options: InteractionSuiteOptions): Promise<InteractionReport> {
  const browser = await chromium.launch({ channel: "chrome", headless: true });
  try {
    fs.mkdirSync(options.outputDir, { recursive: true });
    const context = await browser.newContext({
      viewport: { width: options.width, height: options.height },
      deviceScaleFactor: options.scale,
      colorScheme: "light",
    });
    const page = await context.newPage();
    const failedRequests: string[] = [];
    page.on("requestfailed", (request) => failedRequests.push(request.url()));
    await page.goto(pathToFile(options.htmlPath), { waitUntil: "networkidle" });
    await waitForAssets(page, failedRequests, "natural-width");
    const states: Partial<InteractionState> = {};
    await page.locator("a[data-krr-link], a[href]").first().click();
    await page.waitForLoadState("networkidle").catch(() => undefined);
    states.link = { url: page.url() };
    await page.screenshot({ path: path.join(options.outputDir, "link.png") });

    await page.goto(pathToFile(options.htmlPath), { waitUntil: "networkidle" });
    await waitForAssets(page, failedRequests, "natural-width");
    states.scroll = await page.evaluate(() => {
      window.scrollTo({ top: 600, left: 0, behavior: "instant" });
      return {
        scrollY: window.scrollY,
        contentHeight: document.documentElement.scrollHeight,
        viewportHeight: window.innerHeight,
      };
    });
    await page.screenshot({ path: path.join(options.outputDir, "scroll.png") });

    await page.goto(pathToFile(options.htmlPath), { waitUntil: "networkidle" });
    await waitForAssets(page, failedRequests, "natural-width");
    const form = page.locator("form").first();
    await form.locator("input, textarea").first().fill("ok");
    await form.locator("button[type=submit], input[type=submit]").first().click();
    states.form = await page.evaluate(() => ({
      value:
        (
          document.querySelector("form input, form textarea") as
            | HTMLInputElement
            | HTMLTextAreaElement
            | null
        )?.value ?? "",
      result: document.querySelector("[data-krr-result]")?.textContent?.trim() ?? "",
    }));
    await page.screenshot({ path: path.join(options.outputDir, "form.png") });

    await page.goto(pathToFile(options.htmlPath), { waitUntil: "networkidle" });
    await waitForAssets(page, failedRequests, "natural-width");
    await page.locator("button[data-krr-action], [data-krr-action]").first().click();
    states.js = await page.evaluate(() => {
      const action = document.querySelector("button[data-krr-action], [data-krr-action]");
      const expanded = action?.getAttribute("aria-expanded");
      return {
        expanded: expanded === null ? null : expanded === "true",
        result: document.querySelector("[data-krr-result]")?.textContent?.trim() ?? "",
      };
    });
    await page.screenshot({ path: path.join(options.outputDir, "js.png") });
    const report: InteractionReport = {
      inputSha256: sha256(options.htmlPath),
      browser: {
        name: "Google Chrome",
        version: browser.version(),
        viewport: { width: options.width, height: options.height },
        scale: options.scale,
      },
      states,
    };
    fs.writeFileSync(
      path.join(options.outputDir, "chrome-interactions.json"),
      `${JSON.stringify(report, null, 2)}\n`,
    );
    return report;
  } finally {
    await browser.close();
  }
}

function pathToFile(filePath: string): string {
  return pathToFileURL(filePath).href;
}

export function compareInteractionStates(
  chrome: Partial<InteractionState>,
  krr: Partial<InteractionState>,
): InteractionSuiteResult["comparisons"] {
  return Object.fromEntries(
    INTERACTIONS.map((name) => {
      const left = chrome[name];
      const right = krr[name];
      const mismatches: string[] = [];
      if (left === undefined || right === undefined) mismatches.push("state missing");
      else if (!sameInteractionState(name, left, right))
        mismatches.push(`Chrome=${JSON.stringify(left)} KRR=${JSON.stringify(right)}`);
      return [name, { passed: mismatches.length === 0, mismatches }];
    }),
  ) as InteractionSuiteResult["comparisons"];
}

function sameInteractionState(
  name: InteractionName,
  left: InteractionState[InteractionName],
  right: InteractionState[InteractionName],
): boolean {
  switch (name) {
    case "link":
      return (left as InteractionState["link"]).url === (right as InteractionState["link"]).url;
    case "scroll": {
      const leftScroll = left as InteractionState["scroll"];
      const rightScroll = right as InteractionState["scroll"];
      return (
        leftScroll.scrollY === rightScroll.scrollY &&
        leftScroll.contentHeight === rightScroll.contentHeight &&
        leftScroll.viewportHeight === rightScroll.viewportHeight
      );
    }
    case "form":
      return (
        (left as InteractionState["form"]).value === (right as InteractionState["form"]).value &&
        (left as InteractionState["form"]).result === (right as InteractionState["form"]).result
      );
    case "js":
      return (
        (left as InteractionState["js"]).expanded === (right as InteractionState["js"]).expanded &&
        (left as InteractionState["js"]).result === (right as InteractionState["js"]).result
      );
  }
}

export async function runInteractionSuite(
  options: InteractionSuiteOptions,
): Promise<InteractionSuiteResult> {
  const before = sha256(options.htmlPath);
  const chrome = await captureChrome(options);
  if (sha256(options.htmlPath) !== before)
    throw new Error("HTML input changed during interaction capture");
  if (!options.krrReport) throw new Error("KRR report is required for an independent comparison");
  const krr = JSON.parse(fs.readFileSync(options.krrReport, "utf8")) as InteractionReport;
  if (krr.inputSha256 !== before) throw new Error("KRR report input SHA does not match HTML input");
  const comparisons = compareInteractionStates(chrome.states, krr.states);
  const result = evaluateInteractionEvidence(
    comparisons,
    krr.performance,
    options.maxPerformanceMs,
  );
  fs.writeFileSync(
    path.join(options.outputDir, "scores.json"),
    `${JSON.stringify(result, null, 2)}\n`,
  );
  return result;
}

export function evaluateInteractionEvidence(
  comparisons: InteractionSuiteResult["comparisons"],
  metrics: InteractionReport["performance"],
  maxPerformanceMs: number,
): InteractionSuiteResult {
  const matchedInteractions = Object.values(comparisons).filter((item) => item.passed).length;
  const performance = metrics ?? null;
  const performancePassed =
    performance === null
      ? null
      : Number.isFinite(performance.firstFrameMs) &&
        Number.isFinite(performance.closeMs) &&
        performance.firstFrameMs >= 0 &&
        performance.closeMs >= 0 &&
        performance.firstFrameMs + performance.closeMs < maxPerformanceMs;
  return {
    evidenceKind: "supplemental-interaction-regression",
    matchedInteractions,
    totalInteractions: INTERACTIONS.length,
    passed: matchedInteractions === INTERACTIONS.length && performancePassed === true,
    comparisons,
    performance,
    performancePassed,
  };
}

if (import.meta.main) {
  try {
    const result = await runInteractionSuite(parseInteractionOptions(Bun.argv.slice(2)));
    console.log(JSON.stringify(result));
    if (!result.passed) process.exitCode = 1;
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
