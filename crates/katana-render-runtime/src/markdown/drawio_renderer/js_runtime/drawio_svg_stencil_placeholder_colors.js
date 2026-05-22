const KATANA_DRAWIO_STENCIL_PLACEHOLDER_STYLE_NAMES = new Map([
  ["fillcolor", "fillColor"],
  ["strokecolor", "strokeColor"],
  ["fillcolor2", "fillColor2"],
  ["strokecolor2", "strokeColor2"],
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

function katanaDrawioStencilPlaceholderColor(element, name, value) {
  const token = katanaDrawioStencilPlaceholderToken(value);
  if (!token) {
    return "";
  }
  return (
    katanaDrawioSourceStylePlaceholderColor(element, name, token) ||
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
  return katanaDrawioIsCiscoShapeElement(element)
    ? KATANA_DRAWIO_CISCO_PLACEHOLDER_COLORS_BY_THEME[Number(katanaDrawioIsDarkMode())].get(token)
    : "";
}

function katanaDrawioIsCiscoShapeElement(element) {
  return katanaDrawioElementCellShape(element).startsWith("mxgraph.cisco.");
}
