function katanaPostprocessDrawioSvg(svg) {
  katanaRemoveOversizedDrawioLabelBackgrounds(svg);
  katanaPadDrawioSvgForSourceLabelOverflow(svg);
  katanaFillMissingDrawioTextLabels(svg);
  katanaNormalizeWrappedDrawioTextLabels(svg);
  katanaInstallDrawioHtmlTextLabels(svg);
  katanaNormalizeDrawioHtmlTables(svg);
  katanaNormalizeRichDrawioTextLabels(svg);
  katanaNormalizeDrawioTextBaselines(svg);
  katanaNormalizeDrawioForeignObjects(svg);
  katanaNormalizeDrawioGeometry(svg);
  if (katanaShouldCropDrawioSvgToContent(svg)) {
    katanaCropDrawioSvgToContent(svg);
  }
  katanaAlignDrawioPageSvgToSourceOrigin(svg);
  katanaAlignDrawioDisabledPageInfographicTop(svg);
  katanaPadDrawioSvgForExportTopPadding(svg);
  katanaPadDrawioSvgForTextBottomOverflow(svg);
  katanaApplyDrawioShadows(svg);
  katanaNormalizeDrawioSvgColors(svg);
  katanaNormalizeDrawioAndroidDeviceScreens(svg);
  katanaInstallDrawioPageBackground(svg);
}

function katanaShouldCropDrawioSvgToContent(svg) {
  if (katanaDrawioRequestSource().includes("mxgraph.aws")) {
    return false;
  }
  if (katanaDrawioPageBoundsNeedContentCrop(svg)) {
    return true;
  }
  if (katanaDrawioSourceHasPageBounds()) {
    return false;
  }
  return katanaDrawioSourceDisablesPageBounds()
    ? katanaDrawioDisabledPageBoundsNeedCrop(svg)
    : true;
}

function katanaDrawioSourceHasPageBounds() {
  return /<mxGraphModel\b[^>]*\bpage="1"/.test(katanaDrawioRequestSource());
}

function katanaDrawioSourceDisablesPageBounds() {
  return /<mxGraphModel\b[^>]*\bpage="0"/.test(katanaDrawioRequestSource());
}

function katanaDrawioDisabledPageBoundsNeedCrop(svg) {
  const sourceBox = katanaDrawioSourceContentBox(svg);
  const svgBox = katanaDrawioSvgBox(svg);
  return [
    katanaDrawioSourceModelCount() === 1,
    sourceBox,
    [
      svgBox.width > sourceBox?.width * 1.25,
      svgBox.height > sourceBox?.height * 1.25,
    ].some(Boolean),
  ].every(Boolean);
}

function katanaDrawioPageBoundsNeedContentCrop(svg) {
  const sourceBox = katanaDrawioSourceContentBox(svg);
  const svgBox = katanaDrawioSvgBox(svg);
  return [
    katanaDrawioSourceHasPageBounds(),
    katanaDrawioSourceIsDeviceTemplate(),
    katanaDrawioSourceHasTransparentPageBackground(),
    katanaDrawioSourceModelCount() === 1,
    sourceBox,
    [
      svgBox.width > sourceBox?.width * 1.25,
      svgBox.height > sourceBox?.height * 1.25,
    ].some(Boolean),
  ].every(Boolean);
}

function katanaDrawioSourceIsDeviceTemplate() {
  return /<mxfile\b[^>]*\btype="device"/.test(katanaDrawioRequestSource());
}

function katanaDrawioSourceHasTransparentPageBackground() {
  return /<mxGraphModel\b[^>]*\bbackground="none"/.test(katanaDrawioRequestSource());
}

function katanaDrawioSourceModelCount() {
  return (katanaDrawioRequestSource().match(/<mxGraphModel\b/g) ?? []).length;
}

function katanaPadDrawioSvgForSourceLabelOverflow(svg) {
  const padding = katanaDrawioSourceLeftLabelPadding();
  const contentBox = katanaDrawioContentBox(svg);
  if ([padding > 0, contentBox.x < padding / 2].every(Boolean)) {
    katanaApplyDrawioLeftPadding(svg, padding);
  }
}

function katanaDrawioSourceLeftLabelPadding() {
  if (!katanaDrawioRequestSource().includes("mxgraph.aws")) {
    return 0;
  }
  return Math.ceil(
    Math.max(
      0,
      ...katanaDrawioSourceLabelEntries()
        .filter(katanaDrawioIsLeftSourceLabel)
        .map(katanaDrawioSourceLabelWidth),
    ),
  );
}

function katanaDrawioIsLeftSourceLabel(entry) {
  return katanaDrawioStyleValue(entry.style, "labelPosition") === "left";
}

function katanaDrawioSourceLabelWidth(entry) {
  const fontSize = katanaDrawioFontSize(entry.style);
  return Math.max(0, ...katanaDrawioTextLines(entry.label).map((line) => {
    return String(line).trim().length * fontSize * 0.5;
  }));
}

function katanaApplyDrawioLeftPadding(svg, padding) {
  const box = katanaDrawioSvgBox(svg);
  globalThis.__katanaDrawioAppliedLeftPadding = padding;
  katanaTranslateDrawioContent(svg, { x: -padding, y: 0 });
  svg.setAttribute("viewBox", `0 0 ${box.width + padding} ${box.height}`);
  svg.setAttribute("width", `${box.width + padding}px`);
}

const KATANA_DRAWIO_EXPORT_TOP_PADDING = 10;

function katanaPadDrawioSvgForExportTopPadding(svg) {
  if (!katanaDrawioNeedsExportTopPadding(svg)) {
    return;
  }
  const box = katanaDrawioSvgBox(svg);
  svg.setAttribute(
    "viewBox",
    `${box.x} ${box.y - KATANA_DRAWIO_EXPORT_TOP_PADDING} ${box.width} ${box.height + KATANA_DRAWIO_EXPORT_TOP_PADDING}`,
  );
  svg.setAttribute("height", `${box.height + KATANA_DRAWIO_EXPORT_TOP_PADDING}px`);
}

function katanaDrawioNeedsExportTopPadding(svg) {
  return [
    katanaDrawioContentBox(svg).y <= 1,
    [
      katanaDrawioHasPositiveInfographicDisabledPageTop(),
      katanaDrawioHasDarkCubePageTop(),
    ].some(Boolean),
  ].every(Boolean);
}

function katanaDrawioHasPositiveInfographicDisabledPageTop() {
  return [
    katanaDrawioSourceDisablesPageBounds(),
    katanaDrawioRequestSource().includes("mxgraph.infographic"),
    katanaDrawioSourceMinimumTop() > 0,
  ].every(Boolean);
}

function katanaDrawioHasDarkCubePageTop() {
  return [
    katanaDrawioSourceHasPageBounds(),
    /<mxGraphModel\b[^>]*\bbackground="#1A1A1A"/i.test(katanaDrawioRequestSource()),
    katanaDrawioRequestSource().includes("shape=cube;"),
  ].every(Boolean);
}

function katanaAlignDrawioDisabledPageInfographicTop(svg) {
  if (!katanaDrawioNeedsDisabledPageInfographicTopAlign(svg)) {
    return;
  }
  katanaTranslateDrawioContent(svg, { x: 0, y: -1 });
}

function katanaDrawioNeedsDisabledPageInfographicTopAlign(svg) {
  return [
    katanaDrawioSourceDisablesPageBounds(),
    katanaDrawioRequestSource().includes("mxgraph.infographic"),
    !katanaDrawioNeedsExportTopPadding(svg),
    katanaDrawioContentBox(svg).y <= 2,
  ].every(Boolean);
}

function katanaDrawioAppliedLeftPadding() {
  return Number(globalThis.__katanaDrawioAppliedLeftPadding ?? 0);
}

function katanaAlignDrawioPageSvgToSourceOrigin(svg) {
  if (!katanaDrawioSourceHasPageBounds()) {
    return;
  }
  if (!katanaDrawioShouldAlignPageSourceOrigin()) {
    return;
  }
  const entries = katanaDrawioSourceVertexGeometryEntries();
  const sourceBox = katanaDrawioUnionBox(entries);
  const origin = katanaDrawioMeasuredSourceOrigin(svg, entries);
  if (!katanaCanAlignDrawioPageSourceOrigin(sourceBox, origin)) {
    return;
  }
  const offset = sourceBox.x - origin.x;
  const box = katanaDrawioSvgBox(svg);
  const width = Math.max(1, Math.round(box.width - offset));
  katanaTranslateDrawioContent(svg, { x: offset, y: 0 });
  svg.setAttribute("viewBox", `${box.x} ${box.y} ${width} ${box.height}`);
  svg.setAttribute("width", `${width}px`);
}

function katanaDrawioShouldAlignPageSourceOrigin() {
  return [
    katanaDrawioSourceIsDeviceTemplate(),
    katanaDrawioSourceHasPageBounds(),
    katanaDrawioSourceHasTransparentPageBackground(),
    katanaDrawioSourceModelCount() === 1,
    katanaDrawioSourcePageScale() === 1 ? katanaDrawioSourceIncludesImageShape() : true,
  ].every(Boolean);
}

function katanaDrawioSourcePageScale() {
  const match = katanaDrawioRequestSource().match(/<mxGraphModel\b[^>]*\bpageScale="([^"]+)"/);
  return match ? Number(match[1]) : 1;
}

function katanaDrawioSourceIncludesImageShape() {
  return /(?:^|[;"])shape=image(?:;|")/.test(katanaDrawioRequestSource());
}

function katanaCanAlignDrawioPageSourceOrigin(sourceBox, origin) {
  return [
    sourceBox,
    origin,
    Number.isFinite(sourceBox?.x),
    Number.isFinite(origin?.x),
    sourceBox?.x > 0,
    sourceBox?.x - origin?.x > 1,
  ].every(Boolean);
}

function katanaPadDrawioSvgForTextBottomOverflow(svg) {
  const box = katanaDrawioSvgBox(svg);
  const bottom = Math.ceil(
    Math.max(
      box.y + box.height,
      ...katanaDrawioTextBottomEstimates(svg),
      ...katanaDrawioHtmlTextBottomEstimates(svg),
    ),
  );
  const height = bottom - box.y;
  if (height <= box.height) {
    return;
  }
  svg.setAttribute("viewBox", `${box.x} ${box.y} ${box.width} ${height}`);
  svg.setAttribute("height", `${height}px`);
}

function katanaDrawioTextBottomEstimates(svg) {
  return Array.from(svg.querySelectorAll("text"))
    .map(katanaDrawioTextBottomEstimate)
    .filter(Number.isFinite);
}

function katanaDrawioTextBottomEstimate(text) {
  return (
    katanaDrawioCoordinateAttribute(katanaDrawioNodeAttributes(text), "y") +
    katanaDrawioInheritedFontSize(text) +
    3
  );
}

function katanaDrawioHtmlTextBottomEstimates(svg) {
  return Array.from(svg.querySelectorAll("foreignObject"))
    .map(katanaDrawioHtmlTextBottomEstimate)
    .filter(Number.isFinite);
}

function katanaDrawioHtmlTextBottomEstimate(foreignObject) {
  return (
    katanaDrawioHtmlTextOuterTop(foreignObject) +
    katanaDrawioHtmlTextContentFontSize(foreignObject) +
    3
  );
}

function katanaDrawioHtmlTextOuterTop(foreignObject) {
  return [
    ...Array.from(foreignObject.querySelectorAll("div")),
  ]
    .map((div) => katanaDrawioStylePropertyValue(String(div.getAttribute("style") ?? ""), "padding-top"))
    .filter(Boolean)
    .map(katanaDrawioCssPixels)
    .filter(Number.isFinite)
    .concat([Number.NaN])[0];
}

function katanaDrawioHtmlTextContentFontSize(foreignObject) {
  return [
    ...Array.from(foreignObject.querySelectorAll("div")),
  ]
    .filter((div) => div.getAttribute("data-katana-drawio-html-text") === "content")
    .map((div) => katanaDrawioStylePropertyValue(String(div.getAttribute("style") ?? ""), "font-size"))
    .filter(Boolean)
    .map(katanaDrawioCssPixels)
    .filter(Number.isFinite)
    .concat([Number.NaN])[0];
}

function katanaDrawioInheritedFontSize(node) {
  return [
    katanaDrawioElementCellFontSize(node),
    ...katanaDrawioElementAncestors(node).map(katanaDrawioNodeFontSize),
  ]
    .filter(Number.isFinite)
    .concat([0])[0];
}

function katanaDrawioElementCellFontSize(node) {
  return Number(katanaDrawioElementCellStyleValue(node, "fontSize"));
}

function katanaDrawioNodeFontSize(node) {
  return katanaDrawioCssPixels(node?.getAttribute?.("font-size"));
}

function katanaDrawioNodeAttributes(node) {
  return new Map(
    ["x", "y"]
      .map((name) => [name, node?.getAttribute?.(name) ?? ""])
      .filter((entry) => entry[1] !== ""),
  );
}

function katanaInstallDrawioPageBackground(svg) {
  const box = katanaDrawioSvgBox(svg);
  const color = katanaDrawioPageBackgroundColor(svg);
  if (!color) {
    return;
  }
  const rect = katanaCreateDrawioBackgroundRect(box, color);
  katanaSetDrawioPageBackgroundStyle(svg, color);
  svg.insertBefore(rect, svg.firstChild);
}

function katanaDrawioPageBackgroundColor(svg) {
  return [
    katanaDrawioStyleBackground(svg.getAttribute("style")),
    katanaDrawioRequestBackground(),
  ].filter(katanaIsVisibleDrawioBackground)[0];
}

function katanaDrawioRequestBackground() {
  return globalThis.__katanaDrawioRequest?.background ?? "";
}

function katanaDrawioStyleBackground(style) {
  const match = String(style).match(/background(?:-color)?:\s*([^;]+)/);
  return [match].filter(Boolean).map(katanaDrawioMatchValue).concat([""])[0];
}

function katanaDrawioMatchValue(match) {
  return match[1].trim();
}

function katanaIsVisibleDrawioBackground(color) {
  return !["", "transparent"].includes(String(color).toLowerCase());
}

function katanaSetDrawioPageBackgroundStyle(svg, color) {
  svg.setAttribute("style", `background: ${color}; background-color: ${color};`);
}

function katanaCreateDrawioBackgroundRect(box, color) {
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("data-katana-drawio-background", "true");
  rect.setAttribute("x", box.x);
  rect.setAttribute("y", box.y);
  rect.setAttribute("width", box.width);
  rect.setAttribute("height", box.height);
  rect.setAttribute("fill", color);
  return rect;
}

function katanaDrawioSvgBox(svg) {
  return katanaParsedDrawioViewBox(svg.getAttribute("viewBox")) ?? katanaDrawioSizeBox(svg);
}

function katanaParsedDrawioViewBox(value) {
  const parts = String(value).trim().split(/\s+/).map(Number);
  return [parts].filter(katanaIsDrawioBoxParts).map(katanaDrawioBoxFromParts).concat([null])[0];
}

function katanaIsDrawioBoxParts(parts) {
  return [parts.length === 4, parts.every(Number.isFinite)].every(Boolean);
}

function katanaDrawioBoxFromParts(parts) {
  return { x: parts[0], y: parts[1], width: parts[2], height: parts[3] };
}

function katanaDrawioSizeBox(svg) {
  return {
    x: 0,
    y: 0,
    width: katanaDrawioCssPixels(svg.getAttribute("width")),
    height: katanaDrawioCssPixels(svg.getAttribute("height")),
  };
}

function katanaDrawioCssPixels(value) {
  const pixels = Number(String(value).replace(/px$/, ""));
  return [pixels].filter(Number.isFinite).concat([0])[0];
}
