const KATANA_TEXT_FONT_WEIGHTS = {
  thin: 100,
  normal: 400,
  bold: 700,
  heavy: 900,
  minimum: 1,
  maximum: 1000,
};
const KATANA_RELATIVE_FONT_WEIGHT_BOUNDARIES = { normal: 350, bold: 550, heavy: 750 };

const KATANA_DEFAULT_TEXT_FONT = {
  family: "trebuchet ms, verdana, arial, sans-serif",
  size: 16,
  weight: KATANA_TEXT_FONT_WEIGHTS.normal,
  italic: false,
  anchor: "start",
  letterSpacing: "normal",
  fontFeatureSettings: "normal",
  fontVariationSettings: "normal",
  nativeAdvanceEligible: true,
};

function katanaTextFontContext(node) {
  const css = katanaCssText(node);
  const ancestors = [];
  for (let parent = node.parentNode; parent; parent = parent.parentNode) {
    ancestors.push(parent);
  }
  const inherited = ancestors
    .reverse()
    .reduce((font, parent) => katanaResolveTextFont(parent, font, css), KATANA_DEFAULT_TEXT_FONT);
  return { css, inherited };
}

function katanaOwnTextFontValue(node, name, css) {
  return String(
    katanaCascadedStyleValue(node, name, css) || node.getAttribute?.(name) || "",
  ).trim();
}

function katanaInheritedTextValue(value, inherited, initial) {
  const keyword = value.toLowerCase();
  if (!value || ["inherit", "unset"].includes(keyword)) return inherited;
  return keyword === "initial" ? initial : value;
}

function katanaResolveTextFont(node, inherited, css) {
  if (node.nodeType === 3) return inherited;
  const value = (name) => katanaOwnTextFontValue(node, name, css);
  const style = katanaInheritedTextValue(
    value("font-style"),
    inherited.italic ? "italic" : "normal",
    "normal",
  );
  return {
    family: katanaInheritedTextValue(
      katanaResolvedTextFamilyValue(node, value("font-family"), css),
      inherited.family,
      KATANA_DEFAULT_TEXT_FONT.family,
    ),
    size: katanaResolvedTextFontSize(value("font-size"), inherited.size),
    weight: katanaResolvedTextFontWeight(value("font-weight"), inherited.weight),
    italic: /^(italic|oblique)(\s|$)/i.test(style),
    anchor: katanaInheritedTextValue(value("text-anchor"), inherited.anchor, "start"),
    ...katanaTextAdvanceProperties(node, inherited, css),
  };
}

function katanaTextAdvanceProperties(node, inherited, css) {
  if (!katanaMermaidNativeAdvanceRuntime()) {
    return {
      letterSpacing: "normal",
      fontFeatureSettings: "normal",
      fontVariationSettings: "normal",
      nativeAdvanceEligible: false,
    };
  }
  const value = (name, key) =>
    katanaInheritedTextValue(katanaOwnTextFontValue(node, name, css), inherited[key], "normal");
  const letterSpacing = value("letter-spacing", "letterSpacing");
  const fontFeatureSettings = value("font-feature-settings", "fontFeatureSettings");
  const fontVariationSettings = value("font-variation-settings", "fontVariationSettings");
  return {
    letterSpacing,
    fontFeatureSettings,
    fontVariationSettings,
    nativeAdvanceEligible: katanaNativeAdvancePropertiesAreDefault(
      letterSpacing,
      fontFeatureSettings,
      fontVariationSettings,
    ),
  };
}

function katanaNativeAdvancePropertiesAreDefault(
  letterSpacing,
  fontFeatureSettings,
  fontVariationSettings,
) {
  return (
    /^(normal|0(?:px)?)$/i.test(letterSpacing.trim()) &&
    /^(normal|)$/i.test(fontFeatureSettings.trim()) &&
    /^(normal|)$/i.test(fontVariationSettings.trim())
  );
}

function katanaResolvedTextFontSize(value, inherited) {
  if (value.toLowerCase() === "initial") return KATANA_DEFAULT_TEXT_FONT.size;
  const match = value.match(/^(\d+(?:\.\d+)?)(em|px)?$/i);
  if (!match) return inherited;
  const size = Number(match[1]) * (match[2]?.toLowerCase() === "em" ? inherited : 1);
  return Number.isFinite(size) && size >= 0 ? size : inherited;
}

function katanaResolvedTextFontWeight(value, inherited) {
  const normalized = value.toLowerCase();
  if (!normalized || ["inherit", "unset"].includes(normalized)) return inherited;
  if (["normal", "initial"].includes(normalized)) return KATANA_TEXT_FONT_WEIGHTS.normal;
  if (normalized === "bold") return KATANA_TEXT_FONT_WEIGHTS.bold;
  if (normalized === "bolder") return katanaRelativeBolderWeight(inherited);
  if (normalized === "lighter") return katanaRelativeLighterWeight(inherited);
  const numeric = Number(normalized);
  return Number.isFinite(numeric) &&
    numeric >= KATANA_TEXT_FONT_WEIGHTS.minimum &&
    numeric <= KATANA_TEXT_FONT_WEIGHTS.maximum
    ? numeric
    : inherited;
}

/* WHY: CSS Fonts の相対weight表を親の値へ適用し、子の指定で親runのweightを変えない。 */
function katanaRelativeBolderWeight(inherited) {
  if (inherited < KATANA_RELATIVE_FONT_WEIGHT_BOUNDARIES.normal)
    return KATANA_TEXT_FONT_WEIGHTS.normal;
  if (inherited < KATANA_RELATIVE_FONT_WEIGHT_BOUNDARIES.bold) return KATANA_TEXT_FONT_WEIGHTS.bold;
  return inherited < KATANA_TEXT_FONT_WEIGHTS.heavy ? KATANA_TEXT_FONT_WEIGHTS.heavy : inherited;
}

function katanaRelativeLighterWeight(inherited) {
  if (inherited < KATANA_TEXT_FONT_WEIGHTS.thin) return inherited;
  if (inherited < KATANA_RELATIVE_FONT_WEIGHT_BOUNDARIES.bold) return KATANA_TEXT_FONT_WEIGHTS.thin;
  return inherited < KATANA_RELATIVE_FONT_WEIGHT_BOUNDARIES.heavy
    ? KATANA_TEXT_FONT_WEIGHTS.normal
    : KATANA_TEXT_FONT_WEIGHTS.bold;
}

function katanaResolvedNodeTextFont(node) {
  const context = katanaTextFontContext(node);
  return katanaResolveTextFont(node, context.inherited, context.css);
}
