import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { chromium, type Page } from "playwright";
import { collectFailedAssetResponses, waitForAssets } from "./reference-capture";

export const BOTTOM_SCROLL_REQUEST = 1_000_000_000;
export const TOC_LINK_SELECTOR = '.sidebar .toc-item[href^="#"], nav a[href^="#"]';

export interface ScrollViewport {
  width: number;
  height: number;
}

export interface ScrollRequest {
  name: string;
  requested: number;
}

export interface TocEvidence {
  index: number;
  href: string;
  text: string;
  targetId: string | null;
  targetExists: boolean;
  hash: string;
  requestedScrollY: number;
  actualScrollY: number;
  targetTop: number | null;
  targetVisible: boolean;
  capturePath: string;
  viewport: ScrollViewport;
}

export interface ScrollCoverageManifest {
  htmlPath: string;
  htmlSha256: string;
  viewport: ScrollViewport & { scale: 1 };
  scrollHeight: number;
  maxScrollY: number;
  scrolls: Array<ScrollRequest & { actual: number; capturePath: string }>;
  toc: TocEvidence[];
  entries: Array<{
    destination: string;
    hash: string;
    scrollY: number;
    capturePath: string;
    targetVisible: boolean;
  }>;
  coverage: { positions: number[]; gapLimit: number };
}

export interface MeasureScrollCoverageInput {
  htmlPath: string;
  viewport: ScrollViewport;
  outputDir: string;
}

function sha256File(filePath: string): string {
  return createHash("sha256").update(fs.readFileSync(filePath)).digest("hex");
}

function finite(value: number, label: string): number {
  if (!Number.isFinite(value) || value < 0)
    throw new Error(`${label} must be finite and non-negative`);
  return value;
}

/** Build overlapping viewport samples, preserving the legacy 600/1800 points and TOC targets. */
export function buildScrollPlan(
  maxScrollY: number,
  viewportHeight: number,
  targetScrollYs: number[] = [],
): ScrollRequest[] {
  const max = finite(maxScrollY, "maxScrollY");
  const height = finite(viewportHeight, "viewportHeight");
  if (height === 0) throw new Error("viewportHeight must be greater than zero");
  const clamp = (position: number): number => Math.round(Math.min(max, Math.max(0, position)));
  const positions = new Set<number>([0, clamp(600), clamp(1800)]);
  const step = height * 0.8;
  for (let position = 0; position <= max; position += step) positions.add(clamp(position));
  for (const target of targetScrollYs) positions.add(clamp(finite(target, "targetScrollY")));
  const ordered = [...positions].sort((left, right) => left - right);
  return [
    ...ordered.map((requested) => ({ name: String(requested), requested })),
    { name: "bottom", requested: BOTTOM_SCROLL_REQUEST },
  ];
}

/** Assert that every part of the document can be seen by at least one captured viewport. */
export function assertScrollCoverage(
  actualScrollYs: number[],
  maxScrollY: number,
  viewportHeight: number,
): void {
  const max = finite(maxScrollY, "maxScrollY");
  const height = finite(viewportHeight, "viewportHeight");
  if (height === 0) throw new Error("viewportHeight must be greater than zero");
  if (
    actualScrollYs.some(
      (position) => !Number.isFinite(position) || position < -0.01 || position > max + 0.01,
    )
  ) {
    throw new Error("scroll coverage contains an invalid position");
  }
  const positions = actualScrollYs.slice().sort((left, right) => left - right);
  if (positions.length === 0 || (positions[0] ?? Infinity) > 0.01) {
    throw new Error("scroll coverage does not start at document top");
  }
  const gapLimit = height * 0.8 + 0.01;
  for (let index = 1; index < positions.length; index += 1) {
    const previous = positions[index - 1] ?? 0;
    const current = positions[index] ?? previous;
    if (current - previous > gapLimit)
      throw new Error(`scroll coverage gap: ${previous}..${current}`);
  }
  const last = positions.at(-1) ?? 0;
  if (max - last > 0.01)
    throw new Error(`scroll coverage does not reach document bottom: ${last} < ${max}`);
}

async function waitForScrollSettled(page: Page, requested: number): Promise<number> {
  await page.evaluate(
    (value) => window.scrollTo({ top: value, left: 0, behavior: "auto" }),
    requested,
  );
  await page.waitForFunction((value) => {
    const element = document.scrollingElement;
    const max = Math.max(0, (element?.scrollHeight ?? 0) - window.innerHeight);
    const target = Math.min(Math.max(value, 0), max);
    const current = window.scrollY;
    return Math.abs(current - target) <= 0.01;
  }, requested);
  return await page.evaluate(() => window.scrollY);
}

async function waitForAnchorSettled(page: Page, expectedHash: string): Promise<void> {
  await page.waitForFunction((hash) => {
    if (window.location.hash !== hash) return false;
    const target = document.getElementById(decodeURIComponent(hash.slice(1)));
    if (!target) return false;
    const scrolling = document.scrollingElement;
    const margin = Number.parseFloat(getComputedStyle(target).scrollMarginTop) || 0;
    const padding = scrolling
      ? Number.parseFloat(getComputedStyle(scrolling).scrollPaddingTop) || 0
      : 0;
    const maximum = Math.max(0, (scrolling?.scrollHeight ?? 0) - window.innerHeight);
    const destination = Math.round(
      Math.min(
        maximum,
        Math.max(0, target.getBoundingClientRect().top + window.scrollY - margin - padding),
      ),
    );
    return Math.abs(window.scrollY - destination) <= 1;
  }, expectedHash);
  await page.evaluate(async () => {
    const id = decodeURIComponent(window.location.hash.replace(/^#/, ""));
    const target = id ? document.getElementById(id) : null;
    const visible = (): boolean => {
      if (target === null) return true;
      const rect = target.getBoundingClientRect();
      return rect.bottom >= 0 && rect.top <= window.innerHeight;
    };
    let previous = window.scrollY;
    for (;;) {
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
      const current = window.scrollY;
      if (visible() && Math.abs(current - previous) <= 0.01) {
        await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
        if (visible() && Math.abs(window.scrollY - current) <= 0.01) return;
      }
      previous = current;
    }
  });
}

export async function measureScrollCoverage(
  input: MeasureScrollCoverageInput,
): Promise<ScrollCoverageManifest> {
  const htmlPath = path.resolve(input.htmlPath);
  const before = sha256File(htmlPath);
  const browser = await chromium.launch({ channel: "chrome", headless: true });
  try {
    const context = await browser.newContext({
      viewport: input.viewport,
      deviceScaleFactor: 1,
      colorScheme: "light",
    });
    const page = await context.newPage();
    const failedResponses: string[] = [];
    collectFailedAssetResponses(page, failedResponses);
    await page.goto(pathToFileURL(htmlPath).href, { waitUntil: "networkidle" });
    await waitForAssets(page, failedResponses);
    const documentInfo = await page.evaluate((selector) => {
      const scrolling = document.scrollingElement;
      const maxScrollY = Math.max(0, (scrolling?.scrollHeight ?? 0) - window.innerHeight);
      const links = [...document.querySelectorAll<HTMLAnchorElement>(selector)].map((link) => {
        const href = link.getAttribute("href") ?? "";
        const id = decodeURIComponent(href.slice(1));
        const target = id ? document.getElementById(id) : null;
        return {
          href,
          text: link.textContent?.trim() ?? "",
          targetId: target?.id ?? null,
          targetExists: target !== null,
          targetY: target ? target.getBoundingClientRect().top + window.scrollY : null,
        };
      });
      return {
        scrollHeight: scrolling?.scrollHeight ?? document.documentElement.scrollHeight,
        maxScrollY,
        links,
      };
    }, TOC_LINK_SELECTOR);
    const targetScrollYs = documentInfo.links.flatMap((link) =>
      link.targetY === null ? [] : [Math.max(0, link.targetY - input.viewport.height * 0.2)],
    );
    const plan = buildScrollPlan(documentInfo.maxScrollY, input.viewport.height, targetScrollYs);
    const scrolls: ScrollCoverageManifest["scrolls"] = [];
    const scrollDirectory = path.join(input.outputDir, "scroll");
    fs.mkdirSync(scrollDirectory, { recursive: true });
    for (const request of plan) {
      const actual = await waitForScrollSettled(page, request.requested);
      const capturePath = path.join(
        scrollDirectory,
        `${scrolls.length.toString().padStart(3, "0")}.png`,
      );
      await page.screenshot({ path: capturePath, fullPage: false });
      scrolls.push({ ...request, actual, capturePath });
    }
    assertScrollCoverage(
      scrolls.map((item) => item.actual),
      documentInfo.maxScrollY,
      input.viewport.height,
    );
    const toc: TocEvidence[] = [];
    const links = page.locator(TOC_LINK_SELECTOR);
    for (let index = 0; index < documentInfo.links.length; index += 1) {
      const source = documentInfo.links[index];
      if (!source) continue;
      await page.evaluate(() => window.scrollTo(0, 0));
      await links.nth(index).click({ noWaitAfter: true });
      await waitForAnchorSettled(page, source.href);
      const evidence = await page.evaluate(() => {
        const hash = window.location.hash;
        const id = decodeURIComponent(hash.replace(/^#/, ""));
        const target = id ? document.getElementById(id) : null;
        const rect = target?.getBoundingClientRect() ?? null;
        return {
          hash,
          actualScrollY: window.scrollY,
          targetTop: rect?.top ?? null,
          targetVisible: rect !== null && rect.bottom >= 0 && rect.top <= window.innerHeight,
        };
      });
      const capturePath = path.join(
        input.outputDir,
        "toc",
        `${String(index).padStart(3, "0")}.png`,
      );
      fs.mkdirSync(path.dirname(capturePath), { recursive: true });
      await page.screenshot({ path: capturePath, fullPage: false });
      toc.push({
        index,
        href: source.href,
        text: source.text,
        targetId: source.targetId,
        targetExists: source.targetExists,
        ...evidence,
        requestedScrollY: source.targetY ?? 0,
        capturePath,
        viewport: input.viewport,
      });
    }
    await context.close();
    const after = sha256File(htmlPath);
    if (before !== after) throw new Error("HTML input changed while measuring scroll coverage");
    const entries = toc.map((entry) => ({
      destination: entry.targetId ?? entry.href.replace(/^#/, ""),
      hash: entry.hash,
      scrollY: entry.actualScrollY,
      capturePath: entry.capturePath,
      targetVisible: entry.targetVisible,
    }));
    const manifest: ScrollCoverageManifest = {
      htmlPath,
      htmlSha256: after,
      viewport: { ...input.viewport, scale: 1 },
      scrollHeight: documentInfo.scrollHeight,
      maxScrollY: documentInfo.maxScrollY,
      scrolls,
      toc,
      entries,
      coverage: {
        positions: scrolls.map((item) => item.actual),
        gapLimit: input.viewport.height * 0.8,
      },
    };
    fs.mkdirSync(input.outputDir, { recursive: true });
    fs.writeFileSync(
      path.join(input.outputDir, "scroll-coverage.json"),
      `${JSON.stringify(manifest, null, 2)}\n`,
    );
    return manifest;
  } finally {
    await browser.close();
  }
}
