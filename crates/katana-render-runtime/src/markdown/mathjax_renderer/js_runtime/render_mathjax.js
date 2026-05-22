import { liteAdaptor } from "@mathjax/src/mjs/adaptors/liteAdaptor.js";
import { RegisterHTMLHandler } from "@mathjax/src/mjs/handlers/html.js";
import { TeX } from "@mathjax/src/mjs/input/tex.js";
import { mathjax } from "@mathjax/src/mjs/mathjax.js";
import { SVG } from "@mathjax/src/mjs/output/svg.js";

const adaptor = liteAdaptor();
RegisterHTMLHandler(adaptor);

const tex = new TeX({ packages: ["base"] });
const svg = new SVG({ fontCache: "none" });
const html = mathjax.document("", { InputJax: tex, OutputJax: svg });

globalThis.katanaRunMathJaxRuntime = function katanaRunMathJaxRuntime(request) {
  try {
    const node = html.convert(request.source, {
      display: request.display === true,
      em: 16,
      ex: 8,
      containerWidth: 1280,
    });
    const output = adaptor.outerHTML(node);
    const error = katanaMathJaxError(output);
    if (error) {
      return JSON.stringify({ kind: "error", message: error });
    }
    return JSON.stringify({
      kind: "svg",
      svg: katanaMathJaxSvg(node, request),
    });
  } catch (error) {
    return JSON.stringify({
      kind: "error",
      message: error && error.stack ? error.stack : String(error),
    });
  }
};

function katanaMathJaxError(output) {
  if (output.includes("data-mjx-error") || output.includes("merror")) {
    return "MathJax failed to render TeX";
  }
  return "";
}

function katanaMathJaxSvg(node, request) {
  const svgNodes = adaptor.tags(node, "svg");
  const svgNode = svgNodes[0];
  if (svgNode === undefined) {
    return adaptor.outerHTML(node);
  }
  return katanaApplyMathJaxTheme(adaptor.outerHTML(svgNode), request);
}

function katanaApplyMathJaxTheme(output, request) {
  const style = `color: ${request.text}; background: transparent;`;
  if (output.includes(" style=")) {
    return output.replace(/ style="([^"]*)"/, ` style="$1 ${style}"`);
  }
  return output.replace("<svg", `<svg style="${style}"`);
}
