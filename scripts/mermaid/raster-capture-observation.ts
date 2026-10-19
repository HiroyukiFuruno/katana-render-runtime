import type { PageHandle } from "./official-renderer-types";
import { type RasterCaptureState, rasterSha256 } from "./raster-production-binding";

export interface CaptureSnapshot {
  domHtml: string;
  svgHtml: string;
  bounds: RasterCaptureState["bounds"];
  viewport: RasterCaptureState["viewport"];
  deviceScaleFactor: number;
  theme: string;
  fontsReady: boolean;
  imagesDecoded: boolean;
}

export interface LoadedSvgObservation {
  policy: "browser-domparser-html-svg-outerhtml-v1";
  inputSha256: string;
  expectedSvgSerialization: string;
  assignedSvgSerialization: string;
}

export type SvgCaptureSizingMode = "officialFallback" | "strictViewBox";

export function resizeSvgCapture(
  page: Pick<PageHandle, "evaluate">,
  strict: boolean,
  mode: SvgCaptureSizingMode,
): Promise<void> {
  return page.evaluate(
    ({ strict, mode }) => {
      const svgElement = document.querySelector("#diagram svg") as SVGSVGElement;
      const viewBox = String(
        mode === "strictViewBox"
          ? svgElement.getAttribute("viewBox")
          : (svgElement.getAttribute("viewBox") ?? ""),
      )
        .split(/\s+/)
        .map((value) => Number(value));
      const dimensionAt = (values: number[], index: number): number => {
        const value = values.at(index);
        if (mode === "strictViewBox" && (value === undefined || !Number.isFinite(value))) {
          throw new Error("SVG viewBox is invalid");
        }
        return value === undefined || !Number.isFinite(value) ? 0 : value;
      };
      const attributeDimension = (name: string): number => {
        const value = String(svgElement.getAttribute(name) ?? "").replace(/px$/, "");
        const parsed = Number(value);
        return Number.isFinite(parsed) ? parsed : 0;
      };
      const box = mode === "officialFallback" ? svgElement.getBoundingClientRect() : null;
      const positiveDimension = (values: number[]): number =>
        values.find((value) => value > 0) ?? 1;
      const width = Math.ceil(
        mode === "strictViewBox"
          ? dimensionAt(viewBox, 2)
          : positiveDimension([
              dimensionAt(viewBox, 2),
              attributeDimension("width"),
              box?.width ?? 0,
            ]),
      );
      const height = Math.ceil(
        mode === "strictViewBox"
          ? dimensionAt(viewBox, 3)
          : positiveDimension([
              dimensionAt(viewBox, 3),
              attributeDimension("height"),
              box?.height ?? 0,
            ]),
      );
      if (!strict) {
        svgElement.setAttribute("width", String(width));
        svgElement.setAttribute("height", String(height));
        svgElement.style.maxWidth = `${width}px`;
      }
      const capture = document.getElementById("capture") as HTMLElement;
      capture.style.width = `${width + 24}px`;
      capture.style.height = `${height + 24}px`;
      const naturalWidth = dimensionAt(viewBox, 2),
        naturalHeight = dimensionAt(viewBox, 3);
      const naturalizeDiagram = (diagram: HTMLElement) => {
        if (strict && naturalWidth > 0 && naturalHeight > 0) {
          diagram.style.width = `${naturalWidth}px`;
          diagram.style.height = `${naturalHeight}px`;
        }
      };
      naturalizeDiagram(document.getElementById("diagram") as HTMLElement);
    },
    { strict, mode },
  );
}

// 同じbrowser realmで期待rawを別documentへparseし、実代入DOMから独立した期待値を得る。
export async function assignStrictSvg(
  page: PageHandle,
  rawSvg: string,
): Promise<LoadedSvgObservation> {
  const observed = await page.evaluate((value) => {
    const expectedDocument = new DOMParser().parseFromString(
      `<div id="expected-diagram">${value}</div>`,
      "text/html",
    );
    const expected = expectedDocument.getElementById("expected-diagram");
    const diagram = document.getElementById("diagram");
    const svgOnly = (element: Element | null) => {
      if (
        element === null ||
        element.children.length !== 1 ||
        Array.from(element.childNodes).some(
          (child) => child.nodeType === Node.TEXT_NODE && (child.textContent ?? "").trim() !== "",
        )
      ) {
        throw new Error("Strict SVG input must parse to exactly one SVG root");
      }
      const svg = element.firstElementChild;
      if (
        svg === null ||
        svg.namespaceURI !== "http://www.w3.org/2000/svg" ||
        svg.localName !== "svg"
      ) {
        throw new Error("Strict SVG DOM root is invalid");
      }
      return svg.outerHTML;
    };
    if (
      expectedDocument.body.children.length !== 1 ||
      expected?.parentElement !== expectedDocument.body ||
      expectedDocument.head.children.length !== 0 ||
      diagram?.tagName !== "DIV"
    ) {
      throw new Error("Strict SVG parsing context differs from capture DIV");
    }
    const expectedSvgSerialization = svgOnly(expected);
    diagram.innerHTML = value;
    const assignedSvgSerialization = svgOnly(diagram);
    if (assignedSvgSerialization !== expectedSvgSerialization)
      throw new Error("Assigned SVG differs from expected parsed input");
    return { expectedSvgSerialization, assignedSvgSerialization };
  }, rawSvg);
  return {
    policy: "browser-domparser-html-svg-outerhtml-v1",
    inputSha256: rasterSha256(Buffer.from(rawSvg)),
    ...observed,
  };
}

export function assertLoadedSvgObservation(expectedInput: Buffer, loaded: LoadedSvgObservation) {
  if (
    loaded.policy !== "browser-domparser-html-svg-outerhtml-v1" ||
    loaded.inputSha256 !== rasterSha256(expectedInput) ||
    loaded.expectedSvgSerialization === "" ||
    loaded.expectedSvgSerialization !== loaded.assignedSvgSerialization
  ) {
    throw new Error("Loaded SVG differs from expected input/DOM binding");
  }
}

export function observeCaptureSnapshot(page: PageHandle): Promise<CaptureSnapshot> {
  return page.evaluate(async () => {
    await document.fonts.ready;
    const capture = document.getElementById("capture"),
      svg = document.querySelector("#diagram svg");
    if (capture === null || svg === null)
      throw new Error("Strict raster capture DOM is incomplete");
    const images = Array.from(svg.querySelectorAll("image"));
    const decoded = await Promise.all(
      images.map(async (element) => {
        const href =
          element.getAttribute("href") ??
          element.getAttributeNS("http://www.w3.org/1999/xlink", "href");
        if (href === null) throw new Error("Strict raster image has no source");
        const image = new Image();
        image.src = href;
        await image.decode();
        return image.complete && image.naturalWidth > 0 && image.naturalHeight > 0;
      }),
    );
    const box = capture.getBoundingClientRect();
    return {
      domHtml: capture.outerHTML,
      svgHtml: svg.outerHTML,
      bounds: { x: box.x, y: box.y, width: box.width, height: box.height },
      viewport: { width: window.innerWidth, height: window.innerHeight },
      deviceScaleFactor: window.devicePixelRatio,
      theme: getComputedStyle(document.documentElement).colorScheme,
      fontsReady: document.fonts.status === "loaded",
      imagesDecoded: decoded.every(Boolean),
    };
  });
}

export function closeCaptureObservation(
  expectedInput: Buffer,
  loaded: LoadedSvgObservation,
  before: CaptureSnapshot,
  after: CaptureSnapshot,
): RasterCaptureState {
  assertLoadedSvgObservation(expectedInput, loaded);
  if (
    before.svgHtml !== loaded.expectedSvgSerialization ||
    after.svgHtml !== loaded.expectedSvgSerialization
  ) {
    throw new Error("Captured SVG differs from expected parsed input");
  }
  if (
    before.domHtml !== after.domHtml ||
    before.svgHtml !== after.svgHtml ||
    JSON.stringify(before.bounds) !== JSON.stringify(after.bounds) ||
    JSON.stringify(before.viewport) !== JSON.stringify(after.viewport) ||
    before.deviceScaleFactor !== after.deviceScaleFactor ||
    before.theme !== after.theme ||
    !before.fontsReady ||
    !after.fontsReady ||
    !before.imagesDecoded ||
    !after.imagesDecoded
  ) {
    throw new Error("Strict raster capture observation changed or readiness failed");
  }
  return {
    domSha256Before: rasterSha256(Buffer.from(before.domHtml)),
    domSha256After: rasterSha256(Buffer.from(after.domHtml)),
    svg: {
      policy: loaded.policy,
      inputSha256: rasterSha256(expectedInput),
      expectedDomSha256: rasterSha256(Buffer.from(loaded.expectedSvgSerialization)),
      assignedDomSha256: rasterSha256(Buffer.from(loaded.assignedSvgSerialization)),
      beforeDomSha256: rasterSha256(Buffer.from(before.svgHtml)),
      endDomSha256: rasterSha256(Buffer.from(after.svgHtml)),
    },
    bounds: before.bounds,
    viewport: before.viewport,
    deviceScaleFactor: before.deviceScaleFactor,
    theme: before.theme,
    resized: false,
    fontsReady: true,
    imagesDecoded: true,
  };
}
