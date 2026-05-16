function katanaNormalizeDrawioGeometry(svg) {
  if (katanaDrawioNeedsNearIntegerGeometryRounding()) {
    katanaRoundDrawioNearIntegerPaths(svg);
  }
  if (!katanaDrawioRequestSource().includes("mxgraph.aws")) {
    return;
  }
  Array.from(svg.querySelectorAll("g"))
    .filter(katanaDrawioIsCellContentGroup)
    .filter(katanaDrawioNeedsHalfPixelTranslate)
    .forEach(katanaApplyDrawioHalfPixelTranslate);
}

function katanaDrawioIsCellContentGroup(group) {
  return group.parentNode?.getAttribute?.("data-cell-id");
}

function katanaDrawioNeedsHalfPixelTranslate(group) {
  return [!group.getAttribute("transform"), katanaDrawioGroupHasDirectShape(group)].every(Boolean);
}

function katanaDrawioGroupHasDirectShape(group) {
  return Array.from(group.children).some((child) =>
    KATANA_DRAWIO_HALF_PIXEL_SHAPE_TAGS.has(child.localName),
  );
}

function katanaApplyDrawioHalfPixelTranslate(group) {
  group.setAttribute("transform", "translate(0.5,0.5)");
}

function katanaDrawioNeedsNearIntegerGeometryRounding() {
  return katanaDrawioRequestSource().includes("mxgraph.infographic");
}

function katanaRoundDrawioNearIntegerPaths(svg) {
  Array.from(svg.querySelectorAll("path"))
    .filter((path) => path.getAttribute("d"))
    .forEach((path) => {
      path.setAttribute("d", katanaDrawioRoundNearIntegerPath(path.getAttribute("d")));
    });
}

function katanaDrawioRoundNearIntegerPath(data) {
  return String(data).replace(/-?\d+\.\d+/g, katanaDrawioRoundNearIntegerNumber);
}

function katanaDrawioRoundNearIntegerNumber(value) {
  const number = Number(value);
  const rounded = Math.round(number);
  return Math.abs(number - rounded) <= 0.051 ? String(rounded) : value;
}

const KATANA_DRAWIO_HALF_PIXEL_SHAPE_TAGS = new Set([
  "circle",
  "ellipse",
  "line",
  "path",
  "polygon",
  "polyline",
  "rect",
]);
