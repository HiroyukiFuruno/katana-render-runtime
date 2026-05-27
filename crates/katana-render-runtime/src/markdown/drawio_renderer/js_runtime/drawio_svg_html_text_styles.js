function katanaDrawioHtmlTextOuterStyle(style, box) {
  return [
    "display: flex",
    `align-items: unsafe ${katanaDrawioHtmlTextAlignItems(style)}`,
    `justify-content: unsafe ${katanaDrawioHtmlTextJustifyContent(style)}`,
    `width: ${katanaDrawioHtmlTextWidth(style, box)}px`,
    `height: ${katanaDrawioHtmlTextHeight(style, box)}px`,
    `padding-top: ${katanaDrawioHtmlTextTop(style, box)}px`,
    `margin-left: ${katanaDrawioHtmlTextLeft(style, box)}px`,
  ].join("; ");
}

function katanaDrawioHtmlTextBoxStyle(style, box) {
  return [
    "box-sizing: border-box",
    "font-size: 0",
    `text-align: ${katanaDrawioTextAlign(style)}`,
    ...katanaDrawioHtmlTextBoxSizeStyle(style, box),
    `color: ${katanaDrawioTextColor(style)}`,
    "",
  ].join("; ");
}

function katanaDrawioHtmlTextContentStyle(style) {
  return [
    "display: inline-block",
    `font-size: ${katanaDrawioFontSize(style)}px`,
    `font-family: ${katanaDrawioHtmlFontFamily(style)}`,
    `color: ${katanaDrawioHtmlTextColor(style)}`,
    "line-height: 1.2",
    "pointer-events: all",
    ...katanaDrawioHtmlTextContentSizeStyle(style),
    `white-space: ${katanaDrawioHtmlTextWhiteSpace(style)}`,
    "word-wrap: normal",
    "",
  ].join("; ");
}

function katanaDrawioHtmlTextAlignItems(style) {
  return (
    KATANA_DRAWIO_HTML_TEXT_ALIGN_ITEMS.get(katanaDrawioHtmlTextVerticalAlign(style)) ??
    "center"
  );
}

function katanaDrawioHtmlTextVerticalAlign(style) {
  return katanaDrawioStyleValueFromMap(style, "verticalAlign", katanaDrawioDefaultVerticalAlign(style));
}

function katanaDrawioHtmlTextJustifyContent(style) {
  return (
    KATANA_DRAWIO_HTML_TEXT_JUSTIFY_CONTENT.get(
      katanaDrawioStyleValueFromMap(style, "align", katanaDrawioDefaultAlign(style)),
    ) ?? "center"
  );
}

function katanaDrawioTextAlign(style) {
  return katanaDrawioStyleValueFromMap(style, "align", katanaDrawioDefaultAlign(style));
}

function katanaDrawioDefaultAlign(style) {
  return katanaDrawioIsTextShapeStyle(style) ? "left" : "center";
}

function katanaDrawioDefaultVerticalAlign(style) {
  if (katanaDrawioUsesDefaultBottomImageHtmlTextLabel(style)) {
    return "top";
  }
  return katanaDrawioIsTextShapeStyle(style) ? "top" : "middle";
}

function katanaDrawioIsTextShapeStyle(style) {
  return [
    katanaDrawioStyleMap(style).has("text"),
    katanaDrawioStyleValue(style, "shape") === "text",
  ].some(Boolean);
}

function katanaDrawioHtmlFontFamily(style) {
  const family = katanaDrawioStyleValueFromMap(style, "fontFamily", "Helvetica");
  return family.includes(" ") ? `'${family}'` : family;
}

function katanaDrawioHtmlTextTop(style, box) {
  if (katanaDrawioIsFillHtmlTextLabel(style)) {
    return Math.round(box.y);
  }
  if (katanaDrawioIsBottomHtmlTextLabel(style)) {
    return Math.round(
      box.y + box.height + 7 + katanaDrawioHtmlTextSpacingTop(style),
    );
  }
  if (katanaDrawioUsesExplicitLeftTopHtmlTextLabel(style)) {
    return Math.round(box.y - katanaDrawioHtmlTextSpacingBottom(style) - 3);
  }
  if (katanaDrawioUsesTextShapeWrapInsets(style)) {
    return Math.round(box.y + 2);
  }
  if (katanaDrawioUsesExplicitHtmlTextSpacing(style) && katanaDrawioHtmlTextAlignTop(style)) {
    return Math.round(
      box.y +
        katanaDrawioHtmlTextSpacing(style) +
        katanaDrawioHtmlTextSpacingTop(style) +
        5 +
        katanaDrawioHtmlTextOfficialTopOffset(),
    );
  }
  const alignment = katanaDrawioHtmlTextVerticalAlign(style);
  const textTop = (KATANA_DRAWIO_HTML_TEXT_TOP_BY_ALIGNMENT.get(alignment) ??
    katanaDrawioMiddleHtmlTextTop)(box);
  return Math.round(textTop + katanaDrawioMiddleHtmlTextSpacingTop(style, alignment));
}

function katanaDrawioTopHtmlTextTop(box) {
  return box.y + KATANA_DRAWIO_HTML_TEXT_TOP_PADDING;
}

function katanaDrawioMiddleHtmlTextTop(box) {
  return box.y + box.height / 2;
}

function katanaDrawioMiddleHtmlTextSpacingTop(style, alignment) {
  return alignment === "middle" ? katanaDrawioHtmlTextSpacingTop(style) / 2 : 0;
}

function katanaDrawioBottomHtmlTextTop(box) {
  return box.y + box.height - KATANA_DRAWIO_HTML_TEXT_BOTTOM_PADDING;
}

function katanaDrawioHtmlTextOfficialTopOffset() {
  return katanaDrawioHasDarkCubePageTop() ? 3 : 0;
}

function katanaDrawioHtmlTextWidth(style, box) {
  if (katanaDrawioIsFillHtmlTextLabel(style)) {
    return Math.max(1, Math.round(box.width + 2));
  }
  if (katanaDrawioIsBottomHtmlTextLabel(style)) {
    return 1;
  }
  if (katanaDrawioUsesExplicitLeftTopHtmlTextLabel(style)) {
    return Math.max(1, Math.round(box.width + 2));
  }
  if (katanaDrawioUsesTextShapeWrapInsets(style)) {
    return Math.max(1, Math.round(box.width - 2));
  }
  if (katanaDrawioUsesExplicitHtmlTextSpacing(style)) {
    return Math.max(1, Math.round(box.width - katanaDrawioHtmlTextHorizontalSpacing(style) + 2));
  }
  return katanaDrawioIsSideHtmlTextLabel(style) ? 1 : Math.max(1, Math.round(box.width - 2));
}

function katanaDrawioHtmlTextHeight(style, box) {
  return katanaDrawioIsFillHtmlTextLabel(style) ? Math.max(1, Math.round(box.height)) : 1;
}

function katanaDrawioHtmlTextLeft(style, box) {
  if (katanaDrawioIsFillHtmlTextLabel(style)) {
    return Math.round(box.x - katanaDrawioAppliedLeftPadding());
  }
  if (katanaDrawioIsLeftHtmlTextLabel(style)) {
    if (katanaDrawioUsesExplicitLeftTopHtmlTextLabel(style)) {
      return Math.round(
        box.x -
          katanaDrawioHtmlTextWidth(style, box) -
          katanaDrawioHtmlTextSpacingRight(style) -
          katanaDrawioAppliedLeftPadding(),
      );
    }
    return Math.round(
      box.x - 2 - katanaDrawioHtmlTextSpacingRight(style) - katanaDrawioAppliedLeftPadding(),
    );
  }
  if (katanaDrawioUsesTextShapeWrapInsets(style)) {
    return Math.round(box.x + 2 - katanaDrawioAppliedLeftPadding());
  }
  if (katanaDrawioUsesExplicitHtmlTextSpacing(style)) {
    return Math.round(
      box.x +
        katanaDrawioHtmlTextSpacing(style) +
        katanaDrawioHtmlTextSpacingLeft(style) -
        katanaDrawioRightAlignedHtmlTextOffset(style) -
        katanaDrawioAppliedLeftPadding(),
    );
  }
  if (katanaDrawioIsBottomHtmlTextLabel(style)) {
    return Math.round(
      box.x + box.width / 2 - katanaDrawioAppliedLeftPadding(),
    );
  }
  return Math.round(
    (
      katanaDrawioIsRightHtmlTextLabel(style)
        ? box.x +
          box.width +
          2 +
          katanaDrawioHtmlTextSpacingLeft(style)
        : box.x + KATANA_DRAWIO_HTML_TEXT_LEFT_PADDING
    ) - katanaDrawioAppliedLeftPadding(),
  );
}

function katanaDrawioHtmlTextBoxSizeStyle(style, box) {
  if (katanaDrawioIsFillHtmlTextLabel(style)) {
    return [
      `width: ${Math.max(1, Math.round(box.width))}px`,
      `height: ${Math.max(1, Math.round(box.height))}px`,
      "overflow: hidden",
    ];
  }
  if (katanaDrawioStyleValue(style, "overflow") === "hidden") {
    return [
      `max-height: ${Math.max(1, Math.round(box.height))}px`,
      "overflow: hidden",
    ];
  }
  return [];
}

function katanaDrawioHtmlTextWhiteSpace(style) {
  return [
    katanaDrawioIsFillHtmlTextLabel(style),
    katanaDrawioIsBottomHtmlTextLabel(style),
  ].some(Boolean)
    ? "nowrap"
    : "normal";
}

function katanaDrawioHtmlTextContentSizeStyle(style) {
  return katanaDrawioIsFillHtmlTextLabel(style) ? ["width: 100%", "height: 100%"] : [];
}

function katanaDrawioIsFillHtmlTextLabel(style) {
  return katanaDrawioStyleValue(style, "overflow") === "fill";
}

function katanaDrawioIsRightHtmlTextLabel(style) {
  return katanaDrawioStyleValue(style, "labelPosition") === "right";
}

function katanaDrawioIsLeftHtmlTextLabel(style) {
  return katanaDrawioStyleValue(style, "labelPosition") === "left";
}

function katanaDrawioIsSideHtmlTextLabel(style) {
  return [katanaDrawioIsLeftHtmlTextLabel(style), katanaDrawioIsRightHtmlTextLabel(style)].some(Boolean);
}

function katanaDrawioIsBottomHtmlTextLabel(style) {
  return [
    [
      katanaDrawioStyleValue(style, "verticalLabelPosition") === "bottom",
      katanaDrawioUsesDefaultBottomImageHtmlTextLabel(style),
    ].some(Boolean),
    !katanaDrawioIsSideHtmlTextLabel(style),
  ].every(Boolean);
}

function katanaDrawioUsesDefaultBottomImageHtmlTextLabel(style) {
  return [
    katanaDrawioIsImageStyle(style),
    katanaDrawioStyleValue(style, "verticalLabelPosition") === "",
  ].every(Boolean);
}

function katanaDrawioIsImageStyle(style) {
  const styleMap = katanaDrawioStyleMap(style);
  return [
    styleMap.has("image"),
    styleMap.get("shape") === "image",
  ].some(Boolean);
}

function katanaDrawioUsesExplicitLeftTopHtmlTextLabel(style) {
  return [
    katanaDrawioIsLeftHtmlTextLabel(style),
    katanaDrawioUsesExplicitHtmlTextSpacing(style),
    katanaDrawioStyleValue(style, "verticalLabelPosition") === "top",
  ].every(Boolean);
}

function katanaDrawioHtmlTextColor(style) {
  const color = katanaDrawioTextColor(style);
  return (
    katanaDrawioHtmlTextDarkColor(style, color) ??
    katanaDrawioHtmlTextAdaptiveColor(color) ??
    katanaDrawioResolvedTextColor(color)
  );
}

function katanaDrawioHtmlTextDarkColor(style, color) {
  const value = katanaDrawioColorKey(color);
  return katanaDrawioKeepsLightHtmlTextOnDarkBackground(style, value)
    ? "#ffffff"
    : KATANA_DRAWIO_HTML_TEXT_DARK_COLOR.get(value);
}

function katanaDrawioKeepsLightHtmlTextOnDarkBackground(style, color) {
  return [
    katanaDrawioIsDarkMode(),
    !katanaDrawioHasExplicitHtmlTextFontColor(style),
    katanaDrawioHtmlTextUsesDarkBackground(style),
    KATANA_DRAWIO_HTML_TEXT_LIGHT_ON_DARK_BACKGROUND.has(color),
  ].every(Boolean);
}

function katanaDrawioHasExplicitHtmlTextFontColor(style) {
  const color = katanaDrawioColorKey(katanaDrawioStyleValue(style, "fontColor"));
  return !["", "default"].includes(color);
}

function katanaDrawioHtmlTextUsesDarkBackground(style) {
  return [katanaDrawioHtmlTextUsesTransparentFill(style), katanaDrawioIsTextShapeStyle(style)].some(Boolean);
}

function katanaDrawioHtmlTextUsesTransparentFill(style) {
  return ["", "none"].includes(katanaDrawioColorKey(katanaDrawioStyleValue(style, "fillColor")));
}

function katanaDrawioHtmlTextAdaptiveColor(color) {
  const adaptiveColor = [
    katanaDrawioIsDarkMode(),
    katanaDrawioCanUseAdaptiveSourceColor(color),
  ].every(Boolean)
    ? katanaDrawioAdaptiveDarkColor(color)
    : "";
  return (
    katanaDrawioColorMapForTheme().get(katanaDrawioColorKey(color)) ||
    katanaDrawioColorMapForTheme().get(katanaDrawioColorKey(adaptiveColor)) ||
    adaptiveColor
  ) || undefined;
}

function katanaDrawioCanUseAdaptiveSourceColor(color) {
  return [
    katanaDrawioParsedColor(katanaDrawioColorKey(color)),
    typeof katanaDrawioMxUtils()?.getLightDarkColor === "function",
  ].every(Boolean);
}

function katanaDrawioAdaptiveDarkColor(color) {
  const value = katanaDrawioColorKey(color);
  return katanaDrawioOfficialDarkColor(value) || katanaDrawioFallbackDarkColor(value);
}

const KATANA_DRAWIO_HTML_TEXT_TOP_PADDING = 8;
const KATANA_DRAWIO_HTML_TEXT_BOTTOM_PADDING = 3;
const KATANA_DRAWIO_HTML_TEXT_LEFT_PADDING = 1;

const KATANA_DRAWIO_HTML_TEXT_TOP_BY_ALIGNMENT = new Map([
  ["top", katanaDrawioTopHtmlTextTop],
  ["middle", katanaDrawioMiddleHtmlTextTop],
  ["bottom", katanaDrawioBottomHtmlTextTop],
]);

const KATANA_DRAWIO_HTML_TEXT_ALIGN_ITEMS = new Map([
  ["top", "flex-start"],
  ["middle", "center"],
  ["bottom", "flex-end"],
]);

const KATANA_DRAWIO_HTML_TEXT_JUSTIFY_CONTENT = new Map([
  ["left", "flex-start"],
  ["center", "center"],
  ["right", "flex-end"],
]);

const KATANA_DRAWIO_HTML_TEXT_DARK_COLOR = new Map([
  ["#000000", "light-dark(#000000, #ffffff)"],
  ["default", "light-dark(#000000, #ffffff)"],
]);
const KATANA_DRAWIO_HTML_TEXT_LIGHT_ON_DARK_BACKGROUND = new Set([
  "#000000",
  "#ffffff",
  "default",
]);
