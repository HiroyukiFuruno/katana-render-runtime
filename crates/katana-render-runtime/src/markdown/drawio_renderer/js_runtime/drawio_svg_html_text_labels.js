const KATANA_DRAWIO_NEGATIVE_EDGE_MARKUP_LABEL_X_OFFSET = -40;

function katanaInstallDrawioHtmlTextLabels(svg) {
  katanaDrawioHtmlTextLabelEntries().forEach((entry) => {
    katanaInstallDrawioHtmlTextLabel(svg, entry);
  });
  katanaNormalizeDrawioNativeEdgeHtmlTextLabels(svg);
}

function katanaDrawioHtmlTextLabelEntries() {
  return katanaDrawioSourceLabelEntries()
    .map(katanaDrawioHtmlTextLabelEntry)
    .filter(katanaHasDrawioHtmlTextLabel);
}

function katanaDrawioHtmlTextLabelEntry(entry) {
  return {
    id: entry.id,
    html: katanaDrawioHtmlTextLabelHtml(entry.value),
    label: entry.label,
    edge: entry.edge,
    replaceExisting: katanaShouldReplaceDrawioHtmlTextLabel(entry.value),
    style: entry.style,
  };
}

function katanaDrawioHtmlTextLabelHtml(html) {
  const normalizedHtml = katanaDrawioHtmlTextNormalizedLabelHtml(html);
  return katanaDrawioIsDarkMode()
    ? normalizedHtml
        .replaceAll('color="#000000"', 'color="#ededed"')
        .replaceAll('color="#ffffff"', 'color="#121212"')
        .replaceAll('color="#FFFFFF"', 'color="#121212"')
    : normalizedHtml;
}

function katanaDrawioHtmlTextNormalizedLabelHtml(html) {
  const normalized = katanaDrawioHtmlTextHasMarkup(html)
    ? katanaTrimDrawioEdgeHtmlTextBreaks(String(html))
    : katanaDrawioEscapedPlainHtmlText(html);
  return normalized.replace(/&nbsp;/gi, " ");
}

function katanaTrimDrawioEdgeHtmlTextBreaks(html) {
  return katanaTrimDrawioTrailingHtmlTextBreaks(
    katanaTrimDrawioLeadingHtmlTextBreaks(html),
  );
}

function katanaTrimDrawioLeadingHtmlTextBreaks(html) {
  return String(html).replace(
    /^(?:(?:\s|&nbsp;)+|<br(?:\s[^>]*)?\s*\/?>|<div>\s*(?:<br(?:\s[^>]*)?\s*\/?>|&nbsp;)?\s*<\/div>)+/i,
    "",
  );
}

function katanaTrimDrawioTrailingHtmlTextBreaks(html) {
  return String(html).replace(
    /(?:(?:\s|&nbsp;)+|<br(?:\s[^>]*)?\s*\/?>|<div>\s*(?:<br(?:\s[^>]*)?\s*\/?>|&nbsp;)?\s*<\/div>)+$/i,
    "",
  );
}

function katanaDrawioHtmlTextHasMarkup(html) {
  return /<\/?[a-zA-Z][^>]*>/.test(String(html));
}

function katanaDrawioEscapedPlainHtmlText(html) {
  return katanaDrawioPlainHtmlTextLines(html)
    .map(katanaDrawioEscapePlainHtmlTextLine)
    .join("<br>");
}

function katanaDrawioPlainHtmlTextLines(html) {
  const lines = String(html).split(/\r?\n/);
  return katanaTrimDrawioEmptyEdgeTextLines(lines);
}

function katanaTrimDrawioEmptyEdgeTextLines(lines) {
  let start = 0;
  let end = lines.length;
  while (start < end && lines[start].trim() === "") {
    start += 1;
  }
  while (end > start && lines[end - 1].trim() === "") {
    end -= 1;
  }
  return lines.slice(start, end);
}

function katanaDrawioEscapePlainHtmlTextLine(line) {
  return String(line)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

function katanaShouldReplaceDrawioHtmlTextLabel(html) {
  return [
    !katanaDrawioHtmlTextHasMarkup(html),
    String(html).includes("\n"),
  ].every(Boolean);
}

function katanaHasDrawioHtmlTextLabel(entry) {
  return [
    entry.id,
    entry.html,
    entry.label,
    katanaShouldInstallDrawioHtmlTextLabel(entry),
  ].every(Boolean);
}

function katanaShouldInstallDrawioHtmlTextLabel(entry) {
  if (katanaDrawioShouldKeepNativeEdgeHtmlTextLabel(entry)) {
    return false;
  }
  return [
    katanaDrawioStyleValue(entry.style, "html") === "1",
    entry.replaceExisting,
  ].some(Boolean);
}

function katanaDrawioShouldKeepNativeEdgeHtmlTextLabel(entry) {
  return [entry.edge, katanaDrawioHtmlTextHasMarkup(entry.html)].every(Boolean);
}

function katanaNormalizeDrawioNativeEdgeHtmlTextLabels(svg) {
  const geometryById = katanaDrawioSourceGeometryById();
  katanaDrawioNativeEdgeHtmlTextLabelEntries()
    .filter((entry) => katanaDrawioShouldShiftNegativeEdgeHtmlTextLabel(entry, geometryById))
    .map((entry) => katanaDrawioCellGroup(svg, entry.id))
    .filter(Boolean)
    .forEach(katanaShiftDrawioNativeEdgeTextLabels);
}

function katanaDrawioSourceGeometryById() {
  return new Map(katanaDrawioSourceGeometryEntries().map((entry) => [entry.id, entry]));
}

function katanaDrawioNativeEdgeHtmlTextLabelEntries() {
  return katanaDrawioSourceLabelEntries()
    .map(katanaDrawioHtmlTextLabelEntry)
    .filter(katanaHasDrawioNativeEdgeHtmlTextLabel);
}

function katanaHasDrawioNativeEdgeHtmlTextLabel(entry) {
  return [entry.id, entry.html, entry.label].every(Boolean);
}

function katanaDrawioShouldShiftNegativeEdgeHtmlTextLabel(entry, geometryById) {
  return [
    katanaDrawioShouldKeepNativeEdgeHtmlTextLabel(entry),
    geometryById.get(entry.id)?.x < 0,
  ].every(Boolean);
}

function katanaShiftDrawioNativeEdgeTextLabels(group) {
  Array.from(group.childNodes)
    .filter((node) => node.localName === "text")
    .forEach((node) => {
      node.setAttribute(
        "transform",
        `translate(${KATANA_DRAWIO_NEGATIVE_EDGE_MARKUP_LABEL_X_OFFSET},0)`,
      );
    });
}

function katanaInstallDrawioHtmlTextLabel(svg, entry) {
  [katanaDrawioCellGroup(svg, entry.id)]
    .filter(Boolean)
    .filter((group) => katanaCanInstallDrawioHtmlTextLabel(group, entry))
    .map((group) => katanaDrawioHtmlTextLabelContext(group, entry))
    .filter(katanaHasDrawioHtmlTextLabelContext)
    .forEach(katanaApplyDrawioHtmlTextLabel);
}

function katanaCanInstallDrawioHtmlTextLabel(group, entry) {
  return [entry.replaceExisting, !katanaDrawioGroupHasForeignObject(group)].some(Boolean);
}

function katanaDrawioHtmlTextLabelContext(group, entry) {
  return {
    group,
    entry,
    box: katanaDrawioCellShapeBox(group),
    fallback: katanaDrawioTextNodes(group),
  };
}

function katanaHasDrawioHtmlTextLabelContext(context) {
  return [context.box, context.fallback.length > 0].every(Boolean);
}

function katanaApplyDrawioHtmlTextLabel(context) {
  if (context.entry.replaceExisting) {
    katanaRemoveDrawioHtmlTextForeignObjects(context.group);
  }
  const label = katanaCreateDrawioHtmlTextLabel(context.entry, context.box, context.fallback);
  context.group.appendChild(label);
}

function katanaRemoveDrawioHtmlTextForeignObjects(group) {
  Array.from(group.querySelectorAll("foreignObject")).forEach(katanaRemoveDrawioNode);
}

function katanaCreateDrawioHtmlTextLabel(entry, box, fallback) {
  const group = katanaCreateDrawioSvgElement("g");
  const switchNode = katanaCreateDrawioSvgElement("switch");
  switchNode.appendChild(katanaCreateDrawioHtmlTextForeignObject(entry, box));
  const normalizedFallback = katanaNormalizedDrawioHtmlTextFallbackNodes(fallback, entry, box);
  normalizedFallback.forEach((node) => {
    switchNode.appendChild(node);
  });
  group.appendChild(switchNode);
  return group;
}

function katanaNormalizedDrawioHtmlTextFallbackNodes(nodes, entry, box) {
  const lines = String(entry.label)
    .split(/\n+/)
    .map((line) => line.trim())
    .filter(Boolean);
  if (lines.length === 0) {
    return nodes;
  }
  return nodes.map((node) => katanaNormalizedDrawioHtmlTextFallbackNode(node, lines, entry, box));
}

function katanaNormalizedDrawioHtmlTextFallbackNode(node, lines, entry, box) {
  if (node.tagName !== "text") {
    return node;
  }
  katanaRepositionDrawioTopHtmlTextFallbackNode(node, entry, box);
  const tspans = Array.from(node.querySelectorAll("tspan"));
  if (tspans.length === 0) {
    node.replaceChildren(
      ...lines.map((line, index) => katanaDrawioLineBreakTspan(node, line, index)),
    );
    return node;
  }
  const mergedLines = lines.slice(0, tspans.length);
  if (lines.length > tspans.length) {
    const head = lines.slice(0, tspans.length - 1);
    const tail = lines.slice(tspans.length - 1).join(" ");
    mergedLines.splice(0, mergedLines.length, ...head, tail);
  }
  tspans.forEach((tspan, index) => {
    tspan.textContent = mergedLines[index] ?? "";
  });
  return node;
}

function katanaRepositionDrawioTopHtmlTextFallbackNode(node, entry, box) {
  if (!katanaDrawioHtmlTextAlignTop(entry.style)) {
    return;
  }
  const y = katanaDrawioHtmlTextTop(entry.style, box) + katanaDrawioFontSize(entry.style);
  node.setAttribute("y", String(Math.round(y)));
}

function katanaCreateDrawioHtmlTextForeignObject(entry, box) {
  const foreignObject = katanaCreateDrawioSvgElement("foreignObject");
  foreignObject.setAttribute(
    "style",
    "overflow: visible; text-align: left;",
  );
  foreignObject.setAttribute("pointer-events", "none");
  foreignObject.setAttribute("width", "100%");
  foreignObject.setAttribute("height", "100%");
  foreignObject.setAttribute(
    "requiredFeatures",
    "http://www.w3.org/TR/SVG11/feature#Extensibility",
  );
  foreignObject.appendChild(katanaCreateDrawioHtmlTextOuterDiv(entry, box));
  return foreignObject;
}

function katanaCreateDrawioHtmlTextOuterDiv(entry, box) {
  const div = katanaCreateDrawioHtmlElement("div");
  div.setAttribute("xmlns", "http://www.w3.org/1999/xhtml");
  div.setAttribute("style", katanaDrawioHtmlTextOuterStyle(entry.style, box));
  div.appendChild(katanaCreateDrawioHtmlTextBoxDiv(entry, box));
  return div;
}

function katanaCreateDrawioHtmlTextBoxDiv(entry, box) {
  const div = katanaCreateDrawioHtmlElement("div");
  div.setAttribute("style", katanaDrawioHtmlTextBoxStyle(entry.style, box));
  div.appendChild(katanaCreateDrawioHtmlTextContentDiv(entry));
  return div;
}

function katanaCreateDrawioHtmlTextContentDiv(entry) {
  const div = katanaCreateDrawioHtmlElement("div");
  div.setAttribute("data-katana-drawio-html-text", "content");
  div.setAttribute("style", katanaDrawioHtmlTextContentStyle(entry.style));
  div.innerHTML = entry.html;
  katanaNormalizeDrawioHtmlTextContentColors(div);
  katanaPreserveDrawioHtmlTextDarkBackgroundColor(div, entry.style);
  return div;
}

function katanaPreserveDrawioHtmlTextDarkBackgroundColor(element, style) {
  const color = katanaDrawioColorKey(katanaDrawioTextColor(style));
  if (katanaDrawioKeepsLightHtmlTextOnDarkBackground(style, color)) {
    element.setAttribute("style", katanaDrawioHtmlStyleWithColor(element.getAttribute("style"), "#ffffff"));
  }
}

function katanaNormalizeDrawioHtmlTextContentColors(root) {
  if (!katanaDrawioIsDarkMode()) {
    return;
  }
  katanaDrawioHtmlTextContentDescendants(root)
    .filter((node) => node.getAttribute("style"))
    .forEach(katanaApplyDrawioHtmlTextStyleColor);
  katanaDrawioHtmlTextContentDescendants(root)
    .filter((node) => node.getAttribute("color"))
    .forEach(katanaApplyDrawioHtmlTextColorAttribute);
}

function katanaDrawioHtmlTextContentDescendants(root) {
  return Array.from(root.querySelectorAll("*"));
}

function katanaApplyDrawioHtmlTextColorAttribute(element) {
  const color = katanaDrawioHtmlTextExplicitColor(element.getAttribute("color"));
  if (color) {
    element.setAttribute("style", katanaDrawioHtmlStyleWithColor(element.getAttribute("style"), color));
    katanaMarkDrawioHtmlTextExplicitColor(element);
  }
}

function katanaApplyDrawioHtmlTextStyleColor(element) {
  const style = element.getAttribute("style");
  const color = katanaDrawioHtmlTextExplicitColor(katanaDrawioStylePropertyValue(style, "color"));
  if (color) {
    element.setAttribute("style", katanaDrawioHtmlStyleWithColor(style, color));
    katanaMarkDrawioHtmlTextExplicitColor(element);
  }
}

function katanaMarkDrawioHtmlTextExplicitColor(element) {
  element.setAttribute("data-katana-drawio-html-text-explicit-color", "1");
}

function katanaDrawioHtmlTextExplicitColor(color) {
  if ([katanaDrawioIsDarkMode(), katanaDrawioHtmlTextIsExplicitWhite(color)].every(Boolean)) {
    return "#121212";
  }
  return [color]
    .filter(Boolean)
    .map((it) => katanaDrawioResolvedTextColor(katanaDrawioColorKey(it)))
    .filter(Boolean)
    .concat([""])[0];
}

function katanaDrawioHtmlTextIsExplicitWhite(color) {
  return ["#fff", "#ffffff", "rgb(255, 255, 255)", "white"].includes(katanaDrawioColorKey(color));
}

function katanaDrawioHtmlStyleWithColor(style, color) {
  const text = String(style ?? "").trim();
  if (katanaDrawioStylePropertyValue(text, "color")) {
    return katanaDrawioStyleWithProperty(text, "color", color);
  }
  return [text, `color: ${color}`].filter(Boolean).join("; ");
}

function katanaCreateDrawioSvgElement(tagName) {
  return document.createElementNS("http://www.w3.org/2000/svg", tagName);
}

function katanaCreateDrawioHtmlElement(tagName) {
  return document.createElementNS("http://www.w3.org/1999/xhtml", tagName);
}
