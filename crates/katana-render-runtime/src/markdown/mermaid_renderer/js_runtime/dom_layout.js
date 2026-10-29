const KATANA_HIDDEN_LAYOUT_TAGS = new Set([
  "style",
  "script",
  "defs",
  "marker",
  "lineargradient",
  "radialgradient",
  "stop",
  "filter",
  "fedropshadow",
  "clippath",
  "mask",
  "pattern",
]);

function katanaOwnText(node) {
  return String(node?._textContent ?? "");
}

function katanaTextContent(node) {
  return node ? `${katanaOwnText(node)}${katanaTextContentChildren(node)}` : "";
}

function katanaTextContentChildren(node) {
  return (node.children ?? []).map((child) => katanaTextContent(child)).join("");
}

function katanaLayoutTextContent(node) {
  if (katanaIsHiddenLayoutNode(node)) {
    return "";
  }
  return `${katanaOwnText(node)}${katanaLayoutTextChildren(node)}`;
}

function katanaLayoutTextChildren(node) {
  return (node.children ?? []).map((child) => katanaLayoutTextContent(child)).join("");
}

function katanaIsHiddenLayoutNode(node) {
  return !node || KATANA_HIDDEN_LAYOUT_TAGS.has(node.localName) || katanaIsDisplayNone(node);
}

function katanaNodeValue(value) {
  if (value instanceof KatanaNode) {
    return value;
  }
  return document.createTextNode(katanaStringValue(value));
}

function katanaStringValue(value) {
  return String(value ?? "");
}

function katanaNumberAttr(node, name) {
  return katanaNullableNumber(node?.getAttribute?.(name));
}

function katanaNullableNumber(rawValue) {
  return katanaHasNumberValue(rawValue) ? katanaFiniteNumber(Number(rawValue)) : null;
}

function katanaHasNumberValue(rawValue) {
  return ![null, undefined, ""].includes(rawValue);
}

function katanaFiniteNumber(value) {
  return Number.isFinite(value) ? value : null;
}

function katanaBox(x, y, width, height) {
  return { x, y, width, height, w: width, h: height };
}

function katanaUnionBox(boxes) {
  if (boxes.length === 0) {
    return katanaBox(0, 0, 0, 0);
  }
  const minX = Math.min(...boxes.map((box) => box.x));
  const minY = Math.min(...boxes.map((box) => box.y));
  const maxX = Math.max(...boxes.map((box) => box.x + box.width));
  const maxY = Math.max(...boxes.map((box) => box.y + box.height));
  return katanaBox(minX, minY, maxX - minX, maxY - minY);
}

function katanaVisibleChildBoxes(node) {
  const children = node.children ?? [];
  return children.flatMap((child) => {
    if (KATANA_HIDDEN_LAYOUT_TAGS.has(child.localName) || katanaIsDisplayNone(child)) {
      return [];
    }
    const box = child.getBBox();
    if (!katanaHasArea(box) && !katanaHasDegenerateGeometry(child)) {
      return [];
    }
    return [katanaTransformChildBox(child, box)];
  });
}

function katanaIsDisplayNone(node) {
  const inlineDisplay = node?.style?.getPropertyValue?.("display") ?? node?.style?.display ?? "";
  const display = String(inlineDisplay || node?.getAttribute?.("display") || "")
    .trim()
    .toLowerCase();
  return display === "none";
}

function katanaHasDegenerateGeometry(node, positionedTextGroupDepth = 0) {
  if (!node || KATANA_HIDDEN_LAYOUT_TAGS.has(node.localName) || katanaIsDisplayNone(node)) {
    return false;
  }
  if (["text", "tspan"].includes(node.localName)) {
    // Chromeの空文字点は自身または1段のg経由まで親bboxへ伝播する。
    return (
      katanaLayoutTextContent(node).trim().length > 0 ||
      (positionedTextGroupDepth <= 1 && katanaHasPositionedSvgText(node))
    );
  }
  if (node.localName === "line") {
    return true;
  }
  if (node.localName === "path" && String(node.getAttribute?.("d") ?? "").trim()) {
    return true;
  }
  const nextPositionedTextGroupDepth =
    node.localName === "g" ? positionedTextGroupDepth + 1 : positionedTextGroupDepth;
  return (node.children ?? []).some((child) =>
    katanaHasDegenerateGeometry(child, nextPositionedTextGroupDepth),
  );
}

function katanaMeasuredBox(node) {
  if (KATANA_HIDDEN_LAYOUT_TAGS.has(node.localName)) {
    return katanaBox(0, 0, 0, 0);
  }
  return katanaVisibleMeasuredBox(node);
}

function katanaVisibleMeasuredBox(node) {
  return (
    katanaExplicitMeasuredBox(node) ??
    katanaDirectShapeBox(node) ??
    katanaChildTextMeasuredBox(node)
  );
}

function katanaExplicitMeasuredBox(node) {
  if (katanaIsRoundSvgShape(node)) {
    return null;
  }
  const attrs = katanaBoxAttributes(node);
  if ([attrs.width > 0, attrs.height > 0].includes(true)) {
    return katanaBox(attrs.x, attrs.y, attrs.width, attrs.height);
  }
  return null;
}

function katanaBoxAttributes(node) {
  return {
    x: katanaNumberAttrOrDefault(node, "x", 0),
    y: katanaNumberAttrOrDefault(node, "y", 0),
    width: katanaNumberAttrOrDefault(node, "width", 0),
    height: katanaNumberAttrOrDefault(node, "height", 0),
  };
}

function katanaNumberAttrOrDefault(node, name, fallback) {
  const value = katanaNumberAttr(node, name);
  if (value === null) {
    return fallback;
  }
  return value;
}

function katanaDirectShapeBox(node) {
  return katanaLineBox(node) ?? katanaCircleBox(node);
}

function katanaChildTextMeasuredBox(node) {
  const childBox = katanaUnionBox(katanaVisibleChildBoxes(node));
  const text = katanaMeasuredNodeText(node, childBox);
  return katanaBox(
    0,
    0,
    katanaMeasuredTextWidth(text, childBox),
    katanaMeasuredTextHeight(text, childBox),
  );
}

function katanaMeasuredNodeText(node, childBox) {
  return katanaOwnText(node) || katanaFallbackLayoutText(node, childBox);
}

function katanaFallbackLayoutText(node, childBox) {
  return childBox.width === 0 ? katanaLayoutTextContent(node) : "";
}

function katanaMeasuredTextWidth(text, childBox) {
  return Math.max(16, katanaTextWidthFallback(text), childBox.width);
}

function katanaMeasuredTextHeight(text, childBox) {
  return Math.max(24, katanaTextHeightFallback(text), childBox.height);
}

function katanaTextWidthFallback(text) {
  return text.length === 0 ? 0 : Math.max(16, katanaTextWidth(text));
}

function katanaTextHeightFallback(text) {
  return text.length === 0 ? 0 : 24;
}

function katanaLineBox(node) {
  if (node.localName !== "line") {
    return null;
  }
  return katanaLineShapeBox(node);
}

function katanaLineShapeBox(node) {
  const x1 = katanaNumberAttrOrDefault(node, "x1", 0);
  const y1 = katanaNumberAttrOrDefault(node, "y1", 0);
  const x2 = katanaNumberAttrOrDefault(node, "x2", 0);
  const y2 = katanaNumberAttrOrDefault(node, "y2", 0);
  return katanaBox(Math.min(x1, x2), Math.min(y1, y2), Math.abs(x2 - x1), Math.abs(y2 - y1));
}

function katanaCircleBox(node) {
  if (!katanaIsRoundSvgShape(node)) {
    return null;
  }
  return katanaRoundShapeBox(node);
}

function katanaRoundShapeBox(node) {
  const cx = katanaNumberAttrOrDefault(node, "cx", 0);
  const cy = katanaNumberAttrOrDefault(node, "cy", 0);
  const rx = katanaRoundRadius(node, "rx");
  const ry = katanaRoundRadius(node, "ry");
  // ブラウザのSVGLength/RectFは演算前からfloat32なので、最後だけの丸めでは辺がずれる。
  const fx = Math.fround(cx),
    fy = Math.fround(cy),
    frx = Math.fround(rx),
    fry = Math.fround(ry);
  if (
    node.namespaceURI !== "http://www.w3.org/2000/svg" ||
    rx < 0 ||
    ry < 0 ||
    ![fx, fy, frx, fry].every(Number.isFinite)
  )
    return katanaBox(cx - rx, cy - ry, rx * 2, ry * 2);
  return katanaBox(
    Math.fround(fx - frx),
    Math.fround(fy - fry),
    Math.fround(frx * 2),
    Math.fround(fry * 2),
  );
}

function katanaRoundRadius(node, name) {
  return katanaNumberAttr(node, name) ?? katanaNumberAttrOrDefault(node, "r", 0);
}

function katanaIsRoundSvgShape(node) {
  return ["circle", "ellipse"].includes(node.localName);
}

function katanaTransformChildBox(node, box) {
  const transform = katanaTransformValue(node).trim();
  if (!transform) return box;
  const translation = katanaSimpleSvgTranslation(transform);
  if (translation) {
    return katanaBox(box.x + translation[0], box.y + translation[1], box.width, box.height);
  }
  const transformed = katanaTransformSvgBox(transform, box.x, box.y, box.width, box.height);
  if (!transformed) throw new Error("SVG child transform returned no bounds");
  return katanaBox(transformed.x, transformed.y, transformed.width, transformed.height);
}

function katanaTransformValue(node) {
  return String(node?.getAttribute?.("transform") ?? "");
}
