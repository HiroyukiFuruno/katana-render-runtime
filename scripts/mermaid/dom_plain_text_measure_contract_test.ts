import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { createContext, runInContext } from "node:vm";

const RUNTIME_DIRECTORY = new URL(
  "../../crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime/",
  import.meta.url,
);

function measurePlainText(diagramType: string, text: string) {
  const context = createContext({
    KatanaNode: class {},
    __katanaMermaidDiagramType: diagramType,
    textInput: text,
  });
  for (const name of ["dom_text_metrics.js", "dom_text_measure.js", "dom_canvas_context.js"]) {
    runInContext(readFileSync(new URL(name, RUNTIME_DIRECTORY), "utf8"), context);
  }
  return {
    value: runInContext("katanaMeasuredTextValue(textInput)", context) as string,
    domWidth: runInContext("katanaTextWidth(textInput)", context) as number,
    canvasWidth: runInContext('katanaCanvasTextWidth(textInput, "16px Arial")', context) as number,
  };
}

type SvgShapeCase = {
  tag: "circle" | "ellipse" | "rect";
  attributes: Record<string, string>;
  expected: { x: number; y: number; width: number; height: number };
};

function measureSvgShapes(cases: SvgShapeCase[]) {
  const context = createContext({ KatanaNode: class {}, shapeCases: cases });
  for (const name of ["svg_transform_bounds.js", "dom_layout.js", "dom_measure.js"]) {
    runInContext(readFileSync(new URL(name, RUNTIME_DIRECTORY), "utf8"), context);
  }
  return runInContext(
    `shapeCases.map(({ tag, attributes }) => {
      const node = {
        localName: tag,
        getAttribute: (name) => attributes[name] ?? null,
        children: [],
      };
      return {
        visible: katanaVisibleMeasuredBox(node),
        accurate: katanaVisibleMeasuredBoxAccurate(node),
      };
    })`,
    context,
  ) as Array<{
    visible: { x: number; y: number; width: number; height: number };
    accurate: { x: number; y: number; width: number; height: number };
  }>;
}

test("round SVG shapes use cx/cy/radii before unrelated explicit box attributes", () => {
  const cases: SvgShapeCase[] = [
    {
      tag: "circle",
      attributes: { x: "100", y: "200", width: "14", height: "14", cx: "20", cy: "30", r: "7" },
      expected: { x: 13, y: 23, width: 14, height: 14 },
    },
    {
      tag: "ellipse",
      attributes: {
        x: "100",
        y: "200",
        width: "16",
        height: "10",
        cx: "20",
        cy: "30",
        rx: "8",
        ry: "5",
      },
      expected: { x: 12, y: 25, width: 16, height: 10 },
    },
    {
      tag: "rect",
      attributes: { x: "100", y: "200", width: "16", height: "10" },
      expected: { x: 100, y: 200, width: 16, height: 10 },
    },
  ];
  for (const [index, result] of measureSvgShapes(cases).entries()) {
    const current = cases[index];
    if (!current) throw new Error(`missing SVG shape case at index ${index}`);
    const expected = current.expected;
    expect(result.visible).toMatchObject(expected);
    expect(result.accurate).toMatchObject(expected);
  }
});

type SvgMetricCell = { x: number; y: number; width: number; height: number };
type SvgMetrics = {
  advance: number;
  ascent: number;
  descent: number;
  layoutAscent?: number | null | undefined;
  inkLeft?: number;
  inkTop?: number;
  inkRight?: number;
  inkBottom?: number;
};

function measureNativeSvgMetricCell(
  metrics: SvgMetrics,
  dominantBaseline = "auto",
  xHeight?: number,
): SvgMetricCell {
  const { context } = svgMetricContext(metrics, dominantBaseline, xHeight);
  for (const name of [
    "dom_css_style.js",
    "dom_text_metrics.js",
    "dom_text_font_context.js",
    "svg_transform_bounds.js",
    "dom_layout.js",
    "dom_text_measure.js",
  ]) {
    runInContext(readFileSync(new URL(name, RUNTIME_DIRECTORY), "utf8"), context);
  }
  const box = runInContext(
    'katanaAnchoredTextNodeBox(metricNode, "x", 10, 20)',
    context,
  ) as SvgMetricCell;
  return { x: box.x, y: box.y, width: box.width, height: box.height };
}

function measureSvgFallbackMetricCell(
  metrics: SvgMetrics,
  dominantBaseline: string,
  xHeight: number,
  positioned: boolean,
): { box: SvgMetricCell; nativeMetricCalls: number } {
  const { context, nativeMetricCalls } = svgMetricContext(
    metrics,
    dominantBaseline,
    xHeight,
    false,
  );
  for (const name of [
    "dom_css_style.js",
    "dom_text_metrics.js",
    "dom_text_font_context.js",
    "svg_transform_bounds.js",
    "dom_layout.js",
    "dom_text_measure.js",
    "dom_text_fragment_layout.js",
    "dom_text_run_lines.js",
  ]) {
    runInContext(readFileSync(new URL(name, RUNTIME_DIRECTORY), "utf8"), context);
  }
  const expression = positioned
    ? `(() => {
        const font = katanaResolvedNodeTextFont(metricNode);
        return katanaPositionedTextRunBox({ node: metricNode, font, x: 10, y: 20, width: 10 }, 0);
      })()`
    : 'katanaAnchoredTextNodeBox(metricNode, "x", 10, 20)';
  const box = runInContext(expression, context) as SvgMetricCell;
  return {
    box: { x: box.x, y: box.y, width: box.width, height: box.height },
    nativeMetricCalls: nativeMetricCalls.length,
  };
}

function svgMetricContext(
  metrics: SvgMetrics,
  dominantBaseline: string,
  xHeight?: number,
  nativeAdvanceEligible = true,
) {
  const nativeMetricCalls: number[] = [];
  const context = createContext({
    KatanaNode: class {},
    KATANA_HTML_NAMESPACE: "http://www.w3.org/1999/xhtml",
    __katanaMermaidDiagramType: "flowchart",
    __katanaMeasureSvgTextMetrics: () => {
      nativeMetricCalls.push(1);
      return metrics;
    },
    __katanaMeasureSvgFontXHeight: () => xHeight,
    parentNode: {
      nodeType: 1,
      parentNode: null,
      style: {
        getPropertyValue: (name: string) => {
          if (name === "dominant-baseline") return dominantBaseline;
          if (name === "letter-spacing" && !nativeAdvanceEligible) return "1px";
          return "";
        },
        getPropertyPriority: () => "",
      },
      getAttribute: () => null,
    },
    metricNode: {
      namespaceURI: "http://www.w3.org/2000/svg",
      nodeType: 1,
      localName: "text",
      parentNode: null as object | null,
      children: [],
      style: { getPropertyValue: () => "", getPropertyPriority: () => "" },
      getAttribute: () => "",
    },
  });
  runInContext("metricNode.parentNode = parentNode", context);
  return { context, nativeMetricCalls };
}

function mockFontStyle(values: Record<string, string>) {
  return {
    getPropertyValue: (name: string) => values[name] ?? "",
    getPropertyPriority: () => "",
  };
}

function svgExContext(
  size: string,
  xHeight: number | undefined,
  calls: unknown[][],
  inheritedSize = "16px",
) {
  const root = {
    nodeType: 1,
    parentNode: null as object | null,
    style: mockFontStyle({ "font-family": "Noto Sans", "font-size": inheritedSize }),
    getAttribute: () => null,
  };
  const node = {
    nodeType: 1,
    parentNode: root,
    style: mockFontStyle({ "font-size": size }),
    getAttribute: () => null,
  };
  return createContext({
    KatanaNode: class {},
    KATANA_HTML_NAMESPACE: "http://www.w3.org/1999/xhtml",
    metricNode: node,
    katanaCssText: () => "",
    katanaCascadedStyleValue: (target: typeof root, name: string) =>
      target.style.getPropertyValue(name),
    katanaResolvedTextFamilyValue: (_target: typeof root, value: string) => value,
    katanaMermaidNativeAdvanceRuntime: () => true,
    __katanaMeasureSvgFontXHeight: (
      family: string,
      fontSize: number,
      weight: number,
      italic: boolean,
    ) => {
      calls.push([family, fontSize, weight, italic]);
      return xHeight;
    },
  });
}

function resolveSvgExSize(size: string, xHeight: number | undefined) {
  const calls: unknown[][] = [];
  const context = svgExContext(size, xHeight, calls);
  for (const name of ["dom_text_font_context.js", "dom_text_measure.js"]) {
    runInContext(readFileSync(new URL(name, RUNTIME_DIRECTORY), "utf8"), context);
  }
  const result = runInContext(
    `({fontSize: katanaResolvedNodeTextFont(metricNode).size,
      lineHeightSize: katanaInheritedFontSize(metricNode)})`,
    context,
  ) as { fontSize: number; lineHeightSize: number };
  return { ...result, calls };
}

function resolveLegacySvgFontSize(size: string, inheritedSize: string) {
  const context = svgExContext(size, 7.5, [], inheritedSize);
  for (const name of ["dom_text_font_context.js", "dom_text_measure.js"]) {
    runInContext(readFileSync(new URL(name, RUNTIME_DIRECTORY), "utf8"), context);
  }
  return runInContext("katanaInheritedFontSize(metricNode)", context) as number;
}

test("DOM and Canvas measure entity-looking plain text literally for every diagram", () => {
  for (const diagram of ["block", "classDiagram", "c4Dynamic", "flowchart", "treemap"]) {
    const nbsp = measurePlainText(diagram, "\u00A0");
    for (const literal of ["&nbsp;", "&amp;nbsp;", "&#160;", "&#xA0;"]) {
      const measured = measurePlainText(diagram, literal);
      expect(measured.value).toBe(literal);
      expect(measured.domWidth).toBeGreaterThan(nbsp.domWidth);
      expect(measured.canvasWidth).toBeGreaterThan(nbsp.canvasWidth);
    }
  }
});

test("already decoded NBSP has the same advance as a space without changing its text", () => {
  for (const diagram of ["block", "classDiagram", "c4Dynamic", "flowchart", "treemap"]) {
    const nbsp = measurePlainText(diagram, "\u00A0");
    const space = measurePlainText(diagram, " ");
    expect(nbsp.value).toBe("\u00A0");
    expect(nbsp.domWidth).toBe(space.domWidth);
    expect(nbsp.canvasWidth).toBe(space.canvasWidth);
  }
});

test("optional layout ascent changes the SVG cell top without replacing ink bounds", () => {
  const metrics = {
    advance: 20,
    ascent: 11.3,
    descent: 3.2,
    inkLeft: -2,
    inkTop: -8,
    inkRight: 22,
    inkBottom: 4,
  };
  const natural = measureNativeSvgMetricCell(metrics);
  const adjusted = measureNativeSvgMetricCell({ ...metrics, layoutAscent: 15 });
  const zero = measureNativeSvgMetricCell({ ...metrics, layoutAscent: 0 });
  const overhang = measureNativeSvgMetricCell({ ...metrics, layoutAscent: 15, inkTop: -20 });

  expect(natural).toEqual({ x: 8, y: 9, width: 24, height: 15 });
  expect(adjusted).toEqual({ x: 8, y: 5, width: 24, height: 19 });
  expect(zero).toEqual({ x: 8, y: 12, width: 24, height: 12 });
  expect(overhang).toEqual({ x: 8, y: 0, width: 24, height: 24 });
});

test("dominant baseline middle shifts native SVG metrics by half the selected font x-height", () => {
  const metrics = { advance: 10, ascent: 8, descent: 2 };
  const baseline = measureNativeSvgMetricCell(metrics, "alphabetic");
  const middle = measureNativeSvgMetricCell(metrics, "middle", 7.3125);
  const missingMetric = measureNativeSvgMetricCell(metrics, "middle");
  const unsupported = measureNativeSvgMetricCell(metrics, "central", 7.3125);

  expect(middle.y).toBeCloseTo(baseline.y + 7.3125 / 2, 10);
  expect(missingMetric.y).toBeCloseTo(baseline.y + 16 * 0.25, 10);
  expect(unsupported.y).toBe(baseline.y);
});

test("middle baseline offset is preserved in SVG metric and positioned-run fallbacks", () => {
  const metrics = { advance: 10, ascent: 8, descent: 2 };
  for (const positioned of [false, true]) {
    const baseline = measureSvgFallbackMetricCell(metrics, "alphabetic", 7.3125, positioned);
    const middle = measureSvgFallbackMetricCell(metrics, "middle", 7.3125, positioned);

    expect(baseline.nativeMetricCalls).toBe(0);
    expect(middle.nativeMetricCalls).toBe(0);
    assertMiddleBaselineShift(middle.box, baseline.box, 7.3125 / 2);
  }
});

function assertMiddleBaselineShift(actual: SvgMetricCell, baseline: SvgMetricCell, offset: number) {
  expect(actual.y).toBeCloseTo(baseline.y + offset, 10);
}

test("SVG ex size uses inherited font x-height in both layout paths", () => {
  const resolved = resolveSvgExSize("4ex", 7.5);
  expect(resolved.fontSize).toBe(30);
  expect(resolved.lineHeightSize).toBe(30);
  expect(resolved.calls).toEqual([
    ["Noto Sans", 16, 400, false],
    ["Noto Sans", 16, 400, false],
  ]);
});

test("SVG ex uses the CSS fallback only when the selected font has no x-height", () => {
  const resolved = resolveSvgExSize("4ex", undefined);
  expect(resolved.fontSize).toBe(32);
  expect(resolved.lineHeightSize).toBe(32);
});

test("legacy line-height size keeps negative values and cascades invalid values", () => {
  expect(resolveLegacySvgFontSize("-2em", "16px")).toBe(-32);
  expect(resolveLegacySvgFontSize("invalid", "20px")).toBe(20);
  expect(resolveLegacySvgFontSize("initial", "20px")).toBe(20);
});

test.each([
  ["NaN", Number.NaN],
  ["infinity", Number.POSITIVE_INFINITY],
  ["negative", -1],
  ["null", null],
  ["explicit undefined", undefined],
] as const)("invalid optional layout ascent is rejected: %s", (_label, layoutAscent) => {
  expect(() =>
    measureNativeSvgMetricCell({ advance: 20, ascent: 11, descent: 3, layoutAscent }),
  ).toThrow("native SVG font metrics are invalid");
});

function nativeRuntimeGate(source: string | null, drawio: boolean): boolean {
  const context = createContext({ KatanaNode: class {}, sourceInput: source });
  for (const name of ["mermaid_diagram_type.js", "dom_text_measure.js"]) {
    runInContext(readFileSync(new URL(name, RUNTIME_DIRECTORY), "utf8"), context);
  }
  if (source !== null) {
    runInContext(
      "globalThis.__katanaMermaidDiagramType = katanaMermaidDiagramType(sourceInput)",
      context,
    );
  }
  if (drawio) {
    runInContext("globalThis.__katanaDrawioRequest = { source: '<mxfile/>' }", context);
  }
  return runInContext("katanaMermaidNativeAdvanceRuntime()", context) as boolean;
}

test("native SVG advance follows existing Mermaid type and excludes Drawio and ZenUML requests", () => {
  expect(nativeRuntimeGate(null, false)).toBeFalse();
  expect(nativeRuntimeGate("flowchart LR\n a-->b", false)).toBeTrue();
  expect(nativeRuntimeGate("classDiagram\n class A", false)).toBeTrue();
  expect(nativeRuntimeGate("C4Dynamic\n title Scope", false)).toBeTrue();
  expect(nativeRuntimeGate("zenuml\n A->B", false)).toBeFalse();
  expect(nativeRuntimeGate(null, true)).toBeFalse();
  expect(nativeRuntimeGate("flowchart LR\n a-->b", true)).toBeFalse();
});

const MIXED_C4_RUN_SCRIPT = `(() => {
  const ns = "http://www.w3.org/2000/svg";
  const group = document.createElementNS(ns, "g");
  group.style.setProperty("font-weight", "bold");
  const outer = document.createElementNS(ns, "tspan");
  outer.style.setProperty("font-family", "Open Sans, sans-serif");
  outer.style.setProperty("font-size", "14px");
  for (const [weight, text] of [["normal", "Banking System"], ["bold", " F Queue"]]) {
    const inner = document.createElementNS(ns, "tspan");
    inner.setAttribute("font-weight", weight);
    inner.textContent = text;
    outer.appendChild(inner);
  }
  group.appendChild(outer);
  const normal = document.createElementNS(ns, "tspan");
  normal.style.setProperty("font-family", "Open Sans, sans-serif");
  normal.style.setProperty("font-size", "14px");
  normal.setAttribute("font-weight", "normal");
  normal.textContent = "Banking System F Queue";
  return {
    mixed: outer.getComputedTextLength(),
    sum: outer.children[0].getComputedTextLength() + outer.children[1].getComputedTextLength(),
    flattenedNormal: normal.getComputedTextLength(),
    nativeInstalled: typeof globalThis.__katanaMeasureText === "function",
  };
})()`;

function mixedC4RunWidths() {
  const context = createContext({ sourceInput: "C4Dynamic\n title Mixed runs" });
  runInContext(
    readFileSync(new URL("mermaid_diagram_type.js", RUNTIME_DIRECTORY), "utf8"),
    context,
  );
  runInContext(
    "globalThis.__katanaMermaidDiagramType = katanaMermaidDiagramType(sourceInput)",
    context,
  );
  const bundle = new URL(
    "../../crates/katana-render-runtime/src/markdown/diagram_runtime/generated/mermaid-runtime.min.js",
    import.meta.url,
  );
  runInContext(readFileSync(bundle, "utf8"), context);
  return runInContext(MIXED_C4_RUN_SCRIPT, context) as {
    mixed: number;
    sum: number;
    flattenedNormal: number;
    nativeInstalled: boolean;
  };
}

test("C4 nested tspan respects nearest normal and bold runs through public DOM measurement", () => {
  const widths = mixedC4RunWidths();
  expect(widths.nativeInstalled).toBeFalse();
  expect(widths.mixed).toBeCloseTo(widths.sum, 10);
  expect(widths.mixed).toBeGreaterThan(widths.flattenedNormal);
});
