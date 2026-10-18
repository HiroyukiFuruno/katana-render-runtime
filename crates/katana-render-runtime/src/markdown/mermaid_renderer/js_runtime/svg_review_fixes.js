function katanaNormalizeReviewFeedbackSvg(svg, request) {
  let normalized = svg;
  normalized = katanaNormalizeJourneyReviewSvg(normalized, request);
  normalized = katanaNormalizeGanttReviewSvg(normalized, request);
  normalized = katanaNormalizeQuadrantReviewSvg(normalized, request);
  normalized = katanaNormalizeC4RuntimeContainerReviewSvg(normalized, request);
  normalized = katanaNormalizeC4ReviewSvg(normalized, request);
  normalized = katanaNormalizeIshikawaReviewSvg(normalized);
  normalized = katanaNormalizeTreemapReviewSvg(normalized);
  return normalized;
}

function katanaNormalizeC4RuntimeContainerReviewSvg(svg, request) {
  if (!katanaShouldNormalizeC4RuntimeContainer(svg, request)) {
    return svg;
  }
  return katanaAlignC4ContainerReviewSvg(
    katanaNormalizeC4ContainerText(katanaShiftC4ContainerY(svg)),
  );
}

function katanaAlignC4ContainerReviewSvg(svg) {
  return katanaAlignC4ContainerDatabase(
    katanaAlignC4ContainerTypeLabels(
      katanaAlignC4ContainerRelations(
        katanaAlignC4ContainerBoundary(katanaAlignC4ContainerNodes(svg)),
      ),
    ),
  );
}

function katanaAlignC4ContainerNodes(svg) {
  return svg.replace(
    /(<g transform="translate\((?:-?\d+(?:\.\d+)?),\s*)(-?\d+(?:\.\d+)?)(\)">\s*<g class="node c4-shape)/g,
    (_match, prefix, y, suffix) => `${prefix}${katanaFormatSvgNumber(Number(y) - 0.18)}${suffix}`,
  );
}

function katanaAlignC4ContainerBoundary(svg) {
  return svg
    .replace(/<rect\b[^>]*fill="none" stroke="#AAAAAA"[^>]*>/g, (tag) =>
      katanaShiftTagNumberAttr(katanaShiftTagNumberAttr(tag, "y", 2.34), "height", -0.184565),
    )
    .replace(/<text\b[^>]*font-size: 16px[^>]*>/g, (tag) =>
      katanaShiftTagNumberAttr(tag, "y", 2.34),
    )
    .replace(/<text\b[^>]*font-size: 14px[^>]*>/g, (tag) =>
      katanaShiftTagNumberAttr(tag, "y", 1.84),
    );
}

function katanaAlignC4ContainerRelations(svg) {
  return svg
    .replace(/<line\b[^>]*marker-end="[^"]+"[^>]*>/g, (tag) =>
      katanaShiftTagNumberAttr(katanaShiftTagNumberAttr(tag, "y1", 3.34), "y2", 2.84),
    )
    .replace(/<path\b[^>]*stroke="#AAAAAA"[^>]*>/g, (tag) =>
      tag.replace(/d="([^"]+)"/, (_match, path) => `d="${katanaShiftC4RelationPathY(path, 2.82)}"`),
    )
    .replace(/<text\b[^>]*font-size: 12px[^>]*>/g, (tag) =>
      katanaShiftTagNumberAttr(tag, "y", 2.82),
    );
}

function katanaShiftC4RelationPathY(path, shift) {
  return path.replace(
    /,(-?\d+(?:\.\d+)?)/g,
    (_match, y) => `,${katanaFormatSvgNumber(Number(y) + shift)}`,
  );
}

function katanaAlignC4ContainerTypeLabels(svg) {
  return svg.replace(
    /(<g class="c4-type" transform="translate\((?:-?\d+(?:\.\d+)?),\s*)(-?\d+(?:\.\d+)?)(\)">)/g,
    (_match, prefix, y, suffix) =>
      `${prefix}${katanaFormatSvgNumber(Number(y) + 0.04628)}${suffix}`,
  );
}

function katanaAlignC4ContainerDatabase(svg) {
  return svg.replace(/<path\b[^>]*class="basic label-container outer-path"[^>]*>/g, (tag) =>
    tag
      .replace(/l0,90\.18651/g, "l0,90.001945")
      .replace(/l0,-90\.18651/g, "l0,-90.001945")
      .replace(
        /transform="translate\((-?\d+(?:\.\d+)?),\s*(-?\d+(?:\.\d+)?)\)"/,
        (_match, x, y) =>
          `transform="translate(${x}, ${katanaFormatSvgNumber(Number(y) + 0.092283)})"`,
      ),
  );
}

function katanaShouldNormalizeC4RuntimeContainer(svg, request) {
  return [
    katanaIsRuntimeSvg(svg),
    svg.includes('aria-roledescription="c4"'),
    /(^|\n)\s*C4Container(\s|$)/.test(request.source),
  ].every(Boolean);
}

function katanaShiftC4ContainerY(svg) {
  const runtimeShift = Number(katanaIsRuntimeSvg(svg));
  return svg
    .replace(
      /\b(y|y1|y2)="(-?\d+(?:\.\d+)?)"/g,
      (_match, name, y) => `${name}="${katanaC4ShiftedY(Number(y), runtimeShift)}"`,
    )
    .replace(/d="([^"]+)"/g, (_match, path) => `d="${katanaShiftC4PathY(path, runtimeShift)}"`)
    .replace(/transform="translate\((-?\d+(?:\.\d+)?),\s*(-?\d+(?:\.\d+)?)\)"/g, (_match, x, y) =>
      Number(y) > 100
        ? `transform="translate(${x}, ${katanaFormatSvgNumber(
            Number(y) - runtimeShift - Number(runtimeShift > 0 && Number(y) > 300),
          )})"`
        : _match,
    );
}

function katanaShiftC4PathY(path, runtimeShift) {
  return path.replace(
    /,(-?\d+(?:\.\d+)?)/g,
    (_match, y) => `,${katanaC4ShiftedY(Number(y), runtimeShift)}`,
  );
}

function katanaC4ShiftedY(y, runtimeShift) {
  const thresholdRuntimeShift = runtimeShift + Number(runtimeShift > 0 && y > 300);
  if (y > 450) {
    return katanaFormatSvgNumber(y - 3 - thresholdRuntimeShift);
  }
  if (y > 300) {
    return katanaFormatSvgNumber(y - 2.5 - thresholdRuntimeShift);
  }
  if (y > 100) {
    return katanaFormatSvgNumber(y - 1 - runtimeShift);
  }
  return katanaFormatSvgNumber(y);
}

function katanaNormalizeC4ContainerText(svg) {
  return svg.replace(/font-size: (\d+);/g, "font-size: $1px;");
}

function katanaShiftTagNumberAttr(tag, name, shift) {
  const pattern = new RegExp(`\\b${name}="(-?\\d+(?:\\.\\d+)?)"`);
  return tag.replace(
    pattern,
    (_match, value) => `${name}="${katanaFormatSvgNumber(Number(value) + shift)}"`,
  );
}

function katanaIsRuntimeSvg(svg) {
  return svg.includes('id="katana-mermaid-svg-');
}

function katanaNormalizeJourneyReviewSvg(svg, request) {
  if (!svg.includes('aria-roledescription="journey"')) {
    return svg;
  }
  return svg
    .replace(/font-size: 14; font-family/g, "font-size: 14px; font-family")
    .replace("</style>", `text.journey-section{fill:${request.text}!important;}</style>`);
}

function katanaNormalizeGanttReviewSvg(svg, request) {
  if (!svg.includes('aria-roledescription="gantt"')) {
    return svg;
  }
  return katanaNormalizeGanttReviewTheme(katanaNormalizeGanttReviewSections(svg), request);
}

function katanaNormalizeGanttReviewSections(svg) {
  return svg
    .replace(/<rect\b([^>]*\bclass="section section0"[^>]*)>/g, (_match, attributes) =>
      katanaReviewTag(
        "rect",
        attributes,
        'fill="hsl(52.9411764706, 28.813559322%, 58.431372549%)" opacity="0.2"',
      ),
    )
    .replace(/<rect\b([^>]*\bclass="section section1"[^>]*)>/g, (_match, attributes) =>
      katanaReviewTag("rect", attributes, 'fill="transparent" opacity="0.2"'),
    )
    .replace("</style>", `.grid .tick{stroke:lightgrey!important;opacity:0.8!important;}</style>`);
}

function katanaNormalizeGanttReviewTheme(svg, request) {
  if (request.theme !== "dark") {
    return svg;
  }
  return katanaInsertSvgBackground(svg, "#1e1e1e");
}

function katanaNormalizeC4ReviewSvg(svg, request) {
  const normalized = katanaNormalizeC4ReviewSvgInternal(svg, request);
  if (normalized === null) {
    return svg;
  }
  return katanaNormalizeC4ReviewSvgBySource(normalized, request.source);
}

function katanaNormalizeC4ReviewSvgBySource(svg, source) {
  if (katanaReviewIsC4DynamicSource(source)) {
    return svg;
  }
  return svg;
}

function katanaNormalizeC4ReviewSvgInternal(svg, _request) {
  if (!svg.includes('aria-roledescription="c4"')) {
    return null;
  }
  return katanaNormalizeC4ContainerText(svg);
}

function katanaReviewIsC4DynamicSource(source) {
  return /(^|\n)\s*C4Dynamic(\s|$)/.test(source);
}

function katanaNormalizeQuadrantReviewSvg(svg, request) {
  if (!svg.includes('aria-roledescription="quadrantChart"')) {
    return svg;
  }
  return svg
    .replace(
      /fill="hsl\(0, 0%, NaN%\)" stroke="hsl\(0, 0%, NaN%\)"/g,
      `fill="${request.text}" stroke="${request.text}"`,
    )
    .replace(/style="stroke-width: 2"/g, 'style="stroke: rgb(136, 136, 136); stroke-width: 2;"');
}

function katanaReviewTag(tagName, attributes, forcedAttributes) {
  const cleaned = ["fill", "opacity", "stroke"].reduce(
    (current, name) => katanaRemoveSvgAttr(current, name),
    attributes,
  );
  return `<${tagName}${cleaned} ${forcedAttributes}>`;
}
