import fs from "node:fs";
import path from "node:path";
import { chromium, type Page } from "playwright";
import { captureKrrState, DYNAMIC_VIEWPORTS } from "./dynamic-fidelity";
import type { JsonValue } from "./dynamic-state";
import { loadFeatureCatalog } from "./feature-catalog";
import { type FidelityPhase, fidelityThresholdForPhase } from "./fidelity-phase";
import { compareImages, type ScoreReport, sha256File } from "./reference-capture";

export const INPUT_STATE_OPERATIONS = [
  "empty",
  "filled",
  "selected",
  "expanded",
  "disabled",
  "focus-visible",
] as const;
type InputStateOperation = (typeof INPUT_STATE_OPERATIONS)[number];
const INPUT_STATE_FIXTURE = path.resolve("tests/fixtures/html/aspects/input-state.html");
const STATE_EXPRESSION = "JSON.parse(document.querySelector('#state-summary').textContent || '{}')";

function stableJson(value: JsonValue): string {
  if (Array.isArray(value)) return `[${value.map(stableJson).join(",")}]`;
  if (typeof value === "object" && value !== null) {
    return `{${Object.entries(value)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, item]) => `${JSON.stringify(key)}:${stableJson(item)}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
}

function captureReport(value: JsonValue): { capturePath: string; htmlSha256: string } {
  if (
    typeof value !== "object" ||
    value === null ||
    Array.isArray(value) ||
    typeof value.capturePath !== "string" ||
    typeof value.htmlSha256 !== "string"
  ) {
    throw new Error("invalid KRR visual capture report");
  }
  return { capturePath: value.capturePath, htmlSha256: value.htmlSha256 };
}

export interface VisualStateResult {
  operation: InputStateOperation;
  viewport: { width: number; height: number };
  score: ScoreReport;
  stateMatches: boolean;
  passed: boolean;
  chromePath: string;
  krrPath: string;
}

async function applyChromeOperation(page: Page, operation: InputStateOperation): Promise<void> {
  if (operation === "filled") {
    await page.locator("#name").click();
    await page.locator("#name").fill("架空の名称");
    await page.locator("#save").click();
  } else if (operation === "selected") {
    await page.locator("#tab-advanced").click();
  } else if (operation === "expanded") {
    await page.locator("#accordion").click();
  } else if (operation === "focus-visible") {
    await page.keyboard.press("Tab");
  }
}

async function captureChromeState(
  page: Page,
  operation: InputStateOperation,
  viewport: (typeof DYNAMIC_VIEWPORTS)[number],
  output: string,
): Promise<JsonValue> {
  await page.setViewportSize(viewport);
  await page.goto(`file://${INPUT_STATE_FIXTURE}`, { waitUntil: "networkidle" });
  await applyChromeOperation(page, operation);
  const state = await page.evaluate((expression) => {
    const script = document.createElement("script");
    script.textContent = `globalThis.__krrVisualState = (${expression});`;
    document.head.append(script);
    const stateGlobal = globalThis as typeof globalThis & { __krrVisualState?: JsonValue };
    const value = stateGlobal.__krrVisualState ?? null;
    delete stateGlobal.__krrVisualState;
    script.remove();
    return value;
  }, STATE_EXPRESSION);
  await page.screenshot({ path: output, fullPage: false });
  return state;
}

function assertInputStateCatalogCoverage(): void {
  const catalogCase = loadFeatureCatalog().visualAspects.find(
    (entry) => entry.id === "input-state",
  );
  if (!catalogCase) throw new Error("input-state is missing from the feature catalog");
  if (stableJson(catalogCase.boundaryStates ?? []) !== stableJson([...INPUT_STATE_OPERATIONS])) {
    throw new Error("input-state operations do not cover catalog boundary states");
  }
}

export async function runInputStateFidelity(
  binary: string,
  outputDirectory: string,
  minScore = 99,
  options: { phase: "interim" } | undefined = undefined,
): Promise<VisualStateResult[]> {
  const phase: FidelityPhase = options?.phase ?? "final";
  const threshold = fidelityThresholdForPhase(phase, minScore);
  assertInputStateCatalogCoverage();
  const binarySha = sha256File(binary);
  const fixtureSha = sha256File(INPUT_STATE_FIXTURE);
  fs.mkdirSync(outputDirectory, { recursive: true });
  const browser = await chromium.launch({ channel: "chrome", headless: true });
  const page = await browser.newPage();
  const results: VisualStateResult[] = [];
  try {
    for (const operation of INPUT_STATE_OPERATIONS) {
      for (const viewport of DYNAMIC_VIEWPORTS) {
        const label = `${operation}-${viewport.width}x${viewport.height}`;
        const chromePath = path.join(outputDirectory, `${label}-chrome.png`);
        const krrJson = path.join(outputDirectory, `${label}-krr.json`);
        const krrState = captureKrrState(
          binary,
          INPUT_STATE_FIXTURE,
          STATE_EXPRESSION,
          krrJson,
          `input-state:${operation}`,
          viewport,
        );
        const report = captureReport(JSON.parse(fs.readFileSync(krrJson, "utf8")) as JsonValue);
        if (report.htmlSha256 !== fixtureSha) {
          throw new Error(`input-state provenance mismatch: ${label}`);
        }
        const chromeState = await captureChromeState(page, operation, viewport, chromePath);
        const stateMatches = stableJson(chromeState) === stableJson(krrState);
        const score = compareImages(chromePath, report.capturePath, threshold);
        results.push({
          operation,
          viewport,
          score,
          stateMatches,
          passed: stateMatches && score.passed,
          chromePath,
          krrPath: report.capturePath,
        });
      }
    }
  } finally {
    await page.close();
    await browser.close();
  }
  if (sha256File(binary) !== binarySha) throw new Error("visual state binary changed");
  if (sha256File(INPUT_STATE_FIXTURE) !== fixtureSha)
    throw new Error("visual state fixture changed");
  const resultPath = path.join(
    outputDirectory,
    phase === "interim" ? "interim-fidelity-results.json" : "fidelity-results.json",
  );
  const result = phase === "interim" ? { phase, threshold, results } : results;
  fs.writeFileSync(resultPath, `${JSON.stringify(result, null, 2)}\n`);
  if (results.length !== INPUT_STATE_OPERATIONS.length * DYNAMIC_VIEWPORTS.length) {
    throw new Error("input-state visual state coverage is incomplete");
  }
  if (results.some((result) => !result.passed))
    throw new Error("input-state visual score gate failed");
  return results;
}
