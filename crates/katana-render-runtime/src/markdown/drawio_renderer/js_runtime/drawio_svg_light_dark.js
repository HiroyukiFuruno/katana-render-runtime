const KATANA_DRAWIO_LIGHT_DARK_PATTERN =
  /light-dark\((rgb\([^)]+\)|#[\da-f]{6}|black|white|(?:fill|stroke)color\d*),\s*(rgb\([^)]+\)|#[\da-f]{6}|black|white|var\(--ge-dark-color,\s*#[\da-f]{6}\))\)/gi;

const KATANA_DRAWIO_STYLE_SVG_COLOR_ATTRIBUTES = ["fill", "stroke", "stop-color"];
const KATANA_DRAWIO_LIGHT_DARK_ATTRIBUTE_PREFIX = "data-katana-light-dark-";

function katanaResolveDrawioLightDarkStyleColors(element) {
  const style = katanaDrawioElementStyleText(element);
  if (!style?.includes("light-dark(")) {
    return;
  }

  const resolvedStyle = katanaResolvedDrawioLightDarkStyle(element, style);
  element.setAttribute("style", resolvedStyle);
  katanaMarkDrawioLightDarkTextColor(element, style);
  katanaApplyDrawioStyleColorAttributes(element, resolvedStyle);
}

function katanaMarkDrawioLightDarkTextColor(element, style) {
  if (katanaDrawioStylePropertyValue(style, "color").includes("light-dark(")) {
    element.setAttribute(katanaDrawioLightDarkAttributeName("color"), "true");
  }
}

function katanaDrawioElementStyleText(element) {
  return element.getAttribute("style") ?? element.style?.cssText ?? "";
}

function katanaResolvedDrawioLightDarkStyle(element, style) {
  return style.replace(KATANA_DRAWIO_LIGHT_DARK_PATTERN, (_match, light, dark) =>
    katanaDrawioLightDarkStyleChoice(element, light, dark),
  );
}

function katanaDrawioLightDarkStyleChoice(element, light, dark) {
  return katanaDrawioIsDarkMode()
    ? katanaDrawioLightDarkPlaceholderColor(element, light) ||
        katanaDrawioNamedLightDarkStyleColor(element, light, dark) ||
        katanaDrawioLightDarkDarkStyleColor(dark)
    : String(light).trim();
}

function katanaDrawioLightDarkPlaceholderColor(element, light) {
  const token = katanaDrawioStencilPlaceholderToken(light);
  return token
    ? katanaDrawioStencilPlaceholderColor(element, katanaDrawioPlaceholderPaintName(token), token)
    : "";
}

function katanaDrawioPlaceholderPaintName(token) {
  return token.startsWith("stroke") ? "stroke" : "fill";
}

function katanaDrawioNamedLightDarkStyleColor(element, light, dark) {
  const key = `${String(light).trim().toLowerCase()}|${String(dark).trim().toLowerCase()}`;
  if (key === "white|#000000") {
    return katanaDrawioExplicitNamedWhiteSourceColor(element) ? "#121212" : "#000000";
  }
  return KATANA_DRAWIO_NAMED_LIGHT_DARK_COLORS.get(key) ?? "";
}

function katanaDrawioExplicitNamedWhiteSourceColor(element) {
  const style = katanaDrawioSourceStyleForElement(element);
  return [style.get("fillColor"), style.get("strokeColor")]
    .map(katanaDrawioColorKey)
    .includes("white");
}

const KATANA_DRAWIO_NAMED_LIGHT_DARK_COLORS = new Map([["black|#000000", "#ededed"]]);

function katanaDrawioLightDarkDarkStyleColor(dark) {
  const value = String(dark).trim();
  const fallback = value.match(/var\(--ge-dark-color,\s*(#[\da-f]{6})\)/i);
  return fallback?.[1] ?? value;
}

function katanaApplyDrawioStyleColorAttributes(element, style) {
  KATANA_DRAWIO_STYLE_SVG_COLOR_ATTRIBUTES.map((name) =>
    katanaDrawioStyleColorAttribute(style, name),
  )
    .filter(katanaHasDrawioStyleColorAttributeValue)
    .forEach((attribute) => {
      element.setAttribute(attribute.name, attribute.value);
      element.setAttribute(katanaDrawioLightDarkAttributeName(attribute.name), "true");
    });
}

function katanaDrawioLightDarkAttributeName(name) {
  return `${KATANA_DRAWIO_LIGHT_DARK_ATTRIBUTE_PREFIX}${name}`;
}

function katanaDrawioStyleColorAttribute(style, name) {
  return { name, value: katanaDrawioStylePropertyValue(style, name) };
}

function katanaHasDrawioStyleColorAttributeValue(attribute) {
  return attribute.value !== "";
}

function katanaDrawioStylePropertyValue(style, name) {
  return style
    .split(";")
    .map((declaration) => declaration.trim())
    .filter((declaration) => declaration.toLowerCase().startsWith(`${name}:`))
    .map(katanaDrawioStyleDeclarationValue)
    .filter(Boolean)
    .concat([""])[0];
}

function katanaDrawioStyleDeclarationValue(declaration) {
  return declaration.slice(declaration.indexOf(":") + 1).trim();
}
