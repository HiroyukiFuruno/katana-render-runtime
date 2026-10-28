import { pathToFileURL } from "node:url";
import type { Browser, Page } from "playwright";
import { waitForStrictSvgExternalImages } from "../mermaid/strict-svg-external-images";
import type { SvgFontRequest } from "./actual-svg-font-request";

export async function capturePage(browser: Browser, request: SvgFontRequest) {
  const context = await browser.newContext({
    viewport: { width: request.viewport.width, height: request.viewport.height },
    deviceScaleFactor: request.viewport.deviceScaleFactor,
  });
  try {
    const page = await context.newPage();
    await page.goto(pathToFileURL(request.input.path).href, { waitUntil: "networkidle" });
    const viewport = await page.evaluate(async () => {
      await document.fonts.ready;
      if (document.fonts.status !== "loaded") throw new Error("font loading incomplete");
      await Promise.all(
        Array.from(document.images, async (image) => {
          await image.decode();
          if (!image.complete || image.naturalWidth <= 0 || image.naturalHeight <= 0)
            throw new Error("HTML image loading incomplete");
        }),
      );
      if (
        document.querySelectorAll("svg image").length !==
        document.querySelectorAll("#diagram image").length
      )
        throw new Error("SVG images outside the existing resource readiness scope");
      return { width: innerWidth, height: innerHeight, deviceScaleFactor: devicePixelRatio };
    });
    if (
      viewport.width !== request.viewport.width ||
      viewport.height !== request.viewport.height ||
      viewport.deviceScaleFactor !== request.viewport.deviceScaleFactor
    )
      throw new Error("actual browser viewport differs from request");
    await waitForStrictSvgExternalImages(page);
    const observation = await inspectTarget(page, request.selector);
    const platformFonts = await targetPlatformFonts(page, request.selector);
    const png = await page.screenshot();
    return { observation, platformFonts, png, viewport, browserVersion: browser.version() };
  } finally {
    await context.close();
  }
}

async function inspectTarget(page: Page, selector: string) {
  const target = page.locator(selector);
  if ((await target.count()) !== 1) throw new Error("selector must match exactly one target");
  return target.evaluate((element) => {
    if (!(element instanceof SVGTextContentElement) || !(element instanceof SVGGraphicsElement))
      throw new Error("target is not SVG text");
    const style = getComputedStyle(element);
    const box = element.getBBox();
    const matrix = element.getCTM();
    const screenMatrix = element.getScreenCTM();
    if (!matrix || !screenMatrix) throw new Error("SVG target transform unavailable");
    const matrixValues = (value: DOMMatrix) => [
      value.a,
      value.b,
      value.c,
      value.d,
      value.e,
      value.f,
    ];
    const positions = Array.from({ length: element.getNumberOfChars() }, (_, index) => {
      const point = element.getStartPositionOfChar(index);
      return { index, x: point.x, y: point.y };
    });
    return {
      text: element.textContent,
      computed: {
        family: style.fontFamily,
        size: style.fontSize,
        weight: style.fontWeight,
        fontStyle: style.fontStyle,
        fontStretch: style.fontStretch,
        variationSettings: style.fontVariationSettings,
        dominantBaseline: style.dominantBaseline,
        direction: style.direction,
        writingMode: style.writingMode,
      },
      bbox: { x: box.x, y: box.y, width: box.width, height: box.height },
      ctm: matrixValues(matrix),
      screenCtm: matrixValues(screenMatrix),
      characterIndexConvention: "SVG getStartPositionOfChar indices; not glyph IDs",
      positions,
    };
  });
}

async function targetPlatformFonts(page: Page, selector: string) {
  const session = await page.context().newCDPSession(page);
  try {
    await session.send("DOM.enable");
    await session.send("CSS.enable");
    const { root } = await session.send("DOM.getDocument", { depth: -1, pierce: true });
    const { nodeId } = await session.send("DOM.querySelector", { nodeId: root.nodeId, selector });
    if (!nodeId) throw new Error("target CDP node unavailable");
    const { node } = await session.send("DOM.describeNode", { nodeId, depth: -1, pierce: true });
    const ids = textNodes(node);
    if (!ids.length) throw new Error("target has no CDP text nodes");
    const rows = [];
    for (const id of ids)
      rows.push({
        nodeId: id,
        ...(await session.send("CSS.getPlatformFontsForNode", { nodeId: id })),
      });
    return rows;
  } finally {
    await session.detach();
  }
}

function textNodes(node: {
  nodeId: number;
  nodeType: number;
  children?: readonly unknown[];
}): number[] {
  const result = node.nodeType === 3 ? [node.nodeId] : [];
  for (const child of node.children ?? []) result.push(...textNodes(child as typeof node));
  return result;
}
