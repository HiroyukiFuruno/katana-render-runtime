const KATANA_DRAWIO_STENCIL_PLACEHOLDER_STYLE_NAMES = new Map([
  ["fillcolor", "fillColor"],
  ["strokecolor", "strokeColor"],
  ["fillcolor2", "fillColor2"],
  ["strokecolor2", "strokeColor2"],
  ["fillcolor3", "fillColor3"],
  ["strokecolor3", "strokeColor3"],
]);

const KATANA_DRAWIO_CISCO_PLACEHOLDER_COLORS_BY_THEME = [
  new Map([
    ["fillcolor", "#10739e"],
    ["strokecolor", "#ffffff"],
    ["fillcolor2", "#000000"],
    ["strokecolor2", "#ffffff"],
  ]),
  new Map([
    ["fillcolor", "#54a9ce"],
    ["strokecolor", "#121212"],
    ["fillcolor2", "#ededed"],
    ["strokecolor2", "#121212"],
  ]),
];

const KATANA_DRAWIO_STENCIL_DEFAULT_COLOR_CACHE = new Map();

function katanaDrawioStencilPlaceholderColor(element, name, value) {
  const token = katanaDrawioStencilPlaceholderToken(value);
  if (!token) {
    return "";
  }
  return (
    katanaDrawioSourceStylePlaceholderColor(element, name, token) ||
    katanaDrawioStencilDefaultPlaceholderColor(element, name, token) ||
    katanaDrawioFallbackStencilPlaceholderColor(element, token)
  );
}

function katanaDrawioStencilPlaceholderToken(value) {
  const token = katanaDrawioColorKey(value);
  return KATANA_DRAWIO_STENCIL_PLACEHOLDER_STYLE_NAMES.has(token) ? token : "";
}

function katanaDrawioSourceStylePlaceholderColor(element, name, token) {
  return [katanaDrawioElementCellStyleValue(element, katanaDrawioPlaceholderStyleName(token))]
    .filter(Boolean)
    .map((color) => katanaDrawioResolvedSourcePlaceholderColor(element, name, token, color))
    .concat([""])[0];
}

function katanaDrawioPlaceholderStyleName(token) {
  return KATANA_DRAWIO_STENCIL_PLACEHOLDER_STYLE_NAMES.get(token) ?? "";
}

function katanaDrawioResolvedSourcePlaceholderColor(element, name, token, color) {
  return (
    katanaDrawioCiscoSourceSecondaryPlaceholderColor(element, token, color) ||
    katanaDrawioResolvedColor(element, name, color)
  );
}

function katanaDrawioStencilDefaultPlaceholderColor(element, name, token) {
  const color = katanaDrawioStencilDefaultColor(element, token);
  return color ? katanaDrawioResolvedColor(element, name, color) : "";
}

function katanaDrawioStencilDefaultColor(element, token) {
  const shape = katanaDrawioElementCellShape(element);
  const key = `${shape}|${token}`;
  if (KATANA_DRAWIO_STENCIL_DEFAULT_COLOR_CACHE.has(key)) {
    return KATANA_DRAWIO_STENCIL_DEFAULT_COLOR_CACHE.get(key) ?? "";
  }
  const color = katanaDrawioStencilResourceContents()
    .map((content) => katanaDrawioStencilDefaultColorFromContent(content, shape, token))
    .find(Boolean);
  KATANA_DRAWIO_STENCIL_DEFAULT_COLOR_CACHE.set(key, color ?? "");
  return color ?? "";
}

function katanaDrawioStencilResourceContents() {
  return (globalThis.__katanaDrawioRequest?.resources ?? [])
    .filter((resource) => resource.mime_type === "text/xml")
    .map(katanaDrawioResourceContent);
}

function katanaDrawioStencilDefaultColorFromContent(content, shape, token) {
  const stencilName = katanaDrawioStencilName(content, shape);
  const stencil = content.match(katanaDrawioStencilPattern(stencilName))?.[0] ?? "";
  const styleName = katanaDrawioPlaceholderStyleName(token);
  return (
    Array.from(stencil.matchAll(/<(?:fillcolor|strokecolor)\b[^>]*>/gi))
      .map((match) => match[0])
      .filter((tag) => katanaDrawioStencilColorTagUsesStyle(tag, styleName))
      .map(katanaDrawioStencilColorTagDefault)
      .find(Boolean) ?? ""
  );
}

function katanaDrawioStencilColorTagUsesStyle(tag, styleName) {
  return katanaDrawioStencilTagAttribute(tag, "color").toLowerCase() === styleName.toLowerCase();
}

function katanaDrawioStencilColorTagDefault(tag) {
  return katanaDrawioStencilTagAttribute(tag, "default");
}

function katanaDrawioStencilTagAttribute(tag, name) {
  const escapedName = String(name).replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return tag.match(new RegExp(`\\b${escapedName}="([^"]*)"`, "i"))?.[1] ?? "";
}

function katanaDrawioCiscoSourceSecondaryPlaceholderColor(element, token, color) {
  return [
    katanaDrawioIsDarkMode(),
    katanaDrawioIsCiscoShapeElement(element),
    token === "fillcolor2",
    ["#000000", "rgb(0, 0, 0)"].includes(katanaDrawioColorKey(color)),
  ].every(Boolean)
    ? "#ededed"
    : "";
}

function katanaDrawioFallbackStencilPlaceholderColor(element, token) {
  if (katanaDrawioIsCiscoShapeElement(element)) {
    return (
      KATANA_DRAWIO_CISCO_PLACEHOLDER_COLORS_BY_THEME[Number(katanaDrawioIsDarkMode())].get(
        token,
      ) ?? ""
    );
  }
  return katanaDrawioIsAndroidDeviceShapeElement(element) && token === "fillcolor3"
    ? "transparent"
    : "";
}

function katanaDrawioIsCiscoShapeElement(element) {
  return katanaDrawioElementCellShape(element).startsWith("mxgraph.cisco.");
}

function katanaDrawioIsAndroidDeviceShapeElement(element) {
  return KATANA_DRAWIO_ANDROID_DEVICE_SHAPES.has(katanaDrawioElementCellShape(element));
}
