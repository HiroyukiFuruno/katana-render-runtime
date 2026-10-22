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

function measure(source: TestNode, target: TestNode) {
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
  const result = runInContext(
    "({ childBoxes: katanaVisibleChildBoxes(target), measured: katanaVisibleMeasuredBoxAccurate(target) })",
    context,
  ) as { childBoxes: Box[]; measured: Box };
  return {
    childBoxes: result.childBoxes.map(({ x, y, width, height }) => ({ x, y, width, height })),
    measured: {
      x: result.measured.x,
      y: result.measured.y,
      width: result.measured.width,
      height: result.measured.height,
    },
  };
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
