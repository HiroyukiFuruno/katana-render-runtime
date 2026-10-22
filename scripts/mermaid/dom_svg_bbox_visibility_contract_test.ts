import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { createContext, runInContext } from "node:vm";

const RUNTIME_DIRECTORY = new URL(
  "../../crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime/",
  import.meta.url,
);

type Box = { x: number; y: number; width: number; height: number };
type NodeInput = {
  attributes?: Record<string, string>;
  box?: Box;
  children?: TestNode[];
  textContent?: string;
};
type TestNode = {
  attributes: Record<string, string>;
  children: TestNode[];
  className: string;
  getAttribute: (name: string) => string | null;
  getBBox: () => Box;
  id: string;
  localName: string;
  parentNode: TestNode | null;
  style: { getPropertyPriority: () => string; getPropertyValue: (name: string) => string };
  textContent: string;
  _textContent: string;
};

function node(localName: string, input: NodeInput = {}): TestNode {
  const attributes = input.attributes ?? {};
  const children = input.children ?? [];
  const result: TestNode = {
    attributes,
    children,
    className: attributes.class ?? "",
    getAttribute: (name) => attributes[name] ?? null,
    getBBox: () => input.box ?? { x: 0, y: 0, width: 0, height: 0 },
    id: attributes.id ?? "",
    localName,
    parentNode: null,
    style: {
      getPropertyPriority: () => "",
      getPropertyValue: (name) => attributes[`style:${name}`] ?? "",
    },
    textContent: input.textContent ?? "",
    _textContent: input.textContent ?? "",
  };
  for (const child of children) child.parentNode = result;
  return result;
}

function createMeasurementContext(source: TestNode, target: TestNode) {
  const context = createContext({
    KatanaNode: class {},
    katanaAnchoredTextNodeBox: (_node: TestNode, text: string, x: number, y: number) => ({
      x,
      y,
      width: text.length,
      height: 1,
    }),
    document: { documentElement: source },
    source,
    target,
  });
  for (const name of [
    "dom_css_style.js",
    "svg_transform_bounds.js",
    "dom_layout.js",
    "dom_measure.js",
  ]) {
    runInContext(readFileSync(new URL(name, RUNTIME_DIRECTORY), "utf8"), context);
  }
  return context;
}

function runMeasurement(source: TestNode, target: TestNode, expression: string) {
  return runInContext(expression, createMeasurementContext(source, target));
}

function boxValues(box: Box) {
  return {
    x: box.x,
    y: box.y,
    width: box.width,
    height: box.height,
  };
}

function measure(source: TestNode, target: TestNode) {
  const result = runMeasurement(
    source,
    target,
    "({ childBoxes: katanaVisibleChildBoxes(target), measured: katanaVisibleMeasuredBoxAccurate(target) })",
  ) as { childBoxes: Box[]; measured: Box };
  return {
    childBoxes: result.childBoxes.map(boxValues),
    measured: boxValues(result.measured),
  };
}

function measureAccurate(source: TestNode, target: TestNode) {
  return boxValues(
    runMeasurement(source, target, "katanaMeasuredBoxAccurate(target)") as Box,
  );
}

test("SVG child boxes omit authored display:none subtrees but retain visibility:hidden geometry", () => {
  const hiddenShape = node("rect", {
    attributes: {
      "style:display": "none",
      x: "10",
      y: "20",
      width: "7",
      height: "9",
    },
    box: { x: 10, y: 20, width: 7, height: 9 },
  });
  const retainedShape = node("rect", {
    attributes: { visibility: "hidden", x: "30", y: "40", width: "5", height: "6" },
    box: { x: 30, y: 40, width: 5, height: 6 },
  });
  const presentationHidden = node("rect", {
    attributes: { display: "none", x: "50", y: "60", width: "8", height: "9" },
    box: { x: 50, y: 60, width: 8, height: 9 },
  });
  const source = node("svg", { children: [hiddenShape, retainedShape, presentationHidden] });
  const result = measure(source, source);

  expect(result.childBoxes).toEqual([{ x: 30, y: 40, width: 5, height: 6 }]);
  expect(result.measured).toEqual({ x: 30, y: 40, width: 5, height: 6 });
  expect(measure(source, hiddenShape).measured).toEqual({ x: 10, y: 20, width: 7, height: 9 });
});

test("display:none SVG groups retain intrinsic boxes while hidden self geometry is zero", () => {
  const hiddenGroup = node("g", {
    attributes: { "style:display": "none" },
    children: [
      node("rect", { box: { x: 10, y: 20, width: 100, height: 50 } }),
      node("text", { box: { x: 20, y: 26, width: 32.90625, height: 18 } }),
    ],
  });
  const hiddenSelfNodes = [
    node("rect", {
      attributes: { "style:display": "none" },
      box: { x: 30, y: 40, width: 80, height: 60 },
    }),
    node("text", {
      attributes: { "style:display": "none" },
      box: { x: 20, y: 26, width: 32.90625, height: 18 },
    }),
    node("tspan", {
      attributes: { "style:display": "none" },
      box: { x: 20, y: 26, width: 32.90625, height: 18 },
    }),
    node("a", {
      attributes: { "style:display": "none" },
      children: [node("rect", { box: { x: 10, y: 20, width: 100, height: 50 } })],
    }),
    node("switch", {
      attributes: { "style:display": "none" },
      children: [node("rect", { box: { x: 10, y: 20, width: 100, height: 50 } })],
    }),
    node("svg", {
      attributes: { "style:display": "none" },
      children: [node("rect", { box: { x: 10, y: 20, width: 100, height: 50 } })],
    }),
  ];
  const source = node("svg", { children: [hiddenGroup, ...hiddenSelfNodes] });

  expect(measureAccurate(source, hiddenGroup)).toEqual({
    x: 10,
    y: 20,
    width: 100,
    height: 50,
  });
  for (const hiddenSelfNode of hiddenSelfNodes) {
    expect(measureAccurate(source, hiddenSelfNode)).toEqual({
      x: 0,
      y: 0,
      width: 0,
      height: 0,
    });
  }
  expect(measure(source, source).childBoxes).toEqual([]);
});

test("SVG child unions omit empty text points and retain painted text and degenerate geometry", () => {
  const degenerateGroup = (
    tag: string,
    x: number,
    attributes: Record<string, string> = {},
    textContent = "",
  ) =>
    node("g", {
      attributes: { transform: `translate(${x} 70)` },
      box: { x: 0, y: 0, width: 0, height: 0 },
      children: [node(tag, { attributes, textContent })],
    });
  const pointChildren = [
    degenerateGroup("text", 10),
    node("g", {
      attributes: { transform: "translate(15 70)" },
      children: [node("text", { children: [node("tspan")] })],
    }),
    degenerateGroup("text", 20, {}, "visible"),
    node("line", { attributes: { transform: "translate(30 70)" } }),
    node("path", { attributes: { d: "M0 0", transform: "translate(40 70)" } }),
    node("rect", { attributes: { transform: "translate(50 70)" } }),
    node("g", { attributes: { transform: "translate(60 70)" } }),
    degenerateGroup("text", 80, { "style:display": "none" }, "hidden"),
  ];
  const source = node("svg", { children: pointChildren });
  const result = measure(source, source);

  expect(result.childBoxes).toEqual([
    { x: 20, y: 70, width: 0, height: 0 },
    { x: 30, y: 70, width: 0, height: 0 },
    { x: 40, y: 70, width: 0, height: 0 },
  ]);
  expect(result.measured).toEqual({ x: 20, y: 70, width: 20, height: 0 });
});

test("SVG child unions follow Chrome positioned empty text container depth", () => {
  const positionedEmptyText = () =>
    node("text", {
      attributes: { y: "-10.1" },
      children: [node("tspan", { attributes: { x: "0", y: "-0.1em", dy: "1.1em" } })],
    });
  const directText = positionedEmptyText();
  const oneContainer = node("g", {
    attributes: { transform: "translate(82.765625 -9.5)" },
    children: [positionedEmptyText()],
  });
  const twoContainers = node("g", {
    attributes: { transform: "translate(120 0)" },
    children: [node("g", { children: [positionedEmptyText()] })],
  });
  const threeContainers = node("g", {
    attributes: { transform: "translate(150 0)" },
    children: [node("g", { children: [node("g", { children: [positionedEmptyText()] })] })],
  });
  const hiddenPositionedEmptyLabel = node("g", {
    attributes: { "style:display": "none", transform: "translate(180 0)" },
    children: [positionedEmptyText()],
  });
  const source = node("svg", {
    children: [directText, oneContainer, twoContainers, threeContainers, hiddenPositionedEmptyLabel],
  });

  expect(measure(source, source)).toEqual({
    childBoxes: [
      { x: 0, y: 0, width: 0, height: 0 },
      { x: 82.765625, y: -9.5, width: 0, height: 0 },
    ],
    measured: { x: 0, y: -9.5, width: 82.765625, height: 9.5 },
  });
});

test("parent text fallback excludes display:none child labels", () => {
  const visibleOnly = node("g", { children: [node("text", { textContent: "x" })] });
  const withHiddenChild = node("g", {
    children: [
      node("g", {
        attributes: { "style:display": "none" },
        children: [node("text", { textContent: "hidden label expands the fallback" })],
      }),
      node("text", { textContent: "x" }),
    ],
  });

  expect(measure(withHiddenChild, withHiddenChild)).toEqual(measure(visibleOnly, visibleOnly));
});

test("hidden treemap text does not expand its visible section box", () => {
  const section = node("g", {
    children: [
      node("rect", {
        attributes: { width: "1000", height: "400" },
        box: { x: 0, y: 0, width: 1000, height: 400 },
      }),
      node("text", {
        attributes: { "style:display": "none" },
        box: { x: 1000, y: 0, width: 20, height: 47.5 },
      }),
    ],
  });
  const source = node("svg", { children: [section] });

  expect(measure(source, section).measured).toEqual({ x: 0, y: 0, width: 1000, height: 400 });
});
