const KATANA_DEFAULT_CLIENT_TAGS = new Set(["body", "div", "main", "pre", "section", "article"]);

function katanaMeasuredClientBox(node) {
  const context = katanaClientBoxContext(node);
  return (
    katanaGraphViewerClientBox(context) ??
    katanaDrawioWrappedHtmlClientBox(context) ??
    katanaDrawioEmptyHtmlClientBox(context) ??
    katanaDefaultExplicitZeroBox(context) ??
    katanaExplicitClientBox(context) ??
    katanaSvgClientBox(context) ??
    katanaEmptyDefaultClientBox(context) ??
    context.box
  );
}

function katanaDrawioEmptyHtmlClientBox(context) {
  if (!Number.isFinite(Number(globalThis.__katanaGraphViewerViewportPadding))) {
    return null;
  }
  if (!["div", "span"].includes(context.node.localName)) {
    return null;
  }
  return [
    context.explicitWidth === null,
    context.explicitHeight === null,
    katanaTextContent(context.node).length === 0,
    katanaIsEmptyBox(context.box),
  ].every(Boolean)
    ? context.box
    : null;
}

function katanaDrawioWrappedHtmlClientBox(context) {
  if (
    !Number.isFinite(Number(globalThis.__katanaGraphViewerViewportPadding)) ||
    context.explicitWidth !== null
  ) {
    return null;
  }
  const textNode = katanaWrappedHtmlTextNode(context.node);
  const width = katanaNearestExplicitClientWidth(context.node.parentNode);
  if (!textNode || width === null || context.box.width <= width) {
    return null;
  }
  return katanaBox(
    context.box.x,
    context.box.y,
    width,
    katanaWrappedHtmlTextHeight(textNode, width),
  );
}

function katanaWrappedHtmlTextNode(node) {
  return katanaHtmlElementDescendants(node).find((candidate) =>
    [
      String(candidate.style?.getPropertyValue?.("white-space") ?? "").trim() === "normal",
      katanaTextContent(candidate).trim().length > 0,
    ].every(Boolean),
  );
}

function katanaHtmlElementDescendants(node) {
  return [node].concat(
    Array.from(node.children ?? []).flatMap((child) => katanaHtmlElementDescendants(child)),
  );
}

function katanaNearestExplicitClientWidth(node) {
  if (!node) {
    return null;
  }
  return katanaExplicitClientWidth(node) ?? katanaNearestExplicitClientWidth(node.parentNode);
}

function katanaWrappedHtmlTextHeight(node, width) {
  return Math.ceil(katanaWrappedHtmlLineCount(node, width) * katanaHtmlCssLineHeight(node));
}

function katanaWrappedHtmlLineCount(node, width) {
  const words = katanaTextContent(node).trim().split(/\s+/).filter(Boolean);
  if (words.length === 0) {
    return 0;
  }
  return words.reduce((state, word) => katanaAppendWrappedHtmlWord(state, node, word, width), {
    lines: 1,
    width: 0,
  }).lines;
}

function katanaAppendWrappedHtmlWord(state, node, word, limit) {
  const space = state.width === 0 ? 0 : katanaDrawioWrappedHtmlTextWidth(node, " ");
  const wordWidth = katanaDrawioWrappedHtmlTextWidth(node, word);
  const nextWidth = state.width + space + wordWidth;
  return nextWidth <= limit
    ? { lines: state.lines, width: nextWidth }
    : { lines: state.lines + 1, width: wordWidth };
}

function katanaDrawioWrappedHtmlTextWidth(node, text) {
  const family = String(node.style?.getPropertyValue?.("font-family") ?? "").toLowerCase();
  const scale = family.includes("helvetica") ? KATANA_DRAWIO_HELVETICA_WRAP_WIDTH_SCALE : 1;
  return katanaTextNodeWidth(node, text) * scale;
}

function katanaHtmlCssLineHeight(node) {
  const raw = String(node.style?.getPropertyValue?.("line-height") ?? "").trim();
  const match = raw.match(/^(-?\d+(?:\.\d+)?)(px)?$/);
  if (!match) {
    return katanaLineHeight(node);
  }
  const value = Number(match[1]);
  return match[2] === "px" ? value : value * Number(katanaLineHeightFontSize(node));
}

const KATANA_DRAWIO_HELVETICA_WRAP_WIDTH_SCALE = 0.9611192997399751;

function katanaGraphViewerClientBox(context) {
  const classes = String(context.node.getAttribute?.("class") ?? "").split(/\s+/);
  if (!classes.includes("mxgraph")) {
    return null;
  }
  if (katanaIsEmptyBox(context.box)) {
    return katanaBox(0, 0, 0, 0);
  }
  const padding = Number(globalThis.__katanaGraphViewerViewportPadding ?? 0);
  const availableWidth = Math.max(0, katanaDefaultViewportWidth() - padding);
  const width = katanaGraphViewerUsesAvailableWidth(context.node)
    ? availableWidth
    : Math.min(context.explicitWidth ?? context.box.width, availableWidth);
  const height = context.explicitHeight ?? context.box.height;
  return katanaBox(context.box.x, context.box.y, width, height);
}

function katanaGraphViewerUsesAvailableWidth(node) {
  return (
    String(node.style?.getPropertyValue?.("min-width") ?? node.style?.minWidth ?? "").trim() ===
    "100%"
  );
}

function katanaClientBoxContext(node) {
  return {
    node,
    box: node.getBBox(),
    explicitWidth: katanaExplicitClientWidth(node),
    explicitHeight: katanaExplicitClientHeight(node),
  };
}

function katanaExplicitClientWidth(node) {
  return (
    katanaCssLength(node.style?.getPropertyValue?.("width")) ?? katanaNumberAttr(node, "width")
  );
}

function katanaExplicitClientHeight(node) {
  return (
    katanaCssLength(node.style?.getPropertyValue?.("height")) ?? katanaNumberAttr(node, "height")
  );
}

function katanaDefaultExplicitZeroBox(context) {
  if ([katanaNeedsDefaultClientBox(context.node), context.explicitWidth === 0].every(Boolean)) {
    return katanaBox(
      0,
      0,
      katanaDefaultViewportWidth(),
      katanaPositiveOrDefault(context.explicitHeight, katanaDefaultViewportHeight()),
    );
  }
  return null;
}

function katanaExplicitClientBox(context) {
  if (katanaHasExplicitClientSize(context)) {
    return katanaResolvedExplicitClientBox(context);
  }
  return null;
}

function katanaHasExplicitClientSize(context) {
  return [context.explicitWidth !== null, context.explicitHeight !== null].includes(true);
}

function katanaResolvedExplicitClientBox(context) {
  return katanaBox(
    context.box.x,
    context.box.y,
    katanaExplicitWidthValue(context),
    katanaExplicitHeightValue(context),
  );
}

function katanaExplicitWidthValue(context) {
  return context.explicitWidth ?? context.box.width;
}

function katanaExplicitHeightValue(context) {
  return context.explicitHeight ?? context.box.height;
}

function katanaSvgClientBox(context) {
  if (context.node.localName === "svg") {
    return katanaSvgViewBoxClientBox(context);
  }
  return null;
}

function katanaSvgViewBoxClientBox(context) {
  const viewBox = katanaViewBoxSize(context.node.getAttribute("viewBox"));
  return katanaBox(
    context.box.x,
    context.box.y,
    katanaViewBoxWidth(context, viewBox),
    katanaViewBoxHeight(context, viewBox),
  );
}

function katanaViewBoxWidth(context, viewBox) {
  return viewBox?.[0] ?? Math.max(context.box.width, katanaDefaultViewportWidth());
}

function katanaViewBoxHeight(context, viewBox) {
  return viewBox?.[1] ?? Math.max(context.box.height, katanaDefaultViewportHeight());
}

function katanaEmptyDefaultClientBox(context) {
  if ([katanaNeedsDefaultClientBox(context.node), katanaIsEmptyBox(context.box)].every(Boolean)) {
    return katanaBox(0, 0, katanaDefaultViewportWidth(), katanaDefaultViewportHeight());
  }
  return null;
}

function katanaDefaultViewportWidth() {
  return Number(globalThis.innerWidth ?? globalThis.screen?.width ?? 800);
}

function katanaDefaultViewportHeight() {
  return Number(globalThis.innerHeight ?? globalThis.screen?.height ?? 600);
}

function katanaNeedsDefaultClientBox(node) {
  return [KATANA_DEFAULT_CLIENT_TAGS.has(node.localName), katanaHasSvgChild(node)].includes(true);
}

function katanaIsEmptyBox(box) {
  return [box.width === 0, box.height === 0].every(Boolean);
}

function katanaPositiveOrDefault(value, fallback) {
  return value > 0 ? value : fallback;
}

function katanaCssLength(value) {
  if (!value) {
    return null;
  }
  return katanaFiniteCssLength(value);
}

function katanaFiniteCssLength(value) {
  const number = Number(String(value).replace("px", ""));
  if (Number.isFinite(number)) {
    return number;
  }
  return null;
}

function katanaViewBoxSize(value) {
  const values = katanaViewBoxValues(value);
  if (katanaIsValidViewBox(values)) {
    return [values[2], values[3]];
  }
  return null;
}

function katanaViewBoxValues(value) {
  return String(value ?? "")
    .split(/\s+/)
    .map((it) => Number(it));
}

function katanaIsValidViewBox(values) {
  return [values.length === 4, values.every((it) => Number.isFinite(it))].every(Boolean);
}

function katanaHasSvgChild(node) {
  return (node.children ?? []).some((child) => child.localName === "svg");
}
