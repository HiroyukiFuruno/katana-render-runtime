import { DOMParser } from "@xmldom/xmldom";
import { defaultTreeAdapter, html, parseFragment } from "parse5";

export function svgViewBoxSize(svg: string): { width: number; height: number } | undefined {
  const document = new DOMParser({
      onError: (level, message) => {
        throw new Error(`Native frame SVG parse ${level}: ${message}`);
      },
    }).parseFromString(svg, "image/svg+xml"),
    root = document.documentElement;
  if (
    document.doctype !== null ||
    root === null ||
    root.localName !== "svg" ||
    (root.namespaceURI !== null && root.namespaceURI !== "http://www.w3.org/2000/svg") ||
    document.getElementsByTagName("parsererror").length > 0
  ) {
    throw new Error("Native frame input requires one well-formed SVG root");
  }
  const viewBox = root.getAttribute("viewBox");
  return parseViewBoxSize(viewBox);
}

export function rawHtmlSvgViewBoxSize(svg: string): { width: number; height: number } | undefined {
  const context = defaultTreeAdapter.createElement("div", html.NS.HTML, []);
  const fragment = parseFragment(context, svg, {});
  const elements = fragment.childNodes.filter((node) => "tagName" in node);
  if (
    elements.length !== 1 ||
    fragment.childNodes.some((node) => "value" in node && node.value.trim() !== "")
  ) {
    throw new Error("Raw HTML input requires one SVG root");
  }
  const root = elements[0];
  if (root === undefined || root.tagName !== "svg" || root.namespaceURI !== html.NS.SVG) {
    throw new Error("Raw HTML input requires one SVG root");
  }
  const xmlns = root.attrs.find((attribute) => attribute.name === "xmlns");
  if (xmlns !== undefined && xmlns.value !== html.NS.SVG) {
    throw new Error("Raw HTML SVG namespace declaration is invalid");
  }
  return parseViewBoxSize(
    root.attrs.find((attribute) => attribute.name === "viewBox")?.value ?? null,
  );
}

function parseViewBoxSize(viewBox: string | null): { width: number; height: number } | undefined {
  if (viewBox === null) return undefined;
  const values = viewBox
    .trim()
    .split(/[\s,]+/)
    .map(Number);
  if (
    values.length !== 4 ||
    values.some((value) => !Number.isFinite(value)) ||
    values[2] === undefined ||
    values[3] === undefined ||
    values[2] <= 0 ||
    values[3] <= 0
  ) {
    throw new Error("Native frame input has an invalid SVG viewBox");
  }
  return { width: values[2], height: values[3] };
}
