import { randomUUID } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { chromium, type Page } from "playwright";
import { applyEventsInteraction } from "./dynamic-events";
import { assertExpectedStateMatches, type JsonValue, specForDynamicCase } from "./dynamic-state";
import {
  type CatalogCase,
  type FeatureCaseResult,
  loadFeatureCatalog,
  readExpectedState,
} from "./feature-catalog";
import { type FidelityPhase, fidelityThresholdForPhase } from "./fidelity-phase";
import {
  assertExpectedInputSha,
  compareImages,
  type ScoreReport,
  sha256File,
} from "./reference-capture";

export type DynamicViewport = { width: number; height: number };

export const DYNAMIC_VIEWPORTS = [
  { width: 1280, height: 900 },
  { width: 960, height: 720 },
  { width: 620, height: 900 },
] as const;

export async function captureDynamicStatePair(
  fixture: string,
  captureChrome: () => Promise<JsonValue>,
  captureKrr: (sourceSha256: string) => JsonValue,
): Promise<{ sourceSha256: string; chromeState: JsonValue; krrState: JsonValue }> {
  const sourceSha256 = sha256File(fixture);
  const chromeState = await captureChrome();
  assertExpectedInputSha(sourceSha256, sha256File(fixture));
  const krrState = captureKrr(sourceSha256);
  assertExpectedInputSha(sourceSha256, sha256File(fixture));
  return { sourceSha256, chromeState, krrState };
}

export function captureKrrState(
  binary: string,
  fixture: string,
  expression: string,
  output: string,
  caseId = "dynamic-dom",
  viewport: DynamicViewport = DYNAMIC_VIEWPORTS[0],
  expectedHtmlSha256?: string,
): JsonValue {
  const sourceSha256 = sha256File(fixture);
  assertExpectedInputSha(expectedHtmlSha256, sourceSha256);
  const binarySha256 = sha256File(binary);
  const nonce = randomUUID();
  const result = Bun.spawnSync(
    [binary, "writes_fixture_feature_state_interactive", "--nocapture"],
    {
      env: {
        ...process.env,
        KRR_HTML_FEATURE_INPUT: fixture,
        KRR_HTML_FEATURE_EXPRESSION: expression,
        KRR_HTML_FEATURE_NONCE: nonce,
        KRR_HTML_FEATURE_CASE: caseId,
        KRR_HTML_FEATURE_BINARY_SHA256: binarySha256,
        KRR_HTML_FEATURE_WIDTH: String(viewport.width),
        KRR_HTML_FEATURE_HEIGHT: String(viewport.height),
        KRR_HTML_FEATURE_OUTPUT: output,
      },
    },
  );
  if (result.exitCode !== 0)
    throw new Error(
      new TextDecoder().decode(result.stdout) + new TextDecoder().decode(result.stderr),
    );
  if (sha256File(binary) !== binarySha256) throw new Error("Dynamic probe binary changed");
  assertExpectedInputSha(sourceSha256, sha256File(fixture));
  if (!new TextDecoder().decode(result.stdout).includes("test result: ok. 1 passed"))
    throw new Error("Dynamic diagnostic did not execute exactly one test");
  return readKrrDynamicState(output, nonce, sourceSha256, binarySha256);
}

export function readKrrDynamicState(
  output: string,
  nonce: string,
  sourceSha256: string,
  binarySha256: string,
): JsonValue {
  const report: JsonValue = JSON.parse(fs.readFileSync(output, "utf8"));
  if (typeof report !== "object" || report === null) throw new Error("Invalid dynamic report");
  const value = report as Record<string, JsonValue>;
  if (value.executionNonce !== nonce || value.htmlSha256 !== sourceSha256)
    throw new Error("Stale dynamic report");
  if (value.state === undefined) throw new Error("Missing dynamic state");
  if (typeof value.capturePath !== "string" || !fs.existsSync(value.capturePath))
    throw new Error("Missing post-operation dynamic capture");
  if (value.binarySha256 !== binarySha256) throw new Error("Dynamic binary provenance mismatch");
  return value.state;
}

async function captureChromeState(
  page: Page,
  entry: CatalogCase,
  output: string,
  viewport: DynamicViewport,
): Promise<JsonValue> {
  const fixture = path.resolve("tests/fixtures/html", entry.fixture);
  const spec = specForDynamicCase(entry.id);
  await page.setViewportSize(viewport);
  await page.goto(`file://${fixture}`, { waitUntil: "networkidle" });
  if (entry.id === "events") {
    await applyEventsInteraction(page);
  }
  await waitForDynamicState(page, entry.id);
  const state = (await page.evaluate((expression) => {
    const script = document.createElement("script");
    script.textContent = `globalThis.__krrCatalogState = (${expression});`;
    document.head.append(script);
    const value =
      (globalThis as typeof globalThis & { __krrCatalogState?: JsonValue }).__krrCatalogState ??
      null;
    delete (globalThis as typeof globalThis & { __krrCatalogState?: JsonValue }).__krrCatalogState;
    script.remove();
    return value;
  }, spec.expression)) as JsonValue;
  const expected = readExpectedState(fixture, entry.expectedStateId);
  assertExpectedStateMatches(expected, state);
  await page.screenshot({ path: output, fullPage: false });
  return state;
}

async function waitForDynamicState(page: Page, caseId: string): Promise<void> {
  await page.waitForFunction(() => document.readyState === "complete");
  const selectors: Record<string, string> = {
    async: "#state",
    observers: "#state",
    resources: "#state",
  };
  const selector = selectors[caseId];
  if (!selector) return;
  await page.waitForFunction((target) => {
    const text = document.querySelector(target)?.textContent ?? "";
    return target === "#state" && text !== "" && text !== "initial" && text !== "waiting";
  }, selector);
}

export function runDynamicStates(
  binary: string,
  outputDirectory: string,
): Array<{ caseId: string; passed: boolean; error?: string }> {
  fs.mkdirSync(outputDirectory, { recursive: true });
  const results = loadFeatureCatalog().scriptCases.map((entry) => {
    const fixture = path.resolve("tests/fixtures/html", entry.fixture);
    const spec = specForDynamicCase(entry.id);
    try {
      const actual = captureKrrState(
        binary,
        fixture,
        spec.expression,
        path.join(outputDirectory, `${entry.id}.json`),
        entry.id,
      );
      assertExpectedStateMatches(readExpectedState(fixture, entry.expectedStateId), actual);
      return { caseId: entry.id, passed: true };
    } catch (error) {
      return { caseId: entry.id, passed: false, error: String(error) };
    }
  });
  fs.writeFileSync(
    path.join(outputDirectory, "results.json"),
    `${JSON.stringify(results, null, 2)}\n`,
  );
  return results;
}

export interface DynamicFidelityResult {
  caseId: string;
  sourceSha256: string;
  viewport: { width: number; height: number };
  score: ScoreReport;
  passed: boolean;
  chromePath: string;
  krrPath: string;
}

export async function runDynamicFidelity(
  binary: string,
  outputDirectory: string,
  minScore = 99,
  options: { phase: "interim" } | undefined = undefined,
): Promise<DynamicFidelityResult[]> {
  const phase: FidelityPhase = options?.phase ?? "final";
  const threshold = fidelityThresholdForPhase(phase, minScore);
  const binarySha256 = sha256File(binary);
  fs.mkdirSync(outputDirectory, { recursive: true });
  const browser = await chromium.launch({ channel: "chrome", headless: true });
  const page = await browser.newPage();
  const results: DynamicFidelityResult[] = [];
  try {
    for (const entry of loadFeatureCatalog().scriptCases) {
      for (const viewport of DYNAMIC_VIEWPORTS) {
        const label = `${entry.id}-${viewport.width}x${viewport.height}`;
        const fixture = path.resolve("tests/fixtures/html", entry.fixture);
        const chromePath = path.join(outputDirectory, `${label}-chrome.png`);
        const krrJson = path.join(outputDirectory, `${label}-krr.json`);
        const krrPath = krrJson.replace(/\.json$/, ".png");
        const { sourceSha256, chromeState, krrState } = await captureDynamicStatePair(
          fixture,
          () => captureChromeState(page, entry, chromePath, viewport),
          (expectedSha) =>
            captureKrrState(
              binary,
              fixture,
              specForDynamicCase(entry.id).expression,
              krrJson,
              entry.id,
              viewport,
              expectedSha,
            ),
        );
        assertExpectedStateMatches(readExpectedState(fixture, entry.expectedStateId), chromeState);
        assertExpectedStateMatches(chromeState, krrState);
        const score = compareImages(chromePath, krrPath, threshold);
        results.push({
          caseId: entry.id,
          sourceSha256,
          viewport,
          score,
          passed: score.passed,
          chromePath,
          krrPath,
        });
      }
    }
  } finally {
    await page.close();
    await browser.close();
  }
  if (sha256File(binary) !== binarySha256)
    throw new Error("Dynamic probe binary changed during fidelity capture");
  const fidelityResultPath = path.join(
    outputDirectory,
    phase === "interim" ? "interim-fidelity-results.json" : "fidelity-results.json",
  );
  const fidelityResult = phase === "interim" ? { phase, threshold, results } : results;
  fs.writeFileSync(fidelityResultPath, `${JSON.stringify(fidelityResult, null, 2)}\n`);
  const scriptCases = loadFeatureCatalog().scriptCases;
  const scriptFeatureResults: FeatureCaseResult[] = scriptCases.map((entry) => {
    const caseResults = results.filter((result) => result.caseId === entry.id);
    const passed =
      caseResults.length === DYNAMIC_VIEWPORTS.length &&
      caseResults.every((result) => result.passed);
    return {
      caseId: entry.id,
      status: passed ? "passed" : "failed",
      features: Object.fromEntries(
        entry.features.map((feature) => [feature, passed ? "passed" : "failed"]),
      ),
    };
  });
  const scriptFeatureResultPath = path.join(
    outputDirectory,
    phase === "interim" ? "interim-script-feature-results.json" : "script-feature-results.json",
  );
  const scriptFeatureResult =
    phase === "interim"
      ? { phase, threshold, results: scriptFeatureResults }
      : scriptFeatureResults;
  fs.writeFileSync(scriptFeatureResultPath, `${JSON.stringify(scriptFeatureResult, null, 2)}\n`);
  if (
    results.length !== scriptCases.length * DYNAMIC_VIEWPORTS.length ||
    results.some((result) => !result.passed)
  ) {
    throw new Error("Dynamic fidelity score gate failed");
  }
  return results;
}
