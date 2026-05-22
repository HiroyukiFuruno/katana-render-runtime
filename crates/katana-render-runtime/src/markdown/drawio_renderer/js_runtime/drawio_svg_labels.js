function katanaFillMissingDrawioTextLabels(svg) {
  katanaDrawioSourceLabelEntries().forEach((entry) => {
    katanaFillMissingDrawioCellLabel(svg, entry);
  });
}

function katanaDrawioRequestSource() {
  return String(globalThis.__katanaDrawioRequest?.source ?? "");
}

function katanaFillMissingDrawioCellLabel(svg, entry) {
  [katanaDrawioCellGroup(svg, entry.id)]
    .filter(Boolean)
    .filter(katanaShouldInsertDrawioText)
    .forEach((group) => {
      group.appendChild(katanaCreateDrawioTextLabel(group, entry));
    });
}

function katanaDrawioCellGroup(svg, id) {
  return (
    Array.from(svg.querySelectorAll("g")).find(
      (node) => node.getAttribute("data-cell-id") === id,
    ) ?? null
  );
}

function katanaShouldInsertDrawioText(group) {
  return [group, !katanaDrawioCellHasText(group), katanaDrawioCellShapeBox(group)].every(Boolean);
}

function katanaDrawioCellHasText(group) {
  return group.querySelectorAll("text").length > 0;
}

function katanaCreateDrawioTextLabel(group, entry) {
  const box = katanaDrawioCellShapeBox(group);
  const lines = katanaDrawioTextLines(entry.label);
  const text = katanaDrawioTextElement(box, entry, lines);
  lines.forEach((line, index) => {
    text.appendChild(katanaCreateDrawioTextLine(text, line, index, entry.style));
  });
  return text;
}

function katanaDrawioTextElement(box, entry, lines) {
  const text = document.createElementNS("http://www.w3.org/2000/svg", "text");
  const fontSize = katanaDrawioFontSize(entry.style);
  const position = katanaDrawioLabelPosition(box, entry, lines, fontSize);
  text.setAttribute("x", position.x);
  text.setAttribute("y", position.y);
  text.setAttribute("fill", katanaDrawioTextColor(entry.style));
  text.setAttribute("font-family", "Helvetica");
  text.setAttribute("font-size", `${fontSize}px`);
  text.setAttribute("text-anchor", position.anchor);
  return text;
}

function katanaCreateDrawioTextLine(text, line, index, style) {
  const tspan = document.createElementNS("http://www.w3.org/2000/svg", "tspan");
  tspan.setAttribute("x", text.getAttribute("x"));
  tspan.setAttribute("dy", katanaDrawioTextLineDy(index, katanaDrawioFontSize(style)));
  tspan.textContent = line;
  return tspan;
}

function katanaDrawioCellShapeBox(group) {
  return katanaDrawioUnionBox(
    katanaDrawioCellShapeElements(group).map(katanaDrawioElementBox).filter(katanaDrawioHasArea),
  );
}

function katanaDrawioCellShapeElements(group) {
  return katanaDrawioCellShapeTagNames().flatMap((tagName) =>
    Array.from(group.querySelectorAll(tagName)),
  );
}

function katanaDrawioCellShapeTagNames() {
  return ["rect", "path", "ellipse", "circle", "polygon", "polyline", "image"];
}

function katanaDrawioTextLines(label) {
  return String(label).split(/\n+/).filter(Boolean);
}

function katanaDrawioLabelPosition(box, entry, lines, fontSize) {
  return katanaDrawioLabelPositioner(entry.style)(box, entry, lines, fontSize);
}

function katanaDrawioLabelPositioner(style) {
  return (
    KATANA_DRAWIO_LABEL_POSITIONERS.get(katanaDrawioStyleValue(style, "labelPosition")) ??
    KATANA_DRAWIO_LABEL_POSITIONERS.get(katanaDrawioStyleValue(style, "verticalLabelPosition")) ??
    katanaDrawioCenterLabelPosition
  );
}

function katanaDrawioCenterLabelPosition(box, _entry, lines, fontSize) {
  return katanaDrawioTextPosition(
    box.x + box.width / 2,
    katanaDrawioCenteredTextY(box, lines, fontSize),
    "middle",
  );
}

function katanaDrawioBottomLabelPosition(box, entry, _lines, fontSize) {
  return katanaDrawioTextPosition(
    box.x + box.width / 2,
    box.y + box.height + fontSize + 7 + katanaDrawioSpacing(entry.style, "spacingTop"),
    "middle",
  );
}

function katanaDrawioRightLabelPosition(box, entry, lines, fontSize) {
  const x = box.x + box.width + 2 + katanaDrawioSpacing(entry.style, "spacingLeft") -
    katanaDrawioAppliedLeftPadding() * 2;
  return katanaDrawioTextPosition(x, katanaDrawioCenteredTextY(box, lines, fontSize), "start");
}

function katanaDrawioLeftLabelPosition(box, entry, lines, fontSize) {
  const x = box.x - 2 - katanaDrawioSpacing(entry.style, "spacingRight") -
    katanaDrawioAppliedLeftPadding();
  return katanaDrawioTextPosition(x, katanaDrawioCenteredTextY(box, lines, fontSize), "end");
}

function katanaDrawioTextPosition(x, y, anchor) {
  return { x, y, anchor };
}

function katanaDrawioCenteredTextY(box, lines, fontSize) {
  return box.y + box.height / 2 + fontSize / 3 - ((lines.length - 1) * fontSize * 1.25) / 2;
}

function katanaDrawioSpacing(style, name) {
  const value = Number(katanaDrawioStyleValue(style, name));
  return [value].filter(Number.isFinite).concat([0])[0];
}

function katanaDrawioTextLineDy(index, fontSize) {
  return ["0", String(Math.ceil(fontSize * 1.25))][Number(index > 0)];
}

const KATANA_DRAWIO_LABEL_POSITIONERS = new Map([
  ["bottom", katanaDrawioBottomLabelPosition],
  ["left", katanaDrawioLeftLabelPosition],
  ["right", katanaDrawioRightLabelPosition],
]);
