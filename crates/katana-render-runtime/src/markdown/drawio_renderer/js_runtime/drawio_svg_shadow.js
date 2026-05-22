function katanaApplyDrawioShadows(svg) {
  Array.from(svg.querySelectorAll("g"))
    .filter(katanaDrawioElementHasCellId)
    .filter(katanaDrawioElementCellHasShadowStyle)
    .map(katanaDrawioShadowTarget)
    .filter(Boolean)
    .forEach(katanaSetDrawioShadowStyle);
}

function katanaDrawioElementHasCellId(element) {
  return element.hasAttribute("data-cell-id");
}

function katanaDrawioShadowTarget(group) {
  return Array.from(group.children).find(katanaDrawioIsShadowShapeGroup) ?? null;
}

function katanaDrawioIsShadowShapeGroup(element) {
  return [element.localName === "g", element.hasAttribute("transform")].every(Boolean);
}

function katanaSetDrawioShadowStyle(element) {
  element.setAttribute(
    "style",
    katanaDrawioStyleWithAddedProperty(
      katanaDrawioShadowBaseStyle(element),
      "filter",
      katanaDrawioShadowFilter(),
    ),
  );
}

function katanaDrawioShadowBaseStyle(element) {
  return String(element.getAttribute("style") ?? "");
}

function katanaDrawioShadowFilter() {
  return katanaDrawioIsDarkMode()
    ? "drop-shadow(rgba(237, 237, 237, 0.25) 2px 3px 2px)"
    : "drop-shadow(rgba(0, 0, 0, 0.25) 2px 3px 2px)";
}

function katanaDrawioStyleWithAddedProperty(style, name, value) {
  const rewritten = katanaDrawioStyleWithProperty(style, name, value);
  return rewritten.includes(`${name}:`)
    ? rewritten
    : [rewritten, `${name}: ${value}`].filter(Boolean).join("; ");
}
