import fs from "node:fs";
import path from "node:path";
import type { Page } from "playwright";
import { expect, test } from "playwright/test";
import { applyEventsInteraction } from "./dynamic-events";
import { DYNAMIC_VIEWPORTS } from "./dynamic-fidelity";
import { assertExpectedStateMatches, type JsonValue, specForDynamicCase } from "./dynamic-state";
import {
  assertCatalogComplete,
  assertFeatureResultReportsComplete,
  assertFeatureResultsComplete,
  type FeatureCaseResult,
  type FeatureCatalog,
  loadFeatureCatalog,
  readExpectedState,
  readFeatureResults,
} from "./feature-catalog";
import { TOC_LINK_SELECTOR } from "./scroll-coverage";
import { inspectChromeCatalogBoundary } from "./visual-state-fidelity";

const root = path.resolve("tests/fixtures/html");
const tocSelectorFixture = path.resolve("tests/fixtures/html-harness/toc-selector-contract.html");
function passedFeatureResults(catalog: FeatureCatalog): FeatureCaseResult[] {
  return [...catalog.visualAspects, ...catalog.scriptCases].map((entry) => ({
    caseId: entry.id,
    status: "passed",
    features: Object.fromEntries(entry.features.map((feature) => [feature, "passed"])),
  }));
}

async function evaluateDynamicFixture(
  page: Page,
  entry: { id: string; fixture: string; expectedStateId?: string },
  viewport: (typeof DYNAMIC_VIEWPORTS)[number],
): Promise<void> {
  const fixture = path.resolve(root, entry.fixture);
  const expected = readExpectedState(fixture, entry.expectedStateId ?? "expected-state");
  const spec = specForDynamicCase(entry.id);
  expect(spec.expression).toContain("document");
  await page.setViewportSize({ width: viewport.width, height: viewport.height });
  await page.goto(`file://${fixture}`, { waitUntil: "networkidle" });
  if (entry.id === "events") {
    await applyEventsInteraction(page);
  }
  await page.waitForFunction(() => document.readyState === "complete");
  if (entry.id === "observers")
    await page.waitForFunction(() =>
      document.querySelector("#state")?.textContent?.includes("mutation"),
    );
  if (entry.id === "resources")
    await page.waitForFunction(
      () =>
        document.querySelector("#state")?.textContent === "loaded" &&
        document.documentElement.dataset.fonts === "ready",
    );
  if (entry.id === "async")
    await page.waitForFunction(() =>
      document.querySelector("#state")?.textContent?.includes("timer"),
    );
  const state = (await page.evaluate((expression) => {
    const script = document.createElement("script");
    script.textContent = `globalThis.__krrCatalogState = (${expression});`;
    document.head.append(script);
    script.remove();
    const stateGlobal = globalThis as typeof globalThis & { __krrCatalogState?: JsonValue };
    const value = stateGlobal.__krrCatalogState ?? null;
    delete stateGlobal.__krrCatalogState;
    return value;
  }, spec.expression)) as JsonValue;
  assertExpectedStateMatches(expected, state);
  expect(
    await inspectChromeCatalogBoundary(page, entry.id, "expected-state", viewport),
  ).toBeDefined();
}

test.describe("HTML feature catalog", () => {
  test("collects legacy and navigation TOC links once in document order", async ({ browser }) => {
    const page = await browser.newPage();
    try {
      await page.setContent(fs.readFileSync(tocSelectorFixture, "utf8"));
      const hrefs = await page
        .locator(TOC_LINK_SELECTOR)
        .evaluateAll((links) => links.map((link) => link.getAttribute("href")));
      expect(hrefs).toEqual(["#legacy", "#both", "#nav"]);
    } finally {
      await page.close();
    }
  });

  test("maps every aspect fixture and keeps the 99 threshold", () => {
    const catalog = loadFeatureCatalog();
    expect(() => assertCatalogComplete(catalog)).not.toThrow();
    expect(catalog.visualAspects).toHaveLength(6);
    expect(catalog.scriptCases).toHaveLength(5);
  });

  test("rejects malformed catalog boundaries and incomplete execution results", () => {
    const catalog = loadFeatureCatalog();
    const complete = passedFeatureResults(catalog);
    expect(() => assertFeatureResultsComplete(catalog, complete)).not.toThrow();
    const first = complete[0];
    if (!first) throw new Error("catalog is empty");
    expect(() => assertFeatureResultsComplete(catalog, [...complete, first])).toThrow(
      "duplicate feature result",
    );
    expect(() => assertFeatureResultsComplete(catalog, complete.slice(1))).toThrow(
      "unrun feature case",
    );
    expect(() =>
      assertFeatureResultsComplete(
        catalog,
        complete.map((result, index) =>
          index === 0 ? { ...result, status: "unverified" } : result,
        ),
      ),
    ).toThrow("not passed");
    expect(() =>
      assertFeatureResultsComplete(catalog, [
        ...complete,
        { caseId: "extra", status: "passed", features: {} },
      ]),
    ).toThrow("extra feature result");
    const malformed = path.join(root, "malformed-feature-catalog.json");
    fs.writeFileSync(malformed, JSON.stringify({ schemaVersion: 1, threshold: 98 }));
    try {
      expect(() => loadFeatureCatalog(malformed)).toThrow("schema");
    } finally {
      fs.unlinkSync(malformed);
    }
    expect(() => assertExpectedStateMatches({ state: "correct" }, { state: "mutant" })).toThrow(
      "dynamic state mismatch",
    );
  });

  test("rejects missing policies, mixed case modes and invalid boundary states", () => {
    const catalog = loadFeatureCatalog();
    const file = path.join(root, "malformed-feature-catalog.json");
    try {
      for (const invalid of [
        { ...catalog, coveragePolicy: {} },
        { ...catalog, visualAspects: catalog.scriptCases },
        {
          ...catalog,
          visualAspects: catalog.visualAspects.map((entry) => ({ ...entry, boundaryStates: [12] })),
        },
      ]) {
        fs.writeFileSync(file, JSON.stringify(invalid));
        expect(() => loadFeatureCatalog(file)).toThrow();
      }
    } finally {
      fs.unlinkSync(file);
    }
  });

  test("rejects unmapped and missing expected script states", () => {
    const catalog = loadFeatureCatalog();
    const extra = path.join(root, "aspects", "unmapped.html");
    fs.writeFileSync(extra, "<!doctype html><title>unmapped</title>");
    try {
      expect(() => assertCatalogComplete(catalog)).toThrow("unmapped feature fixture");
    } finally {
      fs.unlinkSync(extra);
    }
    expect(() => readExpectedState(path.join(root, "aspects", "typography.html"))).toThrow(
      "missing expected state",
    );
  });

  test("combines authoritative visual and script reports and rejects malformed reports", () => {
    const catalog = loadFeatureCatalog();
    const complete = passedFeatureResults(catalog);
    const visualFile = path.join(root, "visual-results.json");
    const scriptFile = path.join(root, "script-results.json");
    fs.writeFileSync(visualFile, JSON.stringify(complete.slice(0, catalog.visualAspects.length)));
    fs.writeFileSync(scriptFile, JSON.stringify(complete.slice(catalog.visualAspects.length)));
    try {
      expect(readFeatureResults(visualFile)).toHaveLength(catalog.visualAspects.length);
      expect(() =>
        assertFeatureResultReportsComplete(catalog, visualFile, scriptFile),
      ).not.toThrow();
      fs.writeFileSync(scriptFile, JSON.stringify([]));
      expect(() => assertFeatureResultReportsComplete(catalog, visualFile, scriptFile)).toThrow(
        "unrun feature case",
      );
      fs.writeFileSync(scriptFile, JSON.stringify([{ caseId: "events", status: "passed" }]));
      expect(() => readFeatureResults(scriptFile)).toThrow("features");
    } finally {
      fs.unlinkSync(visualFile);
      fs.unlinkSync(scriptFile);
    }
  });

  for (const viewport of DYNAMIC_VIEWPORTS) {
    test(`evaluates every dynamic fixture in Chrome at ${viewport.width}px`, async ({
      browser,
    }) => {
      const catalog = loadFeatureCatalog();
      const page = await browser.newPage({
        viewport: { width: viewport.width, height: viewport.height },
      });
      try {
        for (const entry of catalog.scriptCases)
          await evaluateDynamicFixture(page, entry, viewport);
      } finally {
        await page.close();
      }
    });
  }

  test("checks responsive flex wrapping at each planned viewport in Chrome", async ({
    browser,
  }) => {
    const page = await browser.newPage();
    try {
      for (const viewport of DYNAMIC_VIEWPORTS) {
        await page.setViewportSize({ width: viewport.width, height: viewport.height });
        await page.goto(`file://${path.resolve(root, "aspects/layout.html")}`, {
          waitUntil: "networkidle",
        });
        const boundary = await inspectChromeCatalogBoundary(
          page,
          "layout",
          "narrow-viewport",
          viewport,
        );
        expect(boundary).toMatchObject({ width: viewport.width, wrapped: viewport.width <= 700 });
      }
    } finally {
      await page.close();
    }
  });
});
