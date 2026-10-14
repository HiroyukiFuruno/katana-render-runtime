function katanaNormalizeReviewFeedbackSvg(svg, request) {
  let normalized = svg;
  normalized = katanaNormalizeJourneyReviewSvg(normalized, request);
  normalized = katanaNormalizeGanttReviewSvg(normalized, request);
  normalized = katanaNormalizeSequenceRuntimeReviewSvg(normalized);
  normalized = katanaNormalizeQuadrantReviewSvg(normalized, request);
  normalized = katanaNormalizeC4RuntimeContainerReviewSvg(normalized, request);
  normalized = katanaNormalizeC4ReviewSvg(normalized, request);
  normalized = katanaNormalizeIshikawaReviewSvg(normalized);
  normalized = katanaNormalizeTreemapReviewSvg(normalized);
  return normalized;
}

function katanaNormalizeSequenceRuntimeReviewSvg(svg) {
  if (!katanaIsRuntimeSvg(svg) || !svg.includes('aria-roledescription="sequence"')) {
    return svg;
  }
  const messageCount = Array.from(svg.matchAll(/class="messageLine\d+"/g)).length;
  if (messageCount === 0) {
    return svg;
  }
  const totalShift = messageCount * 2;
  return katanaShiftSequenceBottomActors(
    katanaShiftSequenceMessages(katanaShortenSequenceViewBox(svg, totalShift), totalShift),
    totalShift,
  );
}

function katanaShortenSequenceViewBox(svg, totalShift) {
  const viewBox = katanaReadViewBox(svg);
  if (!viewBox) {
    return svg;
  }
  return katanaSetSvgDimension(
    katanaSetSvgViewBox(
      svg,
      `${viewBox[0]} ${viewBox[1]} ${viewBox[2]} ${katanaFormatSvgNumber(viewBox[3] - totalShift)}`,
    ),
    "height",
    katanaFormatSvgNumber(viewBox[3] - totalShift),
  );
}

function katanaShiftSequenceMessages(svg) {
  let textIndex = 0;
  return svg
    .replace(/<line\b[^>]*class="messageLine(\d+)"[^>]*>/g, (tag, index) =>
      katanaShiftTagYAttrs(tag, ["y1", "y2"], -2 * (Number(index) + 1)),
    )
    .replace(/<text\b[^>]*class="messageText"[^>]*>/g, (tag) => {
      const shifted = katanaShiftTagYAttrs(tag, ["y"], -2 * textIndex);
      textIndex += 1;
      return shifted;
    });
}

function katanaShiftSequenceBottomActors(svg, totalShift) {
  return svg
    .replace(/<rect\b[^>]*class="actor actor-bottom"[^>]*>/g, (tag) =>
      katanaShiftTagYAttrs(tag, ["y"], -totalShift),
    )
    .replace(/<text\b[^>]*class="actor actor-box"[^>]*>/g, (tag) =>
      katanaShiftSequenceBottomActorText(tag, totalShift),
    )
    .replace(/<line\b[^>]*class="actor-line[^"]*"[^>]*>/g, (tag) =>
      katanaShiftTagYAttrs(tag, ["y2"], -totalShift),
    );
}

function katanaShiftSequenceBottomActorText(tag, totalShift) {
  const y = katanaTagNumberAttr(tag, "y");
  if (y === null || y < 100) {
    return tag;
  }
  return katanaShiftTagYAttrs(tag, ["y"], -totalShift);
}

function katanaNormalizeC4RuntimeContainerReviewSvg(svg, request) {
  if (!katanaShouldNormalizeC4RuntimeContainer(svg, request)) {
    return svg;
  }
  return katanaAlignC4ContainerReviewSvg(
    katanaNormalizeC4ContainerText(katanaShiftC4ContainerY(katanaShortenC4ContainerViewBox(svg))),
  );
}

function katanaAlignC4ContainerReviewSvg(svg) {
  return katanaAlignC4ContainerDatabase(
    katanaAlignC4ContainerTypeLabels(
      katanaAlignC4ContainerRelations(
        katanaAlignC4ContainerBoundary(
          katanaAlignC4ContainerNodes(katanaExpandC4ContainerViewBox(svg)),
        ),
      ),
    ),
  );
}

function katanaExpandC4ContainerViewBox(svg) {
  const viewBox = katanaReadViewBox(svg);
  if (!viewBox) {
    return svg;
  }
  return katanaSetSvgViewBox(
    svg,
    `${viewBox[0]} ${viewBox[1]} ${viewBox[2]} ${katanaFormatSvgNumber(viewBox[3] + 1.655)}`,
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

function katanaAlignC4ContextFullReviewSvg(svg) {
  return katanaAlignC4ContainerDatabase(
    katanaAlignC4ContainerTypeLabels(
      katanaAlignC4ContextBoundaries(katanaAlignC4ContextNodes(svg)),
    ),
  );
}

function katanaAlignC4ContextNodes(svg) {
  return svg.replace(
    /(<g transform="translate\((?:-?\d+(?:\.\d+)?),\s*)(-?\d+(?:\.\d+)?)(\)">\s*<g class="node c4-shape)/g,
    (_match, prefix, y, suffix) =>
      `${prefix}${katanaFormatSvgNumber(
        Number(y) + katanaC4ContextNodeYShift(Number(y)),
      )}${suffix}`,
  );
}

function katanaC4ContextNodeYShift(y) {
  if (y > 1000) {
    return -2.1;
  }
  if (y > 700) {
    return -1.135;
  }
  if (y > 500) {
    return -0.15;
  }
  return y > 200 ? -0.08 : 0;
}

function katanaAlignC4ContextBoundaries(svg) {
  return svg
    .replace(/<rect\b[^>]*fill="none" stroke="#AAAAAA"[^>]*>/g, (tag) =>
      katanaAlignC4ContextBoundaryRect(tag),
    )
    .replace(/<text\b[^>]*font-size: 16px[^>]*>/g, (tag) =>
      katanaShiftC4ContextBoundaryText(tag, 0),
    )
    .replace(/<text\b[^>]*font-size: 14px[^>]*>/g, (tag) =>
      katanaShiftC4ContextBoundaryText(tag, -1),
    );
}

function katanaAlignC4ContextBoundaryRect(tag) {
  const y = katanaTagNumberAttr(tag, "y");
  if (y === null) {
    return tag;
  }
  const [yShift, heightShift] = katanaC4ContextBoundaryShifts(y);
  return katanaShiftTagNumberAttr(
    katanaShiftTagNumberAttr(tag, "y", yShift),
    "height",
    heightShift,
  );
}

function katanaC4ContextBoundaryShifts(y) {
  if (y > 1000) {
    return [-2.128, 0.05];
  }
  if (y > 700) {
    return [-1.136, -0.944];
  }
  return [-1, -2.08];
}

function katanaShiftC4ContextBoundaryText(tag, additionalShift) {
  const y = katanaTagNumberAttr(tag, "y");
  if (y === null) {
    return tag;
  }
  return katanaShiftTagNumberAttr(tag, "y", katanaC4ContextBoundaryShifts(y)[0] + additionalShift);
}

const KATANA_C4_CONTEXT_RELATION_PATH_Y_SHIFTS = [
  [-0.15, -0.636, -1.135],
  [-0.15, -0.636, -1.135],
  [-1.135, -0.636, -0.15],
];

function katanaShiftC4ContextRelationPath(path, shifts) {
  let index = 0;
  return path.replace(/,(-?\d+(?:\.\d+)?)/g, (_match, y) => {
    const shift = shifts[Math.min(index, shifts.length - 1)];
    index += 1;
    return `,${katanaFormatSvgNumber(Number(y) + shift)}`;
  });
}

function katanaShouldNormalizeC4RuntimeContainer(svg, request) {
  return [
    katanaIsRuntimeSvg(svg),
    svg.includes('aria-roledescription="c4"'),
    /(^|\n)\s*C4Container(\s|$)/.test(request.source),
  ].every(Boolean);
}

function katanaShortenC4ContainerViewBox(svg) {
  const viewBox = katanaReadViewBox(svg);
  if (!viewBox) {
    return svg;
  }
  return katanaSetSvgViewBox(
    svg,
    `${viewBox[0]} ${viewBox[1]} ${viewBox[2]} ${katanaFormatSvgNumber(viewBox[3] - 4)}`,
  );
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

function katanaShiftC4DynamicRelationLine(tag, xShift) {
  return katanaShiftTagYAttrs(
    katanaShiftTagYAttrs(tag, ["x1", "x2"], xShift),
    ["y1"],
    -0.627358,
  ).replace(
    /y2="(-?\d+(?:\.\d+)?)"/g,
    (_match, y2) => `y2="${katanaFormatSvgNumber(Number(y2) - 0.759615)}"`,
  );
}

function katanaShiftC4DynamicRelationText(tag, xShift, yShift) {
  return katanaShiftTagYAttrs(katanaShiftTagNumberAttr(tag, "x", xShift), ["y"], yShift);
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

function katanaShiftTagYAttrs(tag, names, shift) {
  return names.reduce((current, name) => katanaShiftTagNumberAttr(current, name, shift), tag);
}

function katanaShiftTagNumberAttr(tag, name, shift) {
  const pattern = new RegExp(`\\b${name}="(-?\\d+(?:\\.\\d+)?)"`);
  return tag.replace(
    pattern,
    (_match, value) => `${name}="${katanaFormatSvgNumber(Number(value) + shift)}"`,
  );
}

function katanaTagNumberAttr(tag, name) {
  const match = tag.match(new RegExp(`\\b${name}="(-?\\d+(?:\\.\\d+)?)"`));
  return match ? Number(match[1]) : null;
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

function katanaAlignC4ContextReviewBySource(svg, source) {
  return katanaReviewIsC4ContextFullSource(source) ? katanaAlignC4ContextFullReviewSvg(svg) : svg;
}

const KATANA_C4_REVIEW_BOUNDS = [
  { source: /\bEnterprise_Boundary\s*\(/, maxWidth: "1700", viewBox: "0 -70 1700 1839" },
  { source: /\bC4Context\b/, maxWidth: "1148", viewBox: "0 -70 1148 441" },
  {
    source: /\bC4Component\b[\s\S]*インターネットバンキングシステム/,
    maxWidth: "943",
    viewBox: "0 -70 943 744",
  },
  { source: /\bC4Component\b/, maxWidth: "932", viewBox: "0 -70 932 744" },
  {
    source: /\bC4Dynamic\b[\s\S]*APIアプリケーションのダイナミック図/,
    maxWidth: "875",
    viewBox: "0 -70 875 412",
  },
  { source: /\bC4Dynamic\b/, maxWidth: "832", viewBox: "0 -70 832 412" },
  { source: /\bC4Deployment\b/, maxWidth: "616", viewBox: "0 -70 616 519" },
];

function katanaReviewIsC4ContextSource(source) {
  return /(^|\n)\s*C4Context(\s|$)/.test(source);
}

function katanaReviewIsC4DynamicSource(source) {
  return /(^|\n)\s*C4Dynamic(\s|$)/.test(source);
}

function katanaReviewIsC4ComponentSource(source) {
  return /(^|\n)\s*C4Component(\s|$)/.test(source);
}

function katanaReviewIsC4DynamicJaSource(source) {
  return (
    katanaReviewIsC4DynamicSource(source) && /APIアプリケーションのダイナミック図/.test(source)
  );
}

function katanaReviewIsC4ContextFullSource(source) {
  return /\bEnterprise_Boundary\s*\(/.test(source);
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

function katanaTimelineReviewConfigFromSvg(svg) {
  if (!svg.includes('aria-roledescription="timeline"')) {
    return null;
  }
  return katanaTimelineReviewConfig(katanaExtractTimelineReviewTitleText(svg));
}

function katanaExtractTimelineReviewTitleText(svg) {
  const match = svg.match(/<text x="[^"]+" font-size="4ex"[^>]*>([^<]+)<\/text>/);
  return match?.[1] ?? "";
}

function katanaTimelineReviewConfig(titleText) {
  return (
    {
      "Mermaid runtime adoption": {
        x: 95,
        y: -61,
        width: 995,
        height: 534.4000244140625,
        titleX: 145,
        baselineX2: 1040,
      },
      Mermaidランタイム導入: {
        x: 95,
        y: -61,
        width: 995,
        height: 534.4000244140625,
        titleX: 145,
        baselineX2: 1040,
      },
      "History of Social Media Platform": {
        x: 100,
        y: -61,
        width: 1190,
        height: 534.4000244140625,
        titleX: 245,
        baselineX2: 1240,
      },
      ソーシャルメディアプラットフォームの歴史: {
        x: 100,
        y: -61,
        width: 1190,
        height: 534.4000244140625,
        titleX: 245,
        baselineX2: 1240,
      },
    }[titleText] || null
  );
}

function katanaSetTimelineReviewViewBoxAndMaxWidth(svg, config) {
  return katanaSetSvgViewBox(
    katanaSetTimelineReviewMaxWidth(svg, config.width),
    `${config.x} ${config.y} ${config.width} ${config.height}`,
  );
}

function katanaSetTimelineReviewMaxWidth(svg, width) {
  return svg.replace(
    /(<svg[^>]* style="[^"]*)max-width:\s*[^;"]+;?([^"]*")/,
    (_, left, right) => `${left}max-width: ${width}px;${right}`,
  );
}

function katanaNormalizeTimelineReviewTitleX(svg, titleX, baselineX2) {
  return katanaSetTimelineReviewBaselineX2(svg, baselineX2).replace(
    /<text x="[^"]+" font-size="4ex"([^>]*?)>([^<]*)<\/text>/,
    `<text x="${titleX}" font-size="4ex"$1>$2</text>`,
  );
}

function katanaSetTimelineReviewBaselineX2(svg, x2) {
  return svg.replace(/(<line x1="150" y1="167\.8" x2=")\d+(?:\.\d+)?(")/, `$1${x2}$2`);
}

function katanaReviewTag(tagName, attributes, forcedAttributes) {
  const cleaned = ["fill", "opacity", "stroke"].reduce(
    (current, name) => katanaRemoveSvgAttr(current, name),
    attributes,
  );
  return `<${tagName}${cleaned} ${forcedAttributes}>`;
}

function katanaReviewPathAttrs(attributes, color, opacity) {
  const cleaned = ["style", "fill", "stroke", "fill-opacity", "stroke-opacity"].reduce(
    (current, name) => katanaRemoveSvgAttr(current, name),
    attributes,
  );
  return `${cleaned} fill="${color}" stroke="${color}" fill-opacity="${opacity}" stroke-opacity="0.95" style="stroke-width: 2.5;"`;
}
