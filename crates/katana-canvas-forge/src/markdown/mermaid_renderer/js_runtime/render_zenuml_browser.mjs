import { execSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

class PlaywrightLoader {
  async load() {
    try {
      return await import("playwright");
    } catch {
      const cliPath = execSync("which playwright", { encoding: "utf8" }).trim();
      const realPath = fs.realpathSync(cliPath);
      const modulePath = path.join(path.dirname(realPath), "index.mjs");
      return import(pathToFileURL(modulePath).href);
    }
  }
}

const request = JSON.parse(fs.readFileSync(0, "utf8"));
const { chromium } = await new PlaywrightLoader().load();
const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage({
    viewport: { width: 1520, height: 845 },
    deviceScaleFactor: 1,
  });
  await page.setContent(baseHtml(request), { waitUntil: "load" });
  await installStorage(page);
  await page.addScriptTag({ path: request.mermaidJs });
  await page.addScriptTag({ path: request.zenumlJs });
  const svg = await renderZenuml(page, request);
  console.log(svg);
  await page.close();
} finally {
  await browser.close();
}

function baseHtml(value) {
  return `<!doctype html><html><head><meta charset="utf-8"><style>
html,body{margin:0;background:${value.background};color:${value.text};font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}
#diagram{width:1520px;min-height:845px}
</style></head><body><div id="diagram"></div></body></html>`;
}

function installStorage(page) {
  return page.evaluate(() => {
    const storage = () => {
      const values = new Map();
      return {
        get length() {
          return values.size;
        },
        clear() {
          values.clear();
        },
        getItem(key) {
          return values.get(String(key)) ?? null;
        },
        key(index) {
          return Array.from(values.keys()).at(index) ?? null;
        },
        removeItem(key) {
          values.delete(String(key));
        },
        setItem(key, value) {
          values.set(String(key), String(value));
        },
      };
    };
    Object.defineProperty(window, "localStorage", { value: storage(), configurable: true });
    Object.defineProperty(window, "sessionStorage", { value: storage(), configurable: true });
  });
}

function renderZenuml(page, value) {
  return page.evaluate(async (request) => {
    const mermaidValue = window.mermaid;
    const zenumlDiagram = window["mermaid-zenuml"];
    if (!zenumlDiagram) {
      throw new Error("ZenUML runtime asset was not registered");
    }
    mermaidValue.initialize({
      startOnLoad: false,
      securityLevel: "loose",
      htmlLabels: false,
      theme: request.theme,
      flowchart: { htmlLabels: false, useMaxWidth: false },
      sequence: { useMaxWidth: false },
      themeVariables: {
        background: request.background,
        mainBkg: request.fill,
        primaryColor: request.fill,
        primaryTextColor: request.text,
        primaryBorderColor: request.stroke,
        secondaryColor: request.fill,
        secondaryTextColor: request.text,
        secondaryBorderColor: request.stroke,
        tertiaryColor: request.fill,
        tertiaryTextColor: request.text,
        tertiaryBorderColor: request.stroke,
        nodeTextColor: request.text,
        lineColor: request.arrow,
        textColor: request.text,
      },
    });
    await mermaidValue.registerExternalDiagrams([zenumlDiagram]);
    const result = await mermaidValue.render(request.svgId, request.source);
    return normalizeSvgMetadata(result.svg);

    function normalizeSvgMetadata(svg) {
      const pageStyle = zenumlPageStyle();
      const template = document.createElement("template");
      template.innerHTML = svg.trim();
      const svgElement = template.content.querySelector("svg");
      if (!svgElement) {
        return svg;
      }
      inlineForeignObjectStyle(svgElement, pageStyle);
      const size = svgSize(svgElement);
      svgElement.setAttribute("width", String(size.width));
      svgElement.setAttribute("height", String(size.height));
      if (!svgElement.getAttribute("viewBox")) {
        svgElement.setAttribute("viewBox", `0 0 ${size.width} ${size.height}`);
      }
      return new XMLSerializer().serializeToString(svgElement);
    }

    function zenumlPageStyle() {
      return Array.from(document.querySelectorAll("style"))
        .map((it) => it.textContent ?? "")
        .filter((it) => it.trim().length > 0)
        .join("\n");
    }

    function inlineForeignObjectStyle(svgElement, pageStyle) {
      const root = svgElement.querySelector("foreignObject")?.firstElementChild;
      if (!root || pageStyle.trim().length === 0) {
        return;
      }
      const styleElement = document.createElement("style");
      styleElement.textContent = pageStyle;
      root.prepend(styleElement);
    }

    function svgSize(svgElement) {
      const viewBox = viewBoxSize(svgElement.getAttribute("viewBox"));
      return {
        width:
          viewBox?.width ??
          numericDimension(svgElement.getAttribute("width")) ??
          numericDimension(svgElement.style.width) ??
          positiveDimension(svgElement.getBoundingClientRect().width) ??
          1,
        height:
          viewBox?.height ??
          numericDimension(svgElement.getAttribute("height")) ??
          numericDimension(svgElement.style.height) ??
          positiveDimension(svgElement.getBoundingClientRect().height) ??
          1,
      };
    }

    function viewBoxSize(value) {
      const parts = String(value ?? "")
        .trim()
        .split(/\s+/)
        .map((it) => Number(it));
      if (parts.length !== 4 || parts.some((it) => !Number.isFinite(it))) {
        return null;
      }
      return { width: parts[2], height: parts[3] };
    }

    function numericDimension(value) {
      const trimmed = String(value ?? "").trim();
      if (!trimmed || trimmed.endsWith("%")) {
        return null;
      }
      const match = trimmed.match(/^-?\d+(?:\.\d+)?/);
      if (!match) {
        return null;
      }
      const numberValue = Number(match[0]);
      return positiveDimension(numberValue);
    }

    function positiveDimension(value) {
      return Number.isFinite(value) && value > 0 ? Math.ceil(value) : null;
    }
  }, value);
}
