const KATANA_DRAWIO_LARGE_WHITE_PATH_AREA = 10000;
const KATANA_DRAWIO_ANDROID_DEVICE_SCREEN_PATH_AREA = 10000;
const KATANA_DRAWIO_AWS_SHAPE_PREFIXES = [
  "mxgraph.aws.",
  "mxgraph.aws2.",
  "mxgraph.aws3.",
  "mxgraph.aws4.",
  "mxgraph.aws3d.",
];
const KATANA_DRAWIO_AZURE_SHAPE_PREFIXES = ["mxgraph.azure.", "mxgraph.mscae.", "mxgraph.office."];
const KATANA_DRAWIO_VISUAL_SHAPE_TAGS = new Set([
  "circle",
  "ellipse",
  "line",
  "path",
  "polygon",
  "polyline",
  "rect",
]);
const KATANA_DRAWIO_PAINT_ATTRIBUTES = new Set(["fill", "stroke", "stop-color"]);
const KATANA_DRAWIO_AZURE_DARK_ATTRIBUTES = new Set(["color", "fill", "stroke"]);
const KATANA_DRAWIO_ANDROID_DEVICE_SHAPES = new Set([
  "mxgraph.android.phone2",
  "mxgraph.android.tab2",
]);
const KATANA_DRAWIO_ANDROID_DEVICE_SCREEN_COLORS = new Set([
  "#ededed",
  "#202020",
  "rgb(237, 237, 237)",
  "rgb(32, 32, 32)",
]);

function katanaDrawioForcedAttributeColor(element, name, value) {
  return katanaDrawioAndroidDeviceScreenFillColor(element, name, value);
}

function katanaNormalizeDrawioAndroidDeviceScreens(svg) {
  Array.from(svg.querySelectorAll("path"))
    .filter((path) => katanaDrawioIsAndroidDeviceScreenFill(path, "fill", katanaDrawioColorKey(path.getAttribute("fill"))))
    .forEach((path) => {
      path.setAttribute("fill", "transparent");
    });
}

function katanaDrawioContextOriginalColor(element, name, value) {
  return (
    katanaDrawioAzureDocumentDarkColor(element, name, value) ||
    katanaDrawioAzureDarkColor(element, name, value) ||
    katanaDrawioLegacyAwsDarkColor(element, name, value) ||
    katanaDrawioPidWhiteFillColor(element, name, value) ||
    katanaDrawioSimpleDiagramDarkColor(element, name, value) ||
    katanaDrawioFloorplanBlackPaintColor(element, name, value) ||
    katanaDrawioOpaqueBlackPathColor(element, name, value) ||
    katanaDrawioLargeWhitePathColor(element, name, value)
  );
}

function katanaDrawioFloorplanBlackPaintColor(element, name, value) {
  return [
    katanaDrawioIsDarkMode(),
    ["fill", "stroke"].includes(name),
    value === "#000000",
    katanaDrawioElementCellShape(element).startsWith("mxgraph.floorplan."),
  ].every(Boolean)
    ? "#ebebeb"
    : "";
}

function katanaDrawioPidWhiteFillColor(element, name, value) {
  return katanaDrawioIsPidWhiteFill(element, name, value) ? "#121212" : "";
}

function katanaDrawioIsPidWhiteFill(element, name, value) {
  return [
    katanaDrawioIsDarkMode(),
    name === "fill",
    value === "white",
    ["ellipse", "path"].includes(element.localName),
    katanaDrawioElementHasWhiteSourceFill(element),
    katanaDrawioIsPidWhiteFillSourceElement(element),
  ].every(Boolean);
}

function katanaDrawioElementHasWhiteSourceFill(element) {
  return ["#ffffff", "white"].includes(
    katanaDrawioColorKey(katanaDrawioElementCellStyleValue(element, "fillColor")),
  );
}

function katanaDrawioIsPidWhiteFillSourceElement(element) {
  const shape = katanaDrawioElementCellShape(element);
  const style = katanaDrawioSourceStyleForElement(element);
  return [shape.startsWith("mxgraph.pid."), style.has("ellipse")].some(Boolean);
}

function katanaDrawioAndroidDeviceScreenFillColor(element, name, value) {
  return katanaDrawioIsAndroidDeviceScreenFill(element, name, value) ? "transparent" : "";
}

function katanaDrawioIsAndroidDeviceScreenFill(element, name, value) {
  return [
    katanaDrawioIsDarkMode(),
    element.localName === "path",
    name === "fill",
    KATANA_DRAWIO_ANDROID_DEVICE_SCREEN_COLORS.has(value),
    katanaDrawioHasAndroidDeviceSource(),
    katanaDrawioPathDataArea(element) > KATANA_DRAWIO_ANDROID_DEVICE_SCREEN_PATH_AREA,
  ].every(Boolean);
}

function katanaDrawioHasAndroidDeviceSource() {
  const source = katanaDrawioRequestSource();
  return Array.from(KATANA_DRAWIO_ANDROID_DEVICE_SHAPES).some((shape) => source.includes(shape));
}

function katanaDrawioPathDataArea(element) {
  const coordinates = Array.from(String(element.getAttribute("d") ?? "").matchAll(/-?\d+\.?\d*/g))
    .map((match) => Number(match[0]))
    .filter(Number.isFinite);
  if (coordinates.length < 8) {
    return 0;
  }
  const axisCoordinates = (offset) => coordinates.filter((_coordinate, index) => index % 2 === offset);
  return katanaDrawioCoordinateSpan(axisCoordinates(0)) * katanaDrawioCoordinateSpan(axisCoordinates(1));
}

function katanaDrawioCoordinateSpan(coordinates) {
  return Math.max(...coordinates) - Math.min(...coordinates);
}

function katanaDrawioAzureDarkColor(element, name, value) {
  return (
    KATANA_DRAWIO_AZURE_DARK_COLOR.get(katanaDrawioAzureDarkColorKey(element, name, value)) ?? ""
  );
}

function katanaDrawioAzureDarkColorKey(element, name, value) {
  return [
    katanaDrawioIsDarkMode(),
    KATANA_DRAWIO_AZURE_DARK_ATTRIBUTES.has(name),
    katanaDrawioIsAzureShapeElement(element),
  ].every(Boolean)
    ? `${name}|${value}`
    : "";
}

function katanaDrawioLegacyAwsDarkColor(element, name, value) {
  return (
    KATANA_DRAWIO_AWS_LEGACY_DARK_COLOR.get(
      katanaDrawioLegacyAwsDarkColorKey(element, name, value),
    ) ?? ""
  );
}

function katanaDrawioLegacyAwsDarkColorKey(element, name, value) {
  return katanaDrawioShouldMapLegacyAwsDarkColor(element, name) ? `${name}|${value}` : "";
}

function katanaDrawioShouldMapLegacyAwsDarkColor(element, name) {
  return [
    katanaDrawioIsDarkMode(),
    katanaDrawioIsPaintAttribute(name),
    katanaDrawioIsVisualShapeTag(element),
    katanaDrawioIsLegacyAwsShapeElement(element),
  ].every(Boolean);
}

function katanaDrawioLargeWhitePathColor(element, name, value) {
  if (!katanaDrawioIsLargeWhitePath(element, name, value)) {
    return "";
  }
  return "#121212";
}

function katanaDrawioOpaqueBlackPathColor(element, name, value) {
  if (!katanaDrawioIsOpaqueBlackPath(element, name, value)) {
    return "";
  }
  return "#ffffff";
}

function katanaDrawioIsOpaqueBlackPath(element, name, value) {
  return [
    katanaDrawioIsDarkMode(),
    element.localName === "path",
    katanaDrawioUsesOfficialWhiteBlackPath(element),
    ["fill", "stroke"].includes(name),
    value === "#000000",
    katanaDrawioPaintOpacity(element, name) >= 1,
  ].every(Boolean);
}

function katanaDrawioUsesOfficialWhiteBlackPath(element) {
  return [
    katanaDrawioElementCellIsEdge(element),
    katanaDrawioElementCellHasStyleKey(element, "swimlane"),
  ].some(Boolean);
}

function katanaDrawioPaintOpacity(element, name) {
  const value =
    element.getAttribute(`${name}-opacity`) ||
    katanaDrawioStylePropertyValue(katanaDrawioElementStyleText(element), `${name}-opacity`) ||
    element.getAttribute("opacity") ||
    "1";
  return Number.parseFloat(value);
}

function katanaDrawioIsLargeWhitePath(element, name, value) {
  return [
    katanaDrawioIsDarkMode(),
    element.localName === "path",
    name === "fill",
    value === "#ffffff",
    katanaDrawioElementArea(element) > KATANA_DRAWIO_LARGE_WHITE_PATH_AREA,
  ].every(Boolean);
}

function katanaDrawioElementArea(element) {
  const box = element.getBBox();
  return box.width * box.height;
}

function katanaDrawioIsPaintAttribute(name) {
  return KATANA_DRAWIO_PAINT_ATTRIBUTES.has(name);
}

function katanaDrawioIsVisualShapeTag(element) {
  return KATANA_DRAWIO_VISUAL_SHAPE_TAGS.has(element.localName);
}

function katanaDrawioIsAzureShapeElement(element) {
  return KATANA_DRAWIO_AZURE_SHAPE_PREFIXES.some((it) =>
    katanaDrawioElementCellShape(element).startsWith(it),
  );
}

function katanaDrawioIsLegacyAwsShapeElement(element) {
  return ["mxgraph.aws.", "mxgraph.aws3.", "mxgraph.aws4."].some((it) =>
    katanaDrawioElementCellShape(element).startsWith(it),
  );
}

const KATANA_DRAWIO_AWS_LEGACY_DARK_COLOR = new Map([
  ["fill|#000000", "#ededed"],
  ["fill|#2e73b8", "#5f9bd6"],
  ["fill|#7d7c7c", "#848383"],
  ["fill|#d2d3d3", "#373838"],
  ["fill|#d9a741", "#835801"],
  ["fill|#e05243", "#ef7568"],
  ["fill|#f58534", "#bd5c17"],
  ["fill|#f58536", "#bd5c18"],
  ["fill|#ffffff", "#232f3e"],
  ["stroke|#000000", "#ffffff"],
  ["stroke|#e6e6e6", "#272727"],
]);

const KATANA_DRAWIO_AZURE_DARK_COLOR = new Map([
  ["color|#ffffff", "#121212"],
  ["color|rgb(255, 255, 255)", "rgb(18, 18, 18)"],
  ["fill|#ffffff", "#121212"],
  ["fill|rgb(255, 255, 255)", "rgb(18, 18, 18)"],
  ["stroke|#ffffff", "#121212"],
  ["stroke|rgb(255, 255, 255)", "rgb(18, 18, 18)"],
]);
