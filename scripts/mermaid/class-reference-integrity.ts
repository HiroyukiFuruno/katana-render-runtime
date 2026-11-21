import { DOMParser, type Element as XmlElement } from "@xmldom/xmldom";

type Point = [number, number];
type Matrix = [number, number, number, number];
type Box = { left: number; top: number; right: number; bottom: number };
type Specificity = [number, number, number];
type CascadePriority = [number, number, number, number, number];
type Rule = { selector: string; values: Map<string, Declaration>; specificity: Specificity };
type Declaration = { value: string; important: boolean };
type ClassContract = { name: string; members: string[]; methods: string[] };
export type ClassReferenceLanguage = "en" | "ja";
export type ClassReferenceIntegrity = {
  accepted: boolean;
  reasons: string[];
  nodes: { name: string; box: Box; anchors: Point[] }[];
  edges: number;
  scope: "class-inheritance-text-anchors";
};

const CONTRACTS: Record<ClassReferenceLanguage, ClassContract[]> = {
  en: [
    {
      name: "Animal",
      members: ["+int age", "+String gender"],
      methods: ["+isMammal()", "+mate()"],
    },
    { name: "Duck", members: ["+String beakColor"], methods: ["+swim()", "+quack()"] },
    { name: "Fish", members: ["-int sizeInFeet"], methods: ["-canEat()"] },
    { name: "Zebra", members: ["+bool is_wild"], methods: ["+run()"] },
  ],
  ja: [
    { name: "動物", members: ["+int 年齢", "+String 性別"], methods: ["+哺乳類か()", "+交配()"] },
    { name: "アヒル", members: ["+String くちばしの色"], methods: ["+泳ぐ()", "+鳴く()"] },
    { name: "魚", members: ["-int 体長フィート"], methods: ["-食べられるか()"] },
    { name: "シマウマ", members: ["+bool 野生か"], methods: ["+走る()"] },
  ],
};
const NUMBER = "[-+]?(?:\\d*\\.\\d+|\\d+\\.?\\d*)(?:[eE][-+]?\\d+)?";
const identity: Matrix = [1, 1, 0, 0];
const fail = (message: string): never => {
  throw new Error(message);
};
const classes = (element: XmlElement) => (element.getAttribute("class") ?? "").split(/\s+/);
const descendants = (element: XmlElement): XmlElement[] =>
  Array.from(element.getElementsByTagName("*"));
const named = (element: XmlElement, name: string) =>
  descendants(element).filter((child) => child.localName === name);
const parent = (element: XmlElement): XmlElement | null =>
  element.parentNode?.nodeType === 1 ? (element.parentNode as XmlElement) : null;
const normalized = (value: string) => value.replace(/\s+/gu, " ").trim();
const numeric = (value: string) => {
  if (!new RegExp(`^${NUMBER}$`).test(value.trim())) return fail(`unsupported:number:${value}`);
  const result = Number(value);
  if (!Number.isFinite(result)) return fail(`unsupported:nonfinite:${value}`);
  return result;
};
const numbers = (value: string) => {
  const tokens = value.trim().split(/[\s,]+/);
  if (!value.trim() || /,\s*,/.test(value)) return fail(`unsupported:numbers:${value}`);
  return tokens.map(numeric);
};
const point = (matrix: Matrix, [x, y]: Point): Point => [
  matrix[0] * x + matrix[2],
  matrix[1] * y + matrix[3],
];
const compose = (a: Matrix, b: Matrix): Matrix => [
  a[0] * b[0],
  a[1] * b[1],
  a[0] * b[2] + a[2],
  a[1] * b[3] + a[3],
];

function transform(value: string): Matrix {
  let matrix: Matrix = identity;
  let rest = value.trim();
  while (rest) {
    const match = rest.match(/^(translate|scale|matrix)\s*\(([^()]*)\)\s*/);
    if (!match) return fail(`unsupported:transform:${value}`);
    const next = transformArguments(match[1] ?? "", numbers(match[2] ?? ""));
    if (next[0] <= 0 || next[1] <= 0) return fail(`unsupported:transform-orientation:${value}`);
    matrix = compose(matrix, next);
    rest = rest.slice(match[0].length);
  }
  return matrix;
}

function transformArguments(command: string, args: number[]): Matrix {
  if (command === "matrix") {
    if (args.length !== 6 || args[1] !== 0 || args[2] !== 0)
      return fail("unsupported:transform-arguments:matrix");
    return [args[0] ?? 0, args[3] ?? 0, args[4] ?? 0, args[5] ?? 0];
  }
  if (args.length !== 1 && args.length !== 2)
    return fail(`unsupported:transform-arguments:${command}`);
  if (command === "translate") return [1, 1, args[0] ?? 0, args[1] ?? 0];
  return [args[0] ?? 0, args[1] ?? args[0] ?? 0, 0, 0];
}

function world(element: XmlElement): Matrix {
  const chain: XmlElement[] = [];
  for (let current: XmlElement | null = element; current; current = parent(current))
    chain.unshift(current);
  return chain.reduce(
    (matrix, item) => compose(matrix, transform(item.getAttribute("transform") ?? "")),
    identity,
  );
}

function declarations(value: string): Map<string, Declaration> {
  const result = new Map<string, Declaration>();
  for (const token of cssLexicalInput(value).split(";")) {
    if (!token.trim()) continue;
    const index = token.indexOf(":");
    if (index < 1) return fail(`unsupported:css-declaration:${token}`);
    const text = token.slice(index + 1).trim();
    const declaration = {
      value: text.replace(/\s*!important\s*$/i, "").trim(),
      important: /!important\s*$/i.test(text),
    };
    const name = declarationName(token.slice(0, index).trim());
    if (!result.get(name)?.important || declaration.important) result.set(name, declaration);
  }
  return result;
}

function declarationName(value: string): string {
  if (value.startsWith("--")) {
    if (!/^--[a-zA-Z0-9_-]+$/.test(value)) return fail(`unsupported:custom-property:${value}`);
    return value;
  }
  if (!/^-?[a-zA-Z][a-zA-Z0-9-]*$/.test(value)) return fail(`unsupported:css-property:${value}`);
  return value.toLowerCase();
}

function cssStringEnd(value: string, start: number): number {
  const quote = value[start];
  for (let index = start + 1; index < value.length; index += 1) {
    const character = value[index] ?? "";
    if (character === "\\") {
      if (/[\r\n\f;{}]/.test(value[index + 1] ?? "")) return fail("unsupported:css-string");
      index += 1;
      continue;
    }
    if (character === quote) return index + 1;
    if (/[\r\n\f;{}]/.test(character)) return fail("unsupported:css-string");
  }
  return fail("unsupported:css-string");
}

function cssLexicalInput(value: string): string {
  for (let index = 0; index < value.length; ) {
    const character = value[index] ?? "";
    if (character === '"' || character === "'") index = cssStringEnd(value, index);
    else {
      // コメント境界のtokenを推測すると、selectorや宣言の実効値を変えてしまう。
      if (value.startsWith("/*", index)) return fail("unsupported:css-comment");
      if (character === "\\") return fail("unsupported:css-escape");
      index += 1;
    }
  }
  return value;
}

function withoutKeyframes(value: string): string {
  return cssLexicalInput(value).replace(
    /@keyframes\s+[\w-]+\s*\{((?:\s*(?:from|to|[\d.]+%)\s*\{[^{}]*\}\s*)+)\}/g,
    (_match, body: string) => {
      for (const frame of body.matchAll(/\{([^{}]*)\}/g))
        for (const property of declarations(frame[1] ?? "").keys())
          if (property !== "stroke-dashoffset") return fail("unsupported:css-keyframe-property");
      return "";
    },
  );
}

function rules(root: XmlElement): Rule[] {
  const result: Rule[] = [];
  for (const style of named(root, "style")) {
    const css = withoutKeyframes(style.textContent ?? "");
    let end = 0;
    for (const match of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
      if (css.slice(end, match.index).trim()) return fail("unsupported:css-schema");
      for (const selector of (match[1] ?? "").split(","))
        result.push(cssRule(selector, match[2] ?? ""));
      end = (match.index ?? 0) + match[0].length;
    }
    if (css.slice(end).trim()) return fail("unsupported:css-schema");
  }
  return result;
}

function cssRule(selector: string, body: string): Rule {
  const specificity: Specificity = [0, 0, 0];
  for (const token of selectorTokens(selector)) {
    const parsed = simpleSelector(token);
    if (parsed.tag && parsed.tag !== "*") specificity[2] += 1;
    for (const part of parsed.parts) {
      if (part[1]) specificity[0] += 1;
      else specificity[1] += 1;
    }
  }
  return { selector: selector.trim(), values: declarations(body), specificity };
}

function selectorTokens(selector: string): string[] {
  const tokens = selector.match(/(?:\[[^\]]*\]|[^\s])+/g) ?? [];
  if (!tokens.length) return fail("unsupported:empty-selector");
  return tokens;
}

function simpleSelector(selector: string): { tag: string | null; parts: RegExpMatchArray[] } {
  let rest = selector;
  const tag = rest.match(/^(\*|[a-zA-Z][\w-]*)/);
  if (tag) rest = rest.slice(tag[0].length);
  const parts: RegExpMatchArray[] = [];
  while (rest) {
    const match = rest.match(
      /^(?:#([\w\u0080-\uFFFF-]+)|\.([\w-]+)|\[([\w-]+)(?:([*$^]?=)"([^"\\]*)")?\]|(:root))/u,
    );
    if (!match) return fail(`unsupported:css-selector:${selector}`);
    parts.push(match);
    rest = rest.slice(match[0].length);
  }
  if (!tag && !parts.length) return fail("unsupported:empty-selector");
  return { tag: tag?.[0] ?? null, parts };
}

function matchesSimple(element: XmlElement, selector: string): boolean {
  const parsed = simpleSelector(selector);
  if (parsed.tag && parsed.tag !== "*" && parsed.tag !== element.localName) return false;
  return parsed.parts.every((part) => matchesSelectorPart(element, part));
}

function matchesSelectorPart(element: XmlElement, match: RegExpMatchArray): boolean {
  if (match[1]) return element.getAttribute("id") === match[1];
  if (match[2]) return classes(element).includes(match[2]);
  if (match[6]) return parent(element) === null;
  const value = element.getAttribute(match[3] ?? "");
  if (value === null) return false;
  return matchesAttribute(value, match[4] ?? "", match[5] ?? "");
}

function matchesAttribute(value: string, operation: string, expected: string): boolean {
  switch (operation) {
    case "=":
      return value === expected;
    case "$=":
      return value.endsWith(expected);
    case "^=":
      return value.startsWith(expected);
    case "*=":
      return value.includes(expected);
    case "":
      return true;
    default:
      return fail("unsupported:css-attribute-operation");
  }
}

function matchingAncestor(element: XmlElement, token: string): XmlElement | null {
  let current = parent(element);
  while (current && !matchesSimple(current, token)) current = parent(current);
  return current;
}

function matches(element: XmlElement, selector: string): boolean {
  // 属性の空白を保持し、子結合子など未対応の構文は推測しない。
  const tokens = selectorTokens(selector);
  let current = element;
  if (!matchesSimple(current, tokens.pop() ?? "")) return false;
  for (const token of tokens.reverse()) {
    const ancestor = matchingAncestor(current, token);
    if (!ancestor) return false;
    current = ancestor;
  }
  return true;
}

function presentationDeclaration(
  element: XmlElement,
  property: string,
  includeAttribute: boolean,
): Declaration | undefined {
  return includeAttribute && element.hasAttribute(property)
    ? { value: element.getAttribute(property) ?? "", important: false }
    : undefined;
}

function cascadePriority(
  declaration: Declaration,
  specificity: Specificity,
  inline = false,
): CascadePriority {
  return [declaration.important ? 1 : 0, inline ? 1 : 0, ...specificity];
}

function atLeastPriority(candidate: CascadePriority, selected?: CascadePriority): boolean {
  if (!selected) return true;
  // 各段は桁上がりしないため、クラス数でIDやinlineの優先順位を覆さない。
  for (const index of [0, 1, 2, 3, 4] as const) {
    if (candidate[index] !== selected[index]) return candidate[index] > selected[index];
  }
  return true;
}

function specified(
  element: XmlElement,
  property: string,
  css: Rule[],
  includeAttribute = true,
): Declaration | undefined {
  let selected = presentationDeclaration(element, property, includeAttribute);
  let priority: CascadePriority | undefined = selected ? [0, 0, 0, 0, 0] : undefined;
  for (const rule of css) {
    const declaration = rule.values.get(property);
    if (!declaration || !matches(element, rule.selector)) continue;
    const candidate = cascadePriority(declaration, rule.specificity);
    if (atLeastPriority(candidate, priority)) {
      selected = declaration;
      priority = candidate;
    }
  }
  const inline = declarations(element.getAttribute("style") ?? "").get(property);
  if (inline && atLeastPriority(cascadePriority(inline, [0, 0, 0], true), priority))
    selected = inline;
  return selected;
}

function computed(element: XmlElement, property: string, css: Rule[], fallback: string): string {
  const selected = specified(element, property, css);
  if (selected && selected.value.trim().toLowerCase() !== "inherit") return selected.value;
  const outer = parent(element);
  return outer ? computed(outer, property, css, fallback) : fallback;
}

function fontSize(element: XmlElement, css: Rule[]): number {
  const value = computed(element, "font-size", css, "");
  if (!value || !new RegExp(`^${NUMBER}(?:px)?$`).test(value))
    return fail(`unsupported:font-size:${value}`);
  const size = numeric(value.replace(/px$/, ""));
  if (size <= 0) return fail("unsupported:font-size-positive");
  return size;
}

function length(value: string | null, element: XmlElement, css: Rule[]): number {
  if (value === null || value === "") return 0;
  if (value.endsWith("em")) return numeric(value.slice(0, -2)) * fontSize(element, css);
  return numeric(value.replace(/px$/, ""));
}

function textAnchor(text: XmlElement, css: Rule[]): Point {
  const rows = named(text, "tspan").filter((span) => classes(span).includes("row"));
  if (rows.length > 1) return fail("unsupported:text-multiple-rows");
  const row = rows[0] ?? text;
  if (
    [text, row].some((element) =>
      ["rotate", "textLength", "lengthAdjust"].some((attribute) => element.hasAttribute(attribute)),
    )
  )
    return fail("unsupported:text-layout-attribute");
  const x =
    length(row.getAttribute("x") ?? text.getAttribute("x"), row, css) +
    length(row.getAttribute("dx"), row, css);
  const y =
    length(row.getAttribute("y") ?? text.getAttribute("y"), row, css) +
    length(row.getAttribute("dy"), row, css);
  for (const child of named(row, "tspan"))
    if (["x", "y", "dx", "dy", "rotate"].some((attr) => child.hasAttribute(attr)))
      return fail("unsupported:text-positioned-inner-run");
  return point(world(row), [x, y]);
}

function segmentEndpoint(command: string, args: number[], current: Point): Point {
  const upper = command.toUpperCase();
  const relative = command !== upper;
  const last = args.length - 2;
  let next: Point = [args[last] ?? 0, args[last + 1] ?? 0];
  if (upper === "H") next = [args[0] ?? 0, relative ? 0 : current[1]];
  if (upper === "V") next = [relative ? 0 : current[0], args[0] ?? 0];
  return relative ? [current[0] + next[0], current[1] + next[1]] : next;
}

function pathArgumentCount(command: string, curved: boolean): number {
  const upper = command.toUpperCase();
  const count = new Map([
    ["M", 2],
    ["L", 2],
    ["H", 1],
    ["V", 1],
    ["C", 6],
    ["Q", 4],
  ]).get(upper);
  if (!count || (!curved && ["C", "Q"].includes(upper)))
    return fail(`unsupported:path-command:${command}`);
  return count;
}

type PathCursor = { index: number; command: string };

function pathSegment(tokens: string[], cursor: PathCursor, curved: boolean): number[] | null {
  if (/^[a-zA-Z]$/.test(tokens[cursor.index] ?? "")) cursor.command = tokens[cursor.index++] ?? "";
  if (cursor.command.toUpperCase() === "Z") {
    if (cursor.index !== tokens.length) return fail("unsupported:path-after-close");
    return null;
  }
  const count = pathArgumentCount(cursor.command, curved);
  const args = tokens.slice(cursor.index, cursor.index + count).map(numeric);
  if (args.length !== count) return fail("unsupported:path-arguments");
  cursor.index += count;
  return args;
}

function pathPoints(value: string, curved: boolean): Point[] {
  const tokens = value.match(new RegExp(`[a-zA-Z]|${NUMBER}`, "g")) ?? [];
  const remainder = value.replace(new RegExp(`[a-zA-Z]|${NUMBER}`, "g"), "").replace(/[\s,]/g, "");
  if (remainder || !tokens.length) return fail(`unsupported:path:${value}`);
  const result: Point[] = [];
  const cursor: PathCursor = { index: 0, command: "" };
  let current: Point = [0, 0];
  while (cursor.index < tokens.length) {
    const args = pathSegment(tokens, cursor, curved);
    if (args === null) break;
    const upper = cursor.command.toUpperCase();
    if (upper === "M" && result.length) return fail("unsupported:path-subpath");
    current = segmentEndpoint(cursor.command, args, current);
    result.push(current);
    if (upper === "M") cursor.command = cursor.command === upper ? "L" : "l";
  }
  if (!/^[Mm]/.test(value.trim())) return fail("unsupported:path-no-move");
  return result;
}

function bounds(points: Point[]): Box {
  return {
    left: Math.min(...points.map(([x]) => x)),
    right: Math.max(...points.map(([x]) => x)),
    top: Math.min(...points.map(([, y]) => y)),
    bottom: Math.max(...points.map(([, y]) => y)),
  };
}

function outerBox(node: XmlElement, css: Rule[]): Box {
  const outers = descendants(node).filter((element) => classes(element).includes("outer-path"));
  const shapes = outers.flatMap((outer) =>
    Array.from(outer.childNodes).filter((child): child is XmlElement => child.nodeType === 1),
  );
  const shape = shapes.find(
    (element) =>
      element.localName === "rect" ||
      (element.localName === "path" && element.getAttribute("fill") !== "none"),
  );
  if (!shape) return fail("unsupported:outer-box-shape");
  assertVisibleTree(shape, css);
  if (!visibleFill(shape, css) && !visibleStroke(shape, css))
    return fail("invalid:outer-box-invisible-paint");
  let vertices: Point[];
  if (shape.localName === "rect") {
    const x = numeric(shape.getAttribute("x") ?? "0"),
      y = numeric(shape.getAttribute("y") ?? "0");
    const width = numeric(shape.getAttribute("width") ?? ""),
      height = numeric(shape.getAttribute("height") ?? "");
    if (width <= 0 || height <= 0) return fail("invalid:outer-box-size");
    vertices = [
      [x, y],
      [x + width, y],
      [x + width, y + height],
      [x, y + height],
    ];
  } else vertices = pathPoints(shape.getAttribute("d") ?? "", false);
  assertRectangle(vertices);
  const box = bounds(vertices.map((p) => point(world(shape), p)));
  if (
    ![box.left, box.top, box.right, box.bottom].every(Number.isFinite) ||
    box.right <= box.left ||
    box.bottom <= box.top
  )
    return fail("invalid:outer-box-size");
  return box;
}

function assertRectangle(vertices: Point[]) {
  if (
    vertices.length !== 4 ||
    new Set(vertices.map(([x]) => x)).size !== 2 ||
    new Set(vertices.map(([, y]) => y)).size !== 2 ||
    new Set(vertices.map((p) => p.join(","))).size !== 4
  )
    return fail("unsupported:outer-box-rectangle");
  for (let index = 0; index < vertices.length; index++) {
    const a = vertices[index] as Point,
      b = vertices[(index + 1) % vertices.length] as Point;
    if (a[0] !== b[0] && a[1] !== b[1]) return fail("unsupported:outer-box-crossing");
  }
}

function assertContent(node: XmlElement, group: string, expected: string[]) {
  const groups = descendants(node).filter((element) => classes(element).includes(group));
  if (groups.length !== 1) return fail(`unsupported:content-group:${group}`);
  const actual = named(groups[0] as XmlElement, "text")
    .map((text) => normalized(text.textContent ?? ""))
    .sort();
  if (JSON.stringify(actual) !== JSON.stringify([...expected].sort()))
    return fail(`invalid:content:${group}:${JSON.stringify(actual)}`);
}

function markerVertices(shape: XmlElement): Point[] {
  let vertices: Point[];
  if (shape.localName === "polygon") {
    const args = numbers(shape.getAttribute("points") ?? "");
    if (args.length !== 6) return fail("invalid:marker-triangle");
    const [x1, y1, x2, y2, x3, y3] = args as [number, number, number, number, number, number];
    vertices = [
      [x1, y1],
      [x2, y2],
      [x3, y3],
    ];
  } else if (shape.localName === "path" && /[zZ]\s*$/.test(shape.getAttribute("d") ?? ""))
    vertices = pathPoints(shape.getAttribute("d") ?? "", false);
  else return fail("unsupported:marker-triangle-schema");
  if (vertices.length !== 3) return fail("invalid:marker-triangle");
  const [a, b, c] = vertices as [Point, Point, Point];
  if ((b[0] - a[0]) * (c[1] - a[1]) === (b[1] - a[1]) * (c[0] - a[0]))
    return fail("invalid:marker-triangle-area");
  return vertices;
}

const PAINT_PROPERTIES = new Set(
  "color fill fill-opacity stroke stroke-opacity stroke-width stroke-dasharray stroke-dashoffset stroke-linecap stroke-linejoin stroke-miterlimit marker marker-start marker-mid marker-end display visibility opacity filter clip clip-path mask mask-image overflow".split(
    " ",
  ),
);
const TEXT_METADATA_PROPERTIES = new Set(
  "font-family font-size font-weight font-style text-anchor line-height text-align vertical-align cursor background background-color margin".split(
    " ",
  ),
);

function appliedProperties(element: XmlElement, css: Rule[]): Set<string> {
  const applied = new Set(declarations(element.getAttribute("style") ?? "").keys());
  for (const rule of css)
    if (matches(element, rule.selector))
      for (const property of rule.values.keys()) applied.add(property);
  return applied;
}

function assertPropertyEffect(element: XmlElement, css: Rule[], property: string) {
  if (property.startsWith("--") || TEXT_METADATA_PROPERTIES.has(property)) return;
  if (property === "max-width") {
    const value = computed(element, property, css, "none").trim();
    if (
      element.localName !== "svg" ||
      !new RegExp(`^${NUMBER}px$`).test(value) ||
      numeric(value.slice(0, -2)) <= 0
    )
      return fail("unsupported:viewport-max-width");
    return;
  }
  if (property.startsWith("animation") || property.startsWith("transition")) {
    const value = computed(element, property, css, "none").trim().toLowerCase();
    if (
      ["animation", "animation-name", "transition", "transition-property"].includes(property) &&
      value === "none"
    )
      return;
    return fail(`unsupported:active-css-effect:${property}`);
  }
  if (!PAINT_PROPERTIES.has(property)) return fail(`unsupported:css-property-effect:${property}`);
}

function assertDeclarationSchema(element: XmlElement, css: Rule[]) {
  for (const property of appliedProperties(element, css))
    assertPropertyEffect(element, css, property);
  const overflow = computed(element, "overflow", css, "visible").trim().toLowerCase();
  if (overflow !== "visible") return fail("unsupported:visibility-effect:overflow");
  const cap = computed(element, "stroke-linecap", css, "butt").trim().toLowerCase();
  if (!["butt", "round", "square"].includes(cap)) return fail("unsupported:stroke-linecap");
  const join = computed(element, "stroke-linejoin", css, "miter").trim().toLowerCase();
  if (!["miter", "round", "bevel"].includes(join)) return fail("unsupported:stroke-linejoin");
}

function assertSourcePreservingFilter(element: XmlElement, css: Rule[]) {
  const value = computed(element, "filter", css, "none").trim().toLowerCase();
  if (value === "none") return;
  const shadow = value.match(/^drop-shadow\(\s*(.+?)\s*\)$/)?.[1];
  if (!shadow) return fail("unsupported:visibility-effect:filter");
  const unit = `${NUMBER}(?:px)?`;
  const match = shadow.match(new RegExp(`^(${unit})\\s+(${unit})(?:\\s+(${unit}))?(?:\\s+(.+))?$`));
  if (!match) return fail("unsupported:filter-drop-shadow-schema");
  for (const lengthValue of match.slice(1, 4).filter((part) => part !== undefined)) {
    const amount = numeric(lengthValue.replace(/px$/, ""));
    if (amount !== 0 && !lengthValue.endsWith("px"))
      return fail("unsupported:filter-drop-shadow-unit");
  }
  if (match[3] !== undefined && numeric(match[3].replace(/px$/, "")) < 0)
    return fail("unsupported:filter-drop-shadow-blur");
  let color = match[4];
  if (color === "currentcolor")
    color = computed(element, "color", css, "black").trim().toLowerCase();
  if (color !== undefined) {
    if (color === "none") return fail("unsupported:filter-drop-shadow-color");
    paintValuePresent(color);
  }
}

function assertVisible(element: XmlElement, css: Rule[]) {
  assertDeclarationSchema(element, css);
  assertSourcePreservingFilter(element, css);
  for (const property of ["clip-path", "mask", "mask-image", "clip"]) {
    const value = computed(element, property, css, "none").trim().toLowerCase();
    const defaults = property === "clip" ? ["none", "auto"] : ["none"];
    if (!defaults.includes(value)) return fail(`unsupported:visibility-effect:${property}`);
  }
  const display = computed(element, "display", css, "inline").trim().toLowerCase();
  const displayKeywords = ["none", "inline", "block", "inline-block"];
  if (!displayKeywords.includes(display)) return fail("unsupported:display-value");
  if (
    numeric(computed(element, "opacity", css, "1")) <= 0 ||
    display === "none" ||
    computed(element, "visibility", css, "visible").trim().toLowerCase() !== "visible"
  )
    return fail("invalid:element-invisible");
}

function assertVisibleTree(element: XmlElement, css: Rule[]) {
  for (let current: XmlElement | null = element; current; current = parent(current))
    assertVisible(current, css);
}

const CSS_COLOR_NAMES = new Set(
  "aliceblue antiquewhite aqua aquamarine azure beige bisque black blanchedalmond blue blueviolet brown burlywood cadetblue chartreuse chocolate coral cornflowerblue cornsilk crimson cyan darkblue darkcyan darkgoldenrod darkgray darkgreen darkgrey darkkhaki darkmagenta darkolivegreen darkorange darkorchid darkred darksalmon darkseagreen darkslateblue darkslategray darkslategrey darkturquoise darkviolet deeppink deepskyblue dimgray dimgrey dodgerblue firebrick floralwhite forestgreen fuchsia gainsboro ghostwhite gold goldenrod gray green greenyellow grey honeydew hotpink indianred indigo ivory khaki lavender lavenderblush lawngreen lemonchiffon lightblue lightcoral lightcyan lightgoldenrodyellow lightgray lightgreen lightgrey lightpink lightsalmon lightseagreen lightskyblue lightslategray lightslategrey lightsteelblue lightyellow lime limegreen linen magenta maroon mediumaquamarine mediumblue mediumorchid mediumpurple mediumseagreen mediumslateblue mediumspringgreen mediumturquoise mediumvioletred midnightblue mintcream mistyrose moccasin navajowhite navy oldlace olive olivedrab orange orangered orchid palegoldenrod palegreen paleturquoise palevioletred papayawhip peachpuff peru pink plum powderblue purple rebeccapurple red rosybrown royalblue saddlebrown salmon sandybrown seagreen seashell sienna silver skyblue slateblue slategray slategrey snow springgreen steelblue tan teal thistle tomato turquoise violet wheat white whitesmoke yellow yellowgreen".split(
    " ",
  ),
);

function colorArguments(body: string): {
  channels: string[];
  alpha: string | null;
  legacy: boolean;
} {
  if (body.includes(",")) {
    if (body.includes("/")) return fail("unsupported:color-mixed-separators");
    const tokens = body.split(",").map((token) => token.trim());
    if (tokens.length !== 3 && tokens.length !== 4) return fail("unsupported:color-legacy-arity");
    return { channels: tokens.slice(0, 3), alpha: tokens[3] ?? null, legacy: true };
  }
  const parts = body.trim().split("/");
  if (parts.length > 2) return fail("unsupported:color-alpha-separator");
  const channels = (parts[0] ?? "").trim().split(/\s+/);
  if (channels.length !== 3) return fail("unsupported:color-modern-arity");
  return { channels, alpha: parts[1]?.trim() ?? null, legacy: false };
}

function colorComponent(value: string, percentageRequired: boolean): number {
  const suffix = percentageRequired ? "%" : "%?";
  if (!new RegExp(`^${NUMBER}${suffix}$`).test(value))
    return fail(`unsupported:color-component:${value}`);
  return numeric(value.replace(/%$/, ""));
}

function validateColorChannels(family: string, channels: string[], legacy: boolean) {
  if (family.startsWith("hsl")) {
    const hue = channels[0] ?? "";
    if (!new RegExp(`^${NUMBER}(?:deg)?$`).test(hue)) return fail(`unsupported:color-hue:${hue}`);
    numeric(hue.replace(/deg$/, ""));
    colorComponent(channels[1] ?? "", true);
    colorComponent(channels[2] ?? "", true);
    return;
  }
  for (const channel of channels) colorComponent(channel, false);
  if (legacy && new Set(channels.map((channel) => channel.endsWith("%"))).size !== 1)
    return fail("unsupported:color-legacy-mixed-units");
}

function functionalPaint(value: string): boolean {
  const match = value.match(/^(rgba?|hsla?)\(([^()]*)\)$/);
  if (!match) return fail(`unsupported:paint:${value}`);
  const { channels, alpha, legacy } = colorArguments(match[2] ?? "");
  validateColorChannels(match[1] ?? "", channels, legacy);
  return alpha === null || colorComponent(alpha, false) > 0;
}

function paintPresent(
  element: XmlElement,
  property: string,
  css: Rule[],
  fallback: string,
): boolean {
  let value = computed(element, property, css, fallback).trim().toLowerCase();
  if (value === "currentcolor")
    value = computed(element, "color", css, "black").trim().toLowerCase();
  return paintValuePresent(value);
}

function paintValuePresent(value: string): boolean {
  if (["none", "transparent"].includes(value)) return false;
  if (/^#[a-f\d]{3}$|^#[a-f\d]{6}$/.test(value)) return true;
  if (/^#[a-f\d]{4}$|^#[a-f\d]{8}$/.test(value))
    return Number.parseInt(value.slice(value.length === 5 ? -1 : -2), 16) > 0;
  if (CSS_COLOR_NAMES.has(value)) return true;
  return functionalPaint(value);
}

function assertSolidStroke(element: XmlElement, css: Rule[]) {
  const value = computed(element, "stroke-dasharray", css, "none").trim().toLowerCase();
  if (value === "none") return;
  const parts = value.split(/\s*,\s*|\s+/);
  if (!parts.length || parts.some((part) => !new RegExp(`^${NUMBER}(?:px|%)?$`).test(part)))
    return fail("unsupported:stroke-dasharray-schema");
  const lengths = parts.map((part) => numeric(part.replace(/px$|%$/, "")));
  if (lengths.some((value) => value < 0)) return fail("unsupported:stroke-dasharray-negative");
  const pattern = lengths.length % 2 === 0 ? lengths : [...lengths, ...lengths];
  if (pattern.some((value, index) => index % 2 === 1 && value !== 0))
    return fail("unsupported:stroke-nonsolid-dash");
}

function visibleStroke(element: XmlElement, css: Rule[]): boolean {
  const visible =
    paintPresent(element, "stroke", css, "none") &&
    numeric(computed(element, "stroke-width", css, "1").replace(/px$/, "")) > 0 &&
    numeric(computed(element, "stroke-opacity", css, "1")) > 0;
  if (visible) assertSolidStroke(element, css);
  return visible;
}

function visibleFill(element: XmlElement, css: Rule[]): boolean {
  return (
    paintPresent(element, "fill", css, "black") &&
    numeric(computed(element, "fill-opacity", css, "1")) > 0
  );
}

function assertTextVisible(text: XmlElement, css: Rule[]) {
  for (const element of [text, ...named(text, "tspan")]) {
    if (!(element.textContent ?? "").trim()) continue;
    assertVisibleTree(element, css);
    const hasDirectText = Array.from(element.childNodes).some(
      (child) => [3, 4].includes(child.nodeType) && (child.nodeValue ?? "").trim() !== "",
    );
    if (hasDirectText) fontSize(element, css);
    if (hasDirectText && !visibleFill(element, css) && !visibleStroke(element, css))
      return fail("invalid:text-invisible-paint");
  }
}

function assertMarkerPaint(shape: XmlElement, marker: XmlElement, css: Rule[]) {
  if (paintPresent(shape, "fill", css, "black")) return fail("invalid:marker-not-hollow");
  if (!visibleStroke(shape, css)) return fail("invalid:marker-invisible-stroke");
  for (
    let element: XmlElement | null = shape;
    element && element !== parent(marker);
    element = parent(element)
  ) {
    assertVisible(element, css);
  }
}

function assertMarkerReference(marker: XmlElement, shape: XmlElement, vertices: Point[]) {
  const refX = numeric(marker.getAttribute("refX") ?? "0"),
    refY = numeric(marker.getAttribute("refY") ?? "0");
  const triangle = bounds(
    vertices.map((vertex) => point(transform(shape.getAttribute("transform") ?? ""), vertex)),
  );
  if (
    refX < triangle.left ||
    refX > triangle.right ||
    refY < triangle.top ||
    refY > triangle.bottom
  )
    return fail("invalid:marker-reference-outside");
  const viewBox = marker.getAttribute("viewBox");
  if (viewBox) {
    const values = numbers(viewBox);
    if (values.length !== 4 || (values[2] ?? 0) <= 0 || (values[3] ?? 0) <= 0)
      return fail("unsupported:marker-viewbox");
  }
}

function hollowMarker(marker: XmlElement, css: Rule[]): number {
  if (
    marker.getAttribute("markerUnits") !== "userSpaceOnUse" ||
    marker.getAttribute("orient") !== "auto"
  )
    return fail("unsupported:marker-coordinate-system");
  const width = numeric(marker.getAttribute("markerWidth") ?? ""),
    height = numeric(marker.getAttribute("markerHeight") ?? "");
  if (width <= 0 || height <= 0) return fail("invalid:marker-size");
  const children = Array.from(marker.childNodes).filter(
    (child): child is XmlElement => child.nodeType === 1,
  );
  if (children.length !== 1) return fail("unsupported:marker-shapes");
  const shape = children[0] as XmlElement;
  assertMarkerReference(marker, shape, markerVertices(shape));
  assertMarkerPaint(shape, marker, css);
  // 接続線の短縮を marker の実寸法内だけ許容する。図固有の座標は使わない。
  return Math.max(width, height);
}

export function inspectClassReference(
  svg: string,
  language: ClassReferenceLanguage,
): ClassReferenceIntegrity {
  const report: ClassReferenceIntegrity = {
    accepted: false,
    reasons: [],
    nodes: [],
    edges: 0,
    scope: "class-inheritance-text-anchors",
  };
  try {
    const { root, css } = readSvg(svg);
    const contract = CONTRACTS[language];
    inspectNodes(root, css, contract, report);
    assertSeparation(report.nodes);
    inspectEdges(root, css, contract, report);
    report.accepted = true;
  } catch (error) {
    report.reasons.push(error instanceof Error ? error.message : "unsupported:exception");
  }
  return report;
}

function assertSvgSchema(root: XmlElement) {
  const all = [root, ...descendants(root)];
  const nodes = [
    ...Array.from(root.ownerDocument?.childNodes ?? []),
    ...all.flatMap((element) => Array.from(element.childNodes)),
  ];
  if (nodes.some((node) => node.nodeType === 7))
    return fail("unsupported:xml-processing-instruction");
  const tags = new Set(
    "svg g defs style text tspan path rect circle polygon marker filter feDropShadow linearGradient stop".split(
      " ",
    ),
  );
  const attributes = new Set(
    "id class role aria-roledescription cx cy d dx dy fill flood-color flood-opacity font-family font-size font-style font-weight gradientUnits height marker-start marker-mid marker-end markerHeight markerUnits markerWidth offset orient points r rx ry refX refY stdDeviation stop-color stop-opacity stroke stroke-opacity stroke-dasharray stroke-dashoffset stroke-linecap stroke-linejoin stroke-miterlimit stroke-width fill-opacity display visibility opacity filter clip clip-path mask mask-image overflow style transform viewBox width x x1 x2 y y1 y2 text-anchor".split(
      " ",
    ),
  );
  for (const element of all) assertSvgElementSchema(element, tags, attributes);
}

function assertSvgElementSchema(element: XmlElement, tags: Set<string>, attributes: Set<string>) {
  if (element.namespaceURI !== "http://www.w3.org/2000/svg")
    return fail("unsupported:svg-namespace");
  if (!tags.has(element.localName ?? "")) return fail(`unsupported:svg-tag:${element.localName}`);
  for (const attribute of Array.from(element.attributes))
    assertSvgAttributeSchema(attribute.name, attribute.namespaceURI, attributes);
}

function assertSvgAttributeSchema(name: string, namespace: string | null, attributes: Set<string>) {
  if (namespace === "http://www.w3.org/2000/xmlns/") return;
  if (namespace) return fail("unsupported:svg-attribute-namespace");
  if (!attributes.has(name) && !/^(?:data-|aria-)/.test(name))
    return fail(`unsupported:svg-attribute:${name}`);
}

function readSvg(svg: string): { root: XmlElement; css: Rule[] } {
  if (/<!DOCTYPE|<!ENTITY/i.test(svg)) return fail("unsupported:xml-external-declaration");
  const document = new DOMParser({
    onError: (_level, message) => fail(`unsupported:xml:${message}`),
  }).parseFromString(svg, "image/svg+xml");
  const root = document.documentElement;
  if (root?.localName !== "svg" || root.namespaceURI !== "http://www.w3.org/2000/svg")
    return fail("unsupported:svg-root");
  if (
    ["svg", "foreignObject", "use", "script", "animate", "animateTransform", "set"].some(
      (name) => named(root, name).length,
    )
  )
    return fail("unsupported:svg-embedded-layout");
  assertSvgSchema(root);
  const css = rules(root);
  if (
    css.some((rule) =>
      ["transform", "translate", "rotate", "scale"].some((property) => rule.values.has(property)),
    ) ||
    [root, ...descendants(root)].some((element) =>
      ["transform", "translate", "rotate", "scale"].some((property) =>
        declarations(element.getAttribute("style") ?? "").has(property),
      ),
    )
  )
    return fail("unsupported:css-transform");
  for (const element of [root, ...descendants(root)])
    for (const property of [
      "d",
      "x",
      "y",
      "dx",
      "dy",
      "width",
      "height",
      "cx",
      "cy",
      "r",
      "rx",
      "ry",
    ])
      if (specified(element, property, css, false))
        return fail(`unsupported:css-geometry:${property}`);
  return { root, css };
}

function inspectNodes(
  root: XmlElement,
  css: Rule[],
  contract: ClassContract[],
  report: ClassReferenceIntegrity,
) {
  const nodes = descendants(root).filter((element) => classes(element).includes("node"));
  if (nodes.length !== 4) return fail(`invalid:node-count:${nodes.length}`);
  for (const expected of contract) {
    const found = nodes.filter((node) =>
      (node.getAttribute("id") ?? "").includes(`classId-${expected.name}-`),
    );
    if (found.length !== 1) return fail(`invalid:node-identity:${expected.name}`);
    const node = found[0] as XmlElement;
    assertVisibleTree(node, css);
    assertContent(node, "label-group", [expected.name]);
    assertContent(node, "members-group", expected.members);
    assertContent(node, "methods-group", expected.methods);
    const texts = named(node, "text");
    for (const text of texts) assertTextVisible(text, css);
    if (texts.length !== 1 + expected.members.length + expected.methods.length)
      return fail(`invalid:node-extra-text:${expected.name}`);
    const box = outerBox(node, css);
    const anchors = texts.map((text) => textAnchor(text, css));
    report.nodes.push({ name: expected.name, box, anchors });
    if (
      anchors.some(
        ([x, y]) =>
          !Number.isFinite(x) ||
          !Number.isFinite(y) ||
          x < box.left ||
          x > box.right ||
          y < box.top ||
          y > box.bottom,
      )
    )
      return fail(`invalid:text-anchor-outside:${expected.name}`);
  }
}

function assertSeparation(nodes: ClassReferenceIntegrity["nodes"]) {
  for (let a = 0; a < nodes.length; a++)
    for (let b = a + 1; b < nodes.length; b++) {
      const x = nodes[a]?.box as Box,
        y = nodes[b]?.box as Box;
      if (
        Math.min(x.right, y.right) > Math.max(x.left, y.left) &&
        Math.min(x.bottom, y.bottom) > Math.max(x.top, y.top)
      )
        return fail(`invalid:node-overlap:${a}:${b}`);
    }
}

function inspectEdges(
  root: XmlElement,
  css: Rule[],
  contract: ClassContract[],
  report: ClassReferenceIntegrity,
) {
  const parentBox = report.nodes[0]?.box as Box;
  const edges = named(root, "path").filter(
    (path) => path.getAttribute("data-edge") === "true" || classes(path).includes("relation"),
  );
  report.edges = edges.length;
  if (edges.length !== 3) return fail(`invalid:edge-count:${edges.length}`);
  const used = new Set<XmlElement>();
  for (let index = 1; index < contract.length; index++) {
    const name = contract[index]?.name ?? "";
    const child = report.nodes[index]?.box as Box;
    if (parentBox.bottom >= child.top) return fail(`invalid:parent-not-above:${name}`);
    const found = edges.filter((edge) =>
      (edge.getAttribute("data-id") ?? edge.getAttribute("id") ?? "").includes(
        `id_${contract[0]?.name}_${name}_`,
      ),
    );
    if (found.length !== 1 || used.has(found[0] as XmlElement))
      return fail(`invalid:edge-identity:${name}`);
    const edge = found[0] as XmlElement;
    used.add(edge);
    const { start, end, margin } = edgeGeometry(root, css, edge);
    assertEndpoints(start, end, margin, parentBox, child, name);
  }
}

function edgeGeometry(
  root: XmlElement,
  css: Rule[],
  edge: XmlElement,
): { start: Point; end: Point; margin: number } {
  assertVisibleTree(edge, css);
  for (let current: XmlElement | null = edge; current; current = parent(current))
    if (specified(current, "marker", css)) return fail("unsupported:marker-shorthand");
  if (!visibleStroke(edge, css)) return fail("invalid:edge-invisible-stroke");
  for (const property of ["marker-mid", "marker-end"])
    if (computed(edge, property, css, "none").trim().toLowerCase() !== "none")
      return fail("unsupported:edge-marker-direction");
  const markerId = computed(edge, "marker-start", css, "none")
    .trim()
    .match(/^url\(#([^()]+)\)$/)?.[1];
  const markers = named(root, "marker").filter((marker) => marker.getAttribute("id") === markerId);
  if (!markerId || markers.length !== 1) return fail("invalid:marker-reference");
  const margin =
    hollowMarker(markers[0] as XmlElement, css) * Math.max(world(edge)[0], world(edge)[1]);
  const points = pathPoints(edge.getAttribute("d") ?? "", true).map((p) => point(world(edge), p));
  const start = points[0] as Point,
    end = points[points.length - 1] as Point;
  if (!start || !end) return fail("invalid:edge-empty");
  return { start, end, margin };
}

function assertEndpoints(
  start: Point,
  end: Point,
  margin: number,
  parentBox: Box,
  child: Box,
  name: string,
) {
  const epsilon =
    Number.EPSILON *
    64 *
    Math.max(1, ...Object.values(child), ...start.map(Math.abs), ...end.map(Math.abs));
  if (
    start[0] < parentBox.left ||
    start[0] > parentBox.right ||
    start[1] < parentBox.bottom - epsilon ||
    start[1] - parentBox.bottom > margin ||
    end[0] < child.left ||
    end[0] > child.right ||
    Math.abs(end[1] - child.top) > epsilon
  )
    return fail(`invalid:edge-endpoint:${name}`);
}
