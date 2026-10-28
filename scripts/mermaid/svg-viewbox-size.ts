import { DOMParser } from "@xmldom/xmldom";

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
