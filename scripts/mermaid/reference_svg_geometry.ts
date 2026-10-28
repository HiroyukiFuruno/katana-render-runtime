import { createHash } from "node:crypto";
import { DOMParser, type Element as XmlElement, type Node as XmlNode } from "@xmldom/xmldom";
import {
  type CssNode,
  type Declaration,
  generate,
  type List,
  type ListItem,
  parse,
  walk,
} from "css-tree";

export const NEUTRAL_SVG_GEOMETRY_POLICY = "neutral-svg-geometry-v1";
export const PAINT_PROJECTED_REFERENCE_POLICY = "raw-svg-geometry-candidate-computed-paint-v1";
export const PAINT_PROJECTED_REFERENCE_TARGET_SLUGS = [
  "08-01-c4-context-simple",
  "08-02-c4-context-full",
  "08-03-c4-container",
  "08-04-c4-component",
  "08-05-c4-dynamic",
  "08-06-c4-deployment",
  "22-01-tree-view-simple",
  "22-02-tree-view-file-system",
] as const;
export const PAINT_PROJECTED_REFERENCE_TARGET_SLUGS_JA = [
  "08-01-c4",
  "08-02-c4",
  "08-03-c4",
  "08-04-c4",
  "08-05-c4",
  "08-06-c4",
  "22-01-tree-view-beta",
  "22-02-tree-view-beta",
] as const;
const PAINT_PROJECTED_TARGET_SET = new Set<string>([
  ...PAINT_PROJECTED_REFERENCE_TARGET_SLUGS,
  ...PAINT_PROJECTED_REFERENCE_TARGET_SLUGS_JA,
]);
export const NEUTRAL_GEOMETRY_TARGET_SLUGS = PAINT_PROJECTED_REFERENCE_TARGET_SLUGS;
const PAINT_PROPERTIES = new Set([
  "color",
  "fill",
  "flood-color",
  "lighting-color",
  "stop-color",
  "stroke",
]);
const NUMERIC_TRANSFORM = /[-+]?(?:\d*\.\d+|\d+\.?\d*)(?:e[-+]?\d+)?/gi;
const TRANSFORM_TOLERANCE = 0.00001;
const CSS_FUNCTION = /^(rgba?|hsla?)\((.*)\)$/i;

export interface SvgGeometryComparison {
  accepted: boolean;
  reasons: string[];
  maxTransformDelta: number;
  signatureSha256: string;
}

export interface NeutralSvgSource {
  svg: string;
  signature: string;
  signatureSha256: string;
}

export function isNeutralGeometryTargetSlug(slug: string): boolean {
  return PAINT_PROJECTED_TARGET_SET.has(slug);
}

export const isPaintProjectedReferenceTargetSlug = isNeutralGeometryTargetSlug;

export function assertCompletePaintProjectedTargetSlugs(
  pairCount: number,
  slugs: readonly string[],
): void {
  if (new Set(slugs).size !== slugs.length) {
    throw new Error("Paint projection target slugs are duplicated");
  }
  if (pairCount !== 59 && pairCount !== 58) return;
  const expected =
    pairCount === 58
      ? PAINT_PROJECTED_REFERENCE_TARGET_SLUGS_JA
      : PAINT_PROJECTED_REFERENCE_TARGET_SLUGS;
  if (slugs.length !== expected.length || expected.some((slug) => !slugs.includes(slug))) {
    throw new Error(
      "Full language comparison requires all eight paint-projected C4 and TreeView targets",
    );
  }
}

export function prepareNeutralSvg(source: string): NeutralSvgSource {
  const root = parseSvg(source);
  const ids = idAliases(root);
  assertSvgNeutralizable(root);
  const signature = JSON.stringify(canonicalElement(root, ids));
  return {
    svg: source,
    signature,
    signatureSha256: digest(signature),
  };
}

export function compareSvgGeometry(
  left: NeutralSvgSource,
  right: NeutralSvgSource,
): SvgGeometryComparison {
  return compareSvgGeometryInternal(left, right, true);
}

export function compareSvgGeometryForNeutralRaster(
  left: NeutralSvgSource,
  right: NeutralSvgSource,
): SvgGeometryComparison {
  return compareSvgGeometryInternal(left, right, false);
}

function compareSvgGeometryInternal(
  left: NeutralSvgSource,
  right: NeutralSvgSource,
  comparePaintAttributes: boolean,
): SvgGeometryComparison {
  const leftRoot = parseSvg(left.svg);
  const rightRoot = parseSvg(right.svg);
  const reasons: string[] = [];
  const leftIds = idAliases(leftRoot),
    rightIds = idAliases(rightRoot);
  const maxTransformDelta = compareElements(
    leftRoot,
    rightRoot,
    leftIds,
    rightIds,
    "$svg",
    reasons,
    comparePaintAttributes,
  );
  return {
    accepted: reasons.length === 0,
    reasons,
    maxTransformDelta,
    signatureSha256: digest(`${left.signature}\n${right.signature}`),
  };
}

function parseSvg(source: string): XmlElement {
  const document = new DOMParser({
    onError: (level, message) => {
      throw new Error(`Neutral geometry SVG parse ${level}: ${message}`);
    },
  }).parseFromString(source, "image/svg+xml");
  const root = document.documentElement;
  if (
    document.doctype !== null ||
    root === null ||
    root.localName !== "svg" ||
    root.namespaceURI !== "http://www.w3.org/2000/svg" ||
    document.getElementsByTagName("parsererror").length > 0
  ) {
    throw new Error("Neutral geometry requires one well-formed SVG root");
  }
  return root;
}

function idAliases(root: XmlElement): Map<string, string> {
  const aliases = new Map<string, string>();
  const elements = [root, ...Array.from(root.getElementsByTagName("*"))];
  for (const [index, element] of elements.entries()) {
    const id = element.getAttribute("id");
    if (typeof id === "string" && id !== "") aliases.set(id, `krr-id-${index}`);
  }
  return aliases;
}

function canonicalElement(element: XmlElement, ids: Map<string, string>): unknown {
  const attributes = Array.from(element.attributes)
    .map(
      (attribute) =>
        [attribute.name, canonicalAttribute(attribute.name, attribute.value, ids)] as const,
    )
    .sort(([left], [right]) => left.localeCompare(right));
  const children = Array.from(element.childNodes).flatMap((child) => {
    if (child.nodeType === 1) return [canonicalElement(child as XmlElement, ids)];
    if (child.nodeType !== 3 && child.nodeType !== 4) return [];
    const text = child.nodeValue ?? "";
    if (text.trim() === "" && !textBearing(element)) return [];
    return [element.localName === "style" ? canonicalCss(text, "stylesheet", ids) : text];
  });
  return { name: element.nodeName, attributes, children };
}

function canonicalAttribute(
  name: string,
  value: string,
  ids: Map<string, string>,
  ignorePaintDeclarations = false,
): string {
  const normalizedName = name.toLowerCase();
  if (normalizedName === "id") return ids.get(value) ?? value;
  if (normalizedName === "style")
    return canonicalCss(value, "declarationList", ids, ignorePaintDeclarations);
  if (PAINT_PROPERTIES.has(normalizedName)) {
    return value.trim().toLowerCase().startsWith("url(")
      ? canonicalAttributeReferences(normalizedName, value, ids)
      : paintToken(value);
  }
  return canonicalAttributeReferences(normalizedName, value, ids);
}

function canonicalCss(
  source: string,
  context: "stylesheet" | "declarationList",
  ids: Map<string, string>,
  ignorePaintDeclarations = false,
): string {
  const ast = parse(source, { context });
  if (ignorePaintDeclarations) removePaintDeclarations(ast);
  walk(ast, (node) => {
    if (node.type === "Declaration") canonicalDeclaration(node);
    if (node.type === "IdSelector") node.name = referenceAlias(node.name, ids);
    if (node.type === "Url" && node.value.startsWith("#")) {
      node.value = `#${referenceAlias(node.value.slice(1), ids)}`;
    }
  });
  return generate(ast);
}

function removePaintDeclarations(ast: ReturnType<typeof parse>) {
  walk(ast, (node: CssNode, item: ListItem<CssNode>, list: List<CssNode>) => {
    if (node.type === "Declaration" && PAINT_PROPERTIES.has(node.property.toLowerCase())) {
      list.remove(item);
    }
  });
  walk(ast, {
    reverse: true,
    enter(node: CssNode, item: ListItem<CssNode>, list: List<CssNode>) {
      if (node.type === "Rule" && node.block.children.isEmpty) list.remove(item);
      if (node.type === "Atrule" && node.block?.children.isEmpty) list.remove(item);
    },
  });
}

function referenceAlias(id: string, ids: Map<string, string>): string {
  const existing = ids.get(id);
  return existing ?? "krr-unresolved-reference";
}

function canonicalDeclaration(node: Declaration) {
  if (!PAINT_PROPERTIES.has(node.property.toLowerCase())) return;
  let hasUrlReference = false;
  walk(node.value, (child) => {
    if (child.type === "Url") hasUrlReference = true;
  });
  if (hasUrlReference) return;
  const value = generate(node.value);
  const token = paintToken(value);
  node.value = parse(token, { context: "value" }) as Declaration["value"];
}

function canonicalAttributeReferences(
  name: string,
  value: string,
  ids: Map<string, string>,
): string {
  if (name === "href" || name.endsWith(":href")) {
    if (!value.startsWith("#")) return value;
    const id = value.slice(1);
    return `#${referenceAlias(id, ids)}`;
  }
  if (name === "aria-labelledby" || name === "aria-describedby") {
    return value
      .split(/\s+/)
      .map((id) => referenceAlias(id, ids))
      .join(" ");
  }
  return value.replace(
    /url\(\s*(['"]?)#([^\s)'"]+)\1\s*\)/g,
    (_reference, _quote: string, id: string) => `url(#${referenceAlias(id, ids)})`,
  );
}

function paintToken(value: string): string {
  const normalized = value.trim().toLowerCase();
  if (normalized === "none") return "none";
  return `krr-neutral-paint/${paintAlpha(normalized)}`;
}

function paintAlpha(value: string): string {
  if (value === "transparent") return "0";
  if (value === "currentcolor") return "currentColor";
  const hex = value.match(/^#([\da-f]{3,8})$/i)?.[1];
  if (hex !== undefined) return hexAlpha(hex);
  const functionMatch = value.match(CSS_FUNCTION);
  if (functionMatch !== null) return functionAlpha(functionMatch, value);
  if (/^[a-z][a-z\d-]*$/i.test(value)) return value === "transparent" ? "0" : "1";
  throw new Error(`Neutral geometry does not support paint syntax: ${value}`);
}

function hexAlpha(hex: string): string {
  if (![3, 4, 6, 8].includes(hex.length)) {
    throw new Error(`Neutral geometry does not support paint syntax: #${hex}`);
  }
  if (hex.length === 4) return normalizeAlpha(Number.parseInt(hex[3] ?? "0", 16) / 15);
  if (hex.length === 8) return normalizeAlpha(Number.parseInt(hex.slice(6), 16) / 255);
  return "1";
}

function functionAlpha(match: RegExpMatchArray, value: string): string {
  const family = match[1]?.toLowerCase() ?? "";
  const components = (match[2] ?? "").trim();
  const parts = components.includes(",")
    ? components.split(",").map((part) => part.trim())
    : components.replace("/", " ").split(/\s+/);
  if (parts.length === 3 && ["rgb", "hsl"].includes(family)) return "1";
  if (parts.length !== 4 || !["rgb", "rgba", "hsl", "hsla"].includes(family))
    throw new Error(`Neutral geometry does not support paint syntax: ${value}`);
  const alphaText = parts[3] ?? "";
  const alpha = Number.parseFloat(alphaText.replace(/%$/, ""));
  if (!Number.isFinite(alpha)) {
    throw new Error(`Neutral geometry has invalid paint alpha: ${value}`);
  }
  return normalizeAlpha(alphaText.endsWith("%") ? alpha / 100 : alpha);
}

function normalizeAlpha(value: number): string {
  if (!Number.isFinite(value)) throw new Error("Neutral geometry paint alpha is not finite");
  return Math.min(1, Math.max(0, value)).toString();
}

function compareElements(
  left: XmlElement,
  right: XmlElement,
  leftIds: Map<string, string>,
  rightIds: Map<string, string>,
  path: string,
  reasons: string[],
  comparePaintAttributes: boolean,
): number {
  if (left.nodeName !== right.nodeName) reasons.push(`${path}:element-order-or-name`);
  const maxAttributeDelta = compareAttributes(
    left,
    right,
    leftIds,
    rightIds,
    path,
    reasons,
    comparePaintAttributes,
  );
  const maxChildDelta = compareChildren(
    left,
    right,
    leftIds,
    rightIds,
    path,
    reasons,
    comparePaintAttributes,
  );
  return Math.max(maxAttributeDelta, maxChildDelta);
}

function compareAttributes(
  left: XmlElement,
  right: XmlElement,
  leftIds: Map<string, string>,
  rightIds: Map<string, string>,
  path: string,
  reasons: string[],
  comparePaintAttributes: boolean,
): number {
  let maxDelta = 0;
  const leftAttributes = attributeMap(left, comparePaintAttributes),
    rightAttributes = attributeMap(right, comparePaintAttributes);
  if (leftAttributes.size !== rightAttributes.size) reasons.push(`${path}:attribute-count`);
  for (const [name, leftValue] of leftAttributes) {
    maxDelta = Math.max(
      maxDelta,
      compareAttribute(
        name,
        leftValue,
        rightAttributes.get(name),
        leftIds,
        rightIds,
        path,
        reasons,
        comparePaintAttributes,
      ),
    );
  }
  return maxDelta;
}

function compareAttribute(
  name: string,
  left: string,
  right: string | undefined,
  leftIds: Map<string, string>,
  rightIds: Map<string, string>,
  path: string,
  reasons: string[],
  comparePaintAttributes: boolean,
): number {
  if (right === undefined) {
    reasons.push(`${path}:missing-attribute:${name}`);
    return 0;
  }
  if (name.toLowerCase() === "transform") {
    const delta = compareTransforms(left, right);
    if (delta === null) {
      reasons.push(`${path}:transform`);
      return 0;
    }
    if (delta > TRANSFORM_TOLERANCE) reasons.push(`${path}:transform`);
    return delta;
  }
  if (
    canonicalAttribute(name, left, leftIds, !comparePaintAttributes) !==
    canonicalAttribute(name, right, rightIds, !comparePaintAttributes)
  ) {
    reasons.push(`${path}:attribute:${name}`);
  }
  return 0;
}

function compareChildren(
  left: XmlElement,
  right: XmlElement,
  leftIds: Map<string, string>,
  rightIds: Map<string, string>,
  path: string,
  reasons: string[],
  comparePaintAttributes: boolean,
): number {
  const leftChildren = canonicalChildren(left, leftIds, !comparePaintAttributes),
    rightChildren = canonicalChildren(right, rightIds, !comparePaintAttributes);
  if (leftChildren.length !== rightChildren.length) reasons.push(`${path}:child-count`);
  let maxDelta = 0;
  for (const [index, child] of leftChildren.entries()) {
    const other = rightChildren.at(index);
    maxDelta = Math.max(
      maxDelta,
      compareChild(
        child,
        other,
        leftIds,
        rightIds,
        `${path}/${index}`,
        reasons,
        comparePaintAttributes,
      ),
    );
  }
  return maxDelta;
}

function compareChild(
  child: { element: XmlElement | null; text: string },
  other: { element: XmlElement | null; text: string } | undefined,
  leftIds: Map<string, string>,
  rightIds: Map<string, string>,
  path: string,
  reasons: string[],
  comparePaintAttributes: boolean,
): number {
  if (other === undefined) return 0;
  if (child.element === null || other.element === null) {
    if (child.text !== other.text) reasons.push(`${path}:text-content`);
    return 0;
  }
  return compareElements(
    child.element,
    other.element,
    leftIds,
    rightIds,
    path,
    reasons,
    comparePaintAttributes,
  );
}

function attributeMap(element: XmlElement, comparePaintAttributes: boolean): Map<string, string> {
  return new Map(
    Array.from(element.attributes)
      .filter(
        (attribute) =>
          comparePaintAttributes || !PAINT_PROPERTIES.has(attribute.name.toLowerCase()),
      )
      .map((attribute) => [attribute.name, attribute.value]),
  );
}

function canonicalChildren(
  element: XmlElement,
  ids: Map<string, string>,
  ignorePaintDeclarations: boolean,
) {
  const children: Array<{ element: XmlElement | null; text: string }> = [];
  for (const node of Array.from(element.childNodes as Iterable<XmlNode>)) {
    if (node.nodeType === 1) {
      children.push({ element: node as XmlElement, text: "" });
      continue;
    }
    if (node.nodeType !== 3 && node.nodeType !== 4) continue;
    const value = node.nodeValue ?? "";
    if (value.trim() === "" && !textBearing(element)) continue;
    children.push({
      element: null,
      text:
        element.localName === "style"
          ? canonicalCss(value, "stylesheet", ids, ignorePaintDeclarations)
          : value,
    });
  }
  return children;
}

function textBearing(element: XmlElement): boolean {
  return ["text", "tspan", "textPath", "title", "desc"].includes(element.localName ?? "");
}

function compareTransforms(left: string, right: string): number | null {
  const leftPrefix = left.replace(NUMERIC_TRANSFORM, "#"),
    rightPrefix = right.replace(NUMERIC_TRANSFORM, "#");
  if (leftPrefix !== rightPrefix) return null;
  const leftValues = left.match(NUMERIC_TRANSFORM)?.map(Number) ?? [],
    rightValues = right.match(NUMERIC_TRANSFORM)?.map(Number) ?? [];
  if (leftValues.length !== rightValues.length) return null;
  let maxDelta = 0;
  for (const [index, value] of leftValues.entries()) {
    const other = rightValues.at(index);
    if (other === undefined || !Number.isFinite(value) || !Number.isFinite(other)) return null;
    maxDelta = Math.max(maxDelta, Math.abs(value - other));
  }
  return maxDelta;
}

function assertSvgNeutralizable(root: XmlElement) {
  for (const element of [root, ...Array.from(root.getElementsByTagName("*"))]) {
    if (element.localName === "foreignObject" || element.localName === "image") {
      throw new Error(`Neutral geometry does not support SVG ${element.localName}`);
    }
  }
}

function digest(value: string): string {
  return createHash("sha256").update(value).digest("hex");
}
