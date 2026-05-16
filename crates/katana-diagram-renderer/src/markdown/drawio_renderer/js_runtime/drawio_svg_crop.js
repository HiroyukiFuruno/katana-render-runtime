const KATANA_DRAWIO_EXTERNAL_IMAGE_EXPORT_BOTTOM_PADDING = 11;
const KATANA_DRAWIO_SHADOW_EXPORT_RIGHT_PADDING = 10;
const KATANA_DRAWIO_SHADOW_EXPORT_BOTTOM_PADDING = 11;
const KATANA_DRAWIO_SOURCE_TOP_PADDING_LIMIT = 12;

function katanaRemoveOversizedDrawioLabelBackgrounds(svg) {
  Array.from(svg.querySelectorAll("rect"))
    .filter(katanaIsDrawioTextLabelBackground)
    .forEach(katanaRemoveDrawioNode);
  Array.from(svg.querySelectorAll("rect"))
    .filter(katanaIsDrawioPageSizedLabelBackground)
    .forEach(katanaRemoveDrawioNode);
}

function katanaRemoveDrawioNode(node) {
  node.parentNode?.removeChild(node);
}

function katanaIsDrawioPageSizedLabelBackground(rect) {
  return [
    katanaDrawioColorValue(rect, "fill") === "#ffffff",
    rect.getAttribute("stroke") === "none",
    katanaDrawioNodeWidth(rect) > katanaDrawioSvgBox(rect.ownerSVGElement).width * 0.5,
  ].every(Boolean);
}

function katanaIsDrawioTextLabelBackground(rect) {
  return [
    katanaDrawioColorValue(rect, "fill") === "#ffffff",
    rect.getAttribute("stroke") === "none",
    katanaDrawioNodeWidth(rect) > 16,
    katanaHasDrawioTextSibling(rect),
  ].every(Boolean);
}

function katanaHasDrawioTextSibling(rect) {
  return Array.from(rect.parentNode?.querySelectorAll("text") ?? []).length > 0;
}

function katanaDrawioColorValue(node, name) {
  return String(node.getAttribute(name) ?? "").toLowerCase();
}

function katanaCropDrawioSvgToContent(svg) {
  katanaApplyDrawioCrop(svg, katanaDrawioCropBox(svg));
}

function katanaDrawioCropBox(svg) {
  return katanaDrawioAlignedCropBox(
    KATANA_DRAWIO_CROP_BOX_READERS[Number(katanaDrawioShouldMeasureRenderedContent(svg))](svg),
  );
}

const KATANA_DRAWIO_CROP_BOX_READERS = [
  (svg) => katanaDrawioPreferredContentBox(svg),
  (svg) => katanaDrawioContentBox(svg),
];

function katanaDrawioPreferredContentBox(svg) {
  if (katanaDrawioUsesDevicePageContentCrop()) {
    return katanaDrawioContentBox(svg);
  }
  const sourceBox = katanaDrawioSourceContentBox(svg);
  return katanaDrawioCanUseSourceContentBox(sourceBox) ? sourceBox : katanaDrawioContentBox(svg);
}

function katanaDrawioCanUseSourceContentBox(box) {
  return [
    box,
    Number.isFinite(box?.x),
    Number.isFinite(box?.y),
    box?.width > 0,
    box?.height > 0,
  ].every(Boolean);
}

function katanaDrawioShouldMeasureRenderedContent(svg) {
  return [
    katanaDrawioRequestSource().includes("mxgraph.aws3d."),
    katanaDrawioNeedsMeasuredContentBox(svg),
  ].some(Boolean);
}

function katanaDrawioNeedsMeasuredContentBox(svg) {
  if (!katanaDrawioSourceIsDeviceTemplate()) {
    return false;
  }
  const sourceBox = katanaDrawioSourceContentBox(svg);
  const contentBox = katanaDrawioContentBox(svg);
  if (!sourceBox) {
    return true;
  }
  return [
    sourceBox,
    contentBox,
    sourceBox.width > 0,
    contentBox.width > 0,
    [
      contentBox.width > sourceBox.width * 1.03,
      contentBox.height > sourceBox.height * 1.03,
    ].some(Boolean),
  ].every(Boolean);
}

function katanaDrawioContentBox(svg) {
  return katanaDrawioOptionalContentBox(svg) ?? katanaDrawioEmptyContentBox();
}

function katanaDrawioOptionalContentBox(svg) {
  const boxes = katanaDrawioContentElements(svg).map(katanaDrawioElementBox);
  return katanaDrawioUnionBox(boxes.filter(katanaDrawioHasArea));
}

function katanaDrawioContentElements(svg) {
  return katanaDrawioContentTagNames()
    .flatMap((tagName) => Array.from(svg.querySelectorAll(tagName)))
    .filter((element) => !katanaShouldIgnoreDrawioContentElement(element));
}

function katanaDrawioContentTagNames() {
  return ["rect", "path", "ellipse", "circle", "line", "polygon", "polyline", "image", "text"];
}

function katanaShouldIgnoreDrawioContentElement(element) {
  return katanaIsWrappedDrawioHtmlFallbackText(element);
}

function katanaIsWrappedDrawioHtmlFallbackText(element) {
  const style = katanaDrawioSourceStyleForElement(element);
  return [
    element.localName === "text",
    style.has("html"),
    style.get("whiteSpace") === "wrap",
    Boolean(katanaDrawioContentCellGroup(element)?.querySelector("foreignObject")),
  ].every(Boolean);
}

function katanaDrawioContentCellGroup(element) {
  return katanaDrawioElementAncestors(element)
    .filter((node) => node.getAttribute?.("data-cell-id"))
    .concat([null])[0];
}

function katanaDrawioElementBox(element) {
  return katanaDrawioTranslatedBox(element.getBBox(), katanaDrawioTranslate(element));
}

function katanaDrawioTranslatedBox(box, translate) {
  return {
    x: box.x + translate.x,
    y: box.y + translate.y,
    width: box.width,
    height: box.height,
  };
}

function katanaDrawioTranslate(element) {
  return katanaDrawioParentTranslate(element.parentNode);
}

function katanaDrawioParentTranslate(node) {
  return node
    ? katanaDrawioAddTranslate(katanaDrawioParentTranslate(node.parentNode), node)
    : { x: 0, y: 0 };
}

function katanaDrawioAddTranslate(translate, node) {
  const next = katanaDrawioNodeTranslate(node);
  return { x: translate.x + next.x, y: translate.y + next.y };
}

function katanaDrawioNodeTranslate(node) {
  return katanaDrawioTranslateMatch(String(node?.getAttribute?.("transform") ?? ""));
}

function katanaDrawioTranslateMatch(transform) {
  const match = transform.match(/translate\(([-\d.]+)(?:[,\s]+([-\d.]+))?\)/);
  return [match]
    .filter(Boolean)
    .map(katanaDrawioTranslateFromMatch)
    .concat([{ x: 0, y: 0 }])[0];
}

function katanaDrawioTranslateFromMatch(match) {
  return { x: Number(match[1]), y: Number(match[2] ?? 0) };
}

function katanaDrawioUnionBox(boxes) {
  return boxes.length === 0 ? null : katanaDrawioUnionNonEmptyBox(boxes);
}

function katanaDrawioUnionNonEmptyBox(boxes) {
  const left = Math.min(...boxes.map((box) => box.x));
  const top = Math.min(...boxes.map((box) => box.y));
  const right = Math.max(...boxes.map(katanaDrawioBoxRight));
  const bottom = Math.max(...boxes.map(katanaDrawioBoxBottom));
  return {
    x: Math.floor(left),
    y: Math.floor(top),
    width: Math.ceil(right - left),
    height: Math.ceil(bottom - top),
  };
}

function katanaDrawioBoxRight(box) {
  return box.x + box.width;
}

function katanaDrawioBoxBottom(box) {
  return box.y + box.height;
}

function katanaDrawioHasArea(box) {
  return [box.width > 0, box.height > 0].every(Boolean);
}

function katanaDrawioEmptyContentBox() {
  return { x: 0, y: 0, width: 1, height: 1 };
}

function katanaDrawioAlignedCropBox(box) {
  if (katanaDrawioHasPreservedTopPadding(box)) {
    return { x: box.x, y: 0, width: box.width, height: box.height + box.y };
  }
  return katanaDrawioHasOnePixelTopLeftCrop(box)
    ? { x: 0, y: 0, width: box.width + 1, height: box.height + 1 }
    : box;
}

function katanaDrawioHasPreservedTopPadding(box) {
  return [box.x === 0, box.y > 0, box.y <= KATANA_DRAWIO_SOURCE_TOP_PADDING_LIMIT].every(Boolean);
}

function katanaDrawioHasOnePixelTopLeftCrop(box) {
  return [box.x === 1, box.y === 1].every(Boolean);
}

function katanaApplyDrawioCrop(svg, box) {
  const paddedBox = katanaDrawioExportPaddedBox(box);
  katanaTranslateDrawioContent(svg, box);
  svg.setAttribute("viewBox", `0 0 ${paddedBox.width} ${paddedBox.height}`);
  svg.setAttribute("width", `${paddedBox.width}px`);
  svg.setAttribute("height", `${paddedBox.height}px`);
}

function katanaDrawioExportPaddedBox(box) {
  return {
    width: box.width + katanaDrawioShadowExportRightPadding() + 1,
    height:
      box.height +
      katanaDrawioExportBottomPadding() +
      katanaDrawioShadowExportBottomPadding() +
      1,
  };
}

function katanaDrawioExportBottomPadding() {
  return katanaDrawioHasExternalImageSource()
    ? KATANA_DRAWIO_EXTERNAL_IMAGE_EXPORT_BOTTOM_PADDING
    : 0;
}

function katanaDrawioShadowExportRightPadding() {
  return katanaDrawioShouldApplyShadowExportPadding()
    ? KATANA_DRAWIO_SHADOW_EXPORT_RIGHT_PADDING
    : 0;
}

function katanaDrawioShadowExportBottomPadding() {
  return katanaDrawioShouldApplyShadowExportPadding()
    ? KATANA_DRAWIO_SHADOW_EXPORT_BOTTOM_PADDING
    : 0;
}

function katanaDrawioShouldApplyShadowExportPadding() {
  return [
    katanaDrawioHasSourceShadowStyle(),
    !katanaDrawioUsesDevicePageContentCrop(),
  ].every(Boolean);
}

function katanaDrawioUsesDevicePageContentCrop() {
  return [
    katanaDrawioSourceHasPageBounds(),
    katanaDrawioSourceIsDeviceTemplate(),
    katanaDrawioSourceHasTransparentPageBackground(),
    katanaDrawioSourceModelCount() === 1,
  ].every(Boolean);
}

function katanaDrawioHasExternalImageSource() {
  return /(?:^|;)image=https?:\/\//.test(katanaDrawioRequestSource());
}

function katanaTranslateDrawioContent(svg, box) {
  const wrapper = document.createElementNS("http://www.w3.org/2000/svg", "g");
  wrapper.setAttribute("transform", `translate(${-box.x},${-box.y})`);
  Array.from(svg.childNodes).forEach((child) => {
    wrapper.appendChild(child);
  });
  svg.appendChild(wrapper);
}

function katanaDrawioNodeWidth(node) {
  return katanaDrawioCssPixels(node.getAttribute("width"));
}
