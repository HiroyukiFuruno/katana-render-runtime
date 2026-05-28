function katanaDrawioHtmlTextAlignTop(style) {
  return katanaDrawioHtmlTextVerticalAlign(style) === "top";
}

function katanaDrawioUsesExplicitHtmlTextSpacing(style) {
  return [
    "spacing",
    "spacingTop",
    "spacingBottom",
    "spacingLeft",
    "spacingRight",
  ].some((name) => katanaDrawioStyleValue(style, name) !== "");
}

function katanaDrawioHtmlTextHorizontalSpacing(style) {
  return (
    katanaDrawioHtmlTextSpacing(style) * 2 +
    katanaDrawioHtmlTextSpacingLeft(style) +
    katanaDrawioHtmlTextSpacingRight(style)
  );
}

function katanaDrawioHtmlTextSpacing(style) {
  return katanaDrawioHtmlTextNumberStyle(style, "spacing");
}

function katanaDrawioHtmlTextSpacingTop(style) {
  return katanaDrawioHtmlTextNumberStyle(style, "spacingTop");
}

function katanaDrawioHtmlTextSpacingBottom(style) {
  return katanaDrawioHtmlTextNumberStyle(style, "spacingBottom");
}

function katanaDrawioHtmlTextSpacingLeft(style) {
  return katanaDrawioHtmlTextNumberStyle(style, "spacingLeft");
}

function katanaDrawioHtmlTextSpacingRight(style) {
  return katanaDrawioHtmlTextNumberStyle(style, "spacingRight");
}

function katanaDrawioHtmlTextNumberStyle(style, name) {
  const value = Number(katanaDrawioStyleValue(style, name));
  return [value].filter(Number.isFinite).concat([0])[0];
}

function katanaDrawioRightAlignedHtmlTextOffset(style) {
  return katanaDrawioTextAlign(style) === "right" ? 2 : 0;
}

function katanaDrawioUsesTextShapeWrapInsets(style) {
  return [
    katanaDrawioIsTextShapeStyle(style),
    katanaDrawioStyleValue(style, "whiteSpace") === "wrap",
    katanaDrawioHtmlTextAlignTop(style),
    katanaDrawioStyleValue(style, "overflow") !== "hidden",
    !katanaDrawioIsSideHtmlTextLabel(style),
    !katanaDrawioIsBottomHtmlTextLabel(style),
    !katanaDrawioIsFillHtmlTextLabel(style),
  ].every(Boolean);
}
