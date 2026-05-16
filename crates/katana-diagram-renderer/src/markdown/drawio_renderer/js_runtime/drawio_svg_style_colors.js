const KATANA_DRAWIO_DARK_TEXT_FALLBACK_COLORS = ["", "#ffffff"];

function katanaDrawioSimpleDiagramDarkColor(element, name, value) {
  return (
    KATANA_DRAWIO_SIMPLE_DIAGRAM_DARK_COLOR.get(
      katanaDrawioSimpleDiagramDarkColorKey(element, name, value),
    ) ?? ""
  );
}

function katanaDrawioSimpleDiagramDarkColorKey(element, name, value) {
  return katanaDrawioShouldUseSimpleDiagramDarkColor(element, name, value)
    ? katanaDrawioContextColorKey(element, name, value)
    : "";
}

function katanaDrawioShouldUseSimpleDiagramDarkColor(element, name, value) {
  return [
    katanaDrawioIsDarkMode(),
    name === "fill",
    value === "#ffffff",
    katanaDrawioIsTooltipsSource(),
    katanaDrawioIsTooltipsDarkFillElement(element),
  ].every(Boolean);
}

function katanaDrawioIsTooltipsSource() {
  return String(globalThis.__katanaDrawioRequest?.source ?? "").includes("tooltip=");
}

function katanaDrawioIsTooltipsDarkFillElement(element) {
  return [element.localName === "ellipse", katanaDrawioIsLargeTooltipsPath(element)].some(Boolean);
}

function katanaDrawioIsLargeTooltipsPath(element) {
  return [element.localName === "path", katanaDrawioElementArea(element) > 1000].every(Boolean);
}

function katanaNormalizeDrawioStyleTextColor(element) {
  if (katanaDrawioShouldPreserveHtmlTextContentColor(element)) {
    return;
  }
  const style = String(element.getAttribute("style"));
  [katanaDrawioStylePropertyValue(style, "color")]
    .filter(Boolean)
    .map(katanaDrawioResolvedTextColor)
    .filter(Boolean)
    .forEach((color) => {
      element.setAttribute("style", katanaDrawioStyleWithProperty(style, "color", color));
    });
}

function katanaDrawioShouldPreserveHtmlTextContentColor(element) {
  const color = katanaDrawioStylePropertyValue(String(element.getAttribute("style")), "color");
  return [
    katanaDrawioShouldPreserveExplicitHtmlTextContentColor(element, color),
    katanaDrawioShouldPreserveExplicitHtmlTextDescendantColor(element, color),
    katanaDrawioShouldPreserveImplicitHtmlTextContentColor(element, color),
  ].some(Boolean);
}

function katanaDrawioShouldPreserveExplicitHtmlTextContentColor(element, color) {
  return [
    katanaDrawioIsDarkMode(),
    element.getAttribute?.("data-katana-drawio-html-text") === "content",
    katanaDrawioSourceStyleForElement(element).has("fontColor"),
    color,
  ].every(Boolean);
}

function katanaDrawioShouldPreserveExplicitHtmlTextDescendantColor(element, color) {
  return [
    katanaDrawioIsDarkMode(),
    element.getAttribute?.("data-katana-drawio-html-text-explicit-color") === "1",
    color,
  ].every(Boolean);
}

function katanaDrawioShouldPreserveImplicitHtmlTextContentColor(element, color) {
  return [
    katanaDrawioIsDarkMode(),
    element.getAttribute?.("data-katana-drawio-html-text") === "content",
    katanaDrawioSourceStyleUsesDarkHtmlTextBackground(element),
    katanaDrawioColorKey(color) === "#ffffff",
  ].every(Boolean);
}

function katanaDrawioSourceStyleUsesDarkHtmlTextBackground(element) {
  const style = katanaDrawioSourceStyleForElement(element);
  return [
    style.has("text"),
    style.get("shape") === "text",
    style.get("fillColor") === "none",
  ].some(Boolean);
}

function katanaDrawioResolvedTextColor(color) {
  const value = katanaDrawioColorKey(color);
  return (
    katanaDrawioLightDarkExactColor(value) ||
    katanaDrawioTextColorMapForTheme().get(value) ||
    katanaDrawioColorMapForTheme().get(value) ||
    katanaDrawioFallbackDarkTextColor(value) ||
    value
  );
}

function katanaDrawioTextColorMapForTheme() {
  return KATANA_DRAWIO_TEXT_COLOR_BY_THEME[Number(katanaDrawioIsDarkMode())];
}

function katanaDrawioStyleWithProperty(style, name, value) {
  return String(style)
    .split(";")
    .map((declaration) => katanaDrawioStylePropertyDeclaration(declaration, name, value))
    .filter(Boolean)
    .join("; ");
}

function katanaDrawioStylePropertyDeclaration(declaration, name, value) {
  const text = declaration.trim();
  return [text]
    .filter(Boolean)
    .map((it) => katanaDrawioStylePropertyReplacement(it, name, value) || it)
    .concat([""])[0];
}

function katanaDrawioStylePropertyReplacement(declaration, name, value) {
  return [declaration]
    .filter((it) => katanaDrawioStyleDeclarationName(it) === name)
    .map(() => `${name}: ${value}`)
    .concat([""])[0];
}

function katanaDrawioStyleDeclarationName(declaration) {
  return declaration.slice(0, declaration.indexOf(":")).trim().toLowerCase();
}

function katanaDrawioFallbackThemeColor(element, name, value) {
  return katanaDrawioShouldUseFallbackDarkColor(element, name)
    ? katanaDrawioFallbackDarkColor(value)
    : "";
}

function katanaDrawioShouldUseFallbackDarkColor(element, name) {
  return [
    katanaDrawioIsDarkMode(),
    ["fill", "stop-color"].includes(name),
    element.localName !== "text",
  ].every(Boolean);
}

function katanaDrawioFallbackDarkTextColor(value) {
  return KATANA_DRAWIO_DARK_TEXT_FALLBACK_COLORS[
    Number(katanaDrawioShouldUseFallbackDarkTextColor(value))
  ];
}

function katanaDrawioShouldUseFallbackDarkTextColor(value) {
  return [katanaDrawioIsDarkMode(), katanaDrawioColorLuminance(value) < 0.45].every(Boolean);
}

function katanaDrawioFallbackDarkColor(value) {
  return [katanaDrawioParsedColor(value)]
    .filter(Boolean)
    .map((color) => katanaDrawioOfficialDarkColor(value) || katanaDrawioDarkThemeColor(color))
    .concat([""])[0];
}

function katanaDrawioOfficialDarkColor(value) {
  return katanaDrawioCanUseAdaptiveSourceColor(value)
    ? katanaDrawioMxUtils().getLightDarkColor(value).dark || ""
    : "";
}

function katanaDrawioDarkThemeColor(color) {
  return katanaDrawioRgbHex(katanaDrawioScaledDarkColor(color, katanaDrawioDarkScale(color)));
}
