import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { parseArgs } from "node:util";
import { chromium, type Page } from "playwright";
import { z } from "zod";
import { waitForStrictSvgExternalImages } from "./strict-svg-external-images";

const WIDTH = 300;
const VIEWPORT_HEIGHT = 300;
const ORIGIN = "http://krr-marker.invalid";
const ROOT = path.resolve("tests/fixtures/svg-engine/upstream/resvg/tests");
const INVENTORY = path.resolve(
  "tests/fixtures/svg-engine/browser-corrections/marker/capture-cases.json",
);
const sha = z.string().regex(/^[a-f0-9]{64}$/);
const item = z.strictObject({
  input: z.string(),
  inputSha256: sha,
  upstreamPngSha256: sha,
  requestedWidth: z.literal(WIDTH),
  requestedHeight: z.number().int().positive().max(VIEWPORT_HEIGHT),
});
const inventory = z.strictObject({
  schema: z.literal("krr-svg-marker-capture-cases-v1"),
  requestedWidth: z.literal(WIDTH),
  cases: z.array(item).length(48),
});
type CaptureCase = z.infer<typeof item>;
type Pin = { path: string; sha256: string };
const hash = (data: Uint8Array) => createHash("sha256").update(data).digest("hex");

export function fixturePath(root: string, relative: string): string {
  if (
    !relative ||
    path.isAbsolute(relative) ||
    relative.includes("\\") ||
    relative.split("/").some((part) => !part || part === "." || part === "..")
  ) {
    throw new Error("invalid fixture path");
  }
  const file = path.resolve(root, relative);
  if (
    !file.startsWith(`${root}${path.sep}`) ||
    fs.realpathSync(file) !== file ||
    !fs.statSync(file).isFile()
  ) {
    throw new Error("fixture path escapes root or is not a regular file");
  }
  return file;
}

export function parseMarkerInventory(value: unknown): z.infer<typeof inventory> {
  const parsed = inventory.parse(value);
  const names = parsed.cases.map((entry) => entry.input);
  if (
    new Set(names).size !== names.length ||
    names.some((name) => !name.startsWith("tests/painting/") || !name.endsWith(".svg"))
  ) {
    throw new Error("invalid marker capture inventory paths");
  }
  if (
    parsed.cases.some(
      (entry) =>
        entry.requestedHeight !==
        (entry.input === "tests/painting/marker/percent-values.svg" ? 150 : 300),
    ) ||
    !names.includes("tests/painting/marker/percent-values.svg")
  ) {
    throw new Error("invalid marker capture inventory dimensions");
  }
  return parsed;
}

async function captureSizedSvg(page: Page, width: number, height: number): Promise<Buffer> {
  const aspectMatches = await page.locator("#diagram > svg").evaluate(
    (svg, dimensions) => {
      if (!(svg instanceof SVGSVGElement)) return false;
      const viewBox = svg.viewBox.baseVal;
      return (
        viewBox.width > 0 &&
        viewBox.height > 0 &&
        dimensions.width * viewBox.height === dimensions.height * viewBox.width
      );
    },
    { width, height },
  );
  if (!aspectMatches) throw new Error("SVG viewBox aspect differs from requested dimensions");
  const box = await page.locator("#diagram > svg").boundingBox();
  if (!box || box.width !== width || box.height !== height) throw new Error("SVG viewport differs");
  const png = await page.locator("#diagram").screenshot({ omitBackground: true });
  if (png.readUInt32BE(16) !== width || png.readUInt32BE(20) !== height)
    throw new Error("PNG dimensions differ from requested dimensions");
  return png;
}

export async function captureMarkerFixture(
  page: Page,
  root: string,
  entry: Pick<CaptureCase, "input" | "inputSha256" | "upstreamPngSha256"> &
    Partial<Pick<CaptureCase, "requestedWidth" | "requestedHeight">>,
) {
  const width = entry.requestedWidth ?? WIDTH;
  const height = entry.requestedHeight ?? VIEWPORT_HEIGHT;
  const source = fs.readFileSync(fixturePath(root, entry.input));
  if (hash(source) !== entry.inputSha256) throw new Error("SVG input SHA differs");
  const fontPath = fixturePath(root, "fonts/NotoSans-Regular.ttf");
  const font = fs.readFileSync(fontPath);
  const resources: Pin[] = [];
  const rejected: string[] = [];
  const mainUrl = `${ORIGIN}/${entry.input}`;
  await page.route("**/*", async (route) => {
    const url = new URL(route.request().url());
    try {
      if (url.origin !== ORIGIN || url.search || url.hash || route.request().method() !== "GET") {
        throw new Error("unexpected network request");
      }
      if (url.href === mainUrl) {
        const body = `<!doctype html><meta charset="utf-8"><style>html,body{margin:0;background:transparent}#diagram,#diagram>svg{width:${width}px;height:${height}px;display:block}</style><div id="diagram">${source.toString("utf8")}</div>`;
        await route.fulfill({ status: 200, contentType: "text/html", body });
      } else {
        const relative = decodeURIComponent(url.pathname.slice(1));
        const resource = fs.readFileSync(fixturePath(root, relative));
        if (!relative.startsWith("resources/")) throw new Error("unexpected SVG resource");
        resources.push({ path: relative, sha256: hash(resource) });
        await route.fulfill({ status: 200, contentType: "image/svg+xml", body: resource });
      }
    } catch (error) {
      rejected.push(error instanceof Error ? error.message : "invalid request");
      await route.abort();
    }
  });
  try {
    await page.goto(mainUrl, { waitUntil: "load" });
    const fontLoaded = await page.evaluate(async (base64) => {
      const bytes = Uint8Array.from(atob(base64), (character) => character.charCodeAt(0));
      const face = await new FontFace("Noto Sans", bytes).load();
      document.fonts.add(face);
      await document.fonts.ready;
      return face.status === "loaded" && document.fonts.check('8px "Noto Sans"', "Text");
    }, font.toString("base64"));
    if (!fontLoaded) throw new Error("Noto Sans fixture font did not load");
    await waitForStrictSvgExternalImages(page);
    if (rejected.length) throw new Error(`fixture request rejected: ${rejected.join(", ")}`);
    const png = await captureSizedSvg(page, width, height);
    if (hash(fs.readFileSync(fixturePath(root, entry.input))) !== entry.inputSha256) {
      throw new Error("SVG input changed during capture");
    }
    for (const resource of resources) {
      if (hash(fs.readFileSync(fixturePath(root, resource.path))) !== resource.sha256) {
        throw new Error("SVG resource changed during capture");
      }
    }
    if (hash(fs.readFileSync(fontPath)) !== hash(font))
      throw new Error("fixture font changed during capture");
    return {
      png,
      resources,
      font: { path: "fonts/NotoSans-Regular.ttf", sha256: hash(font) },
      fontLoaded,
    };
  } finally {
    await page.unroute("**/*");
  }
}

export async function captureMarkerInventory(output: string): Promise<void> {
  const tmp = fs.realpathSync(path.resolve("tmp"));
  if (
    !path.isAbsolute(output) ||
    path.dirname(output) !== fs.realpathSync(path.dirname(output)) ||
    !output.startsWith(`${tmp}${path.sep}`)
  )
    throw new Error("output must be a canonical path under tmp");
  const inventoryBytes = fs.readFileSync(INVENTORY);
  const value: unknown = JSON.parse(inventoryBytes.toString("utf8"));
  const entries = parseMarkerInventory(value);
  fs.mkdirSync(output);
  const server = await chromium.launchServer({ channel: "chrome", headless: true });
  try {
    const spawned = server.process().spawnfile;
    if (!path.isAbsolute(spawned)) throw new Error("launched Chrome executable is not absolute");
    const executable = fs.realpathSync(spawned);
    const tool = { executable, sha256: hash(fs.readFileSync(executable)) };
    const browser = await chromium.connect(server.wsEndpoint());
    try {
      const page = await browser.newPage({
        viewport: { width: WIDTH, height: VIEWPORT_HEIGHT },
        deviceScaleFactor: 1,
      });
      try {
        const captures = [];
        for (const [index, entry] of entries.cases.entries()) {
          const upstream = fixturePath(ROOT, entry.input.replace(/\.svg$/, ".png"));
          if (hash(fs.readFileSync(upstream)) !== entry.upstreamPngSha256)
            throw new Error("upstream PNG SHA differs");
          const result = await captureMarkerFixture(page, ROOT, entry);
          const pngPath = `${String(index).padStart(2, "0")}.png`;
          fs.writeFileSync(path.join(output, pngPath), result.png, { flag: "wx" });
          captures.push({
            ...entry,
            png: { path: pngPath, sha256: hash(result.png) },
            resources: result.resources,
            font: result.font,
          });
        }
        if (hash(fs.readFileSync(executable)) !== tool.sha256)
          throw new Error("Chrome executable changed during capture");
        fs.writeFileSync(
          path.join(output, "manifest.json"),
          JSON.stringify(
            {
              schema: "krr-svg-marker-browser-capture-v1",
              diagnosticOnly: true,
              requestedWidth: WIDTH,
              inventorySha256: hash(inventoryBytes),
              browserVersion: browser.version(),
              tool,
              captures,
            },
            null,
            2,
          ),
          { flag: "wx" },
        );
      } finally {
        await page.close();
      }
    } finally {
      await browser.close();
    }
  } finally {
    await server.close();
  }
}

if (import.meta.main) {
  const args = parseArgs({
    args: process.argv.slice(2),
    options: { output: { type: "string" } },
    strict: true,
  });
  if (!args.values.output) throw new Error("--output is required");
  await captureMarkerInventory(args.values.output);
}
