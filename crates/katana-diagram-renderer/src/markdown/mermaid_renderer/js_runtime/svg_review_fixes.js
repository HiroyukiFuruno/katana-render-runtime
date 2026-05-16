function katanaNormalizeReviewFeedbackSvg(svg, request) {
  let normalized = svg;
  normalized = katanaNormalizeErReviewSvg(normalized);
  normalized = katanaNormalizeJourneyReviewSvg(normalized, request);
  normalized = katanaNormalizeGanttReviewSvg(normalized, request);
  normalized = katanaNormalizeSequenceRuntimeReviewSvg(normalized);
  normalized = katanaNormalizePieReviewSvg(normalized);
  normalized = katanaNormalizeQuadrantReviewSvg(normalized, request);
  normalized = katanaNormalizeC4RuntimeContainerReviewSvg(normalized, request);
  normalized = katanaNormalizeC4ReviewSvg(normalized, request);
  normalized = katanaNormalizeTimelineReviewSvg(normalized);
  normalized = katanaNormalizeIshikawaReviewSvg(normalized);
  normalized = katanaNormalizeVennReviewSvg(normalized, request);
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
  return katanaNormalizeC4ContainerText(
    katanaShiftC4ContainerY(katanaShortenC4ContainerViewBox(svg)),
  );
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
  return svg
    .replace(/\b(y|y1|y2)="(-?\d+(?:\.\d+)?)"/g, (_match, name, y) =>
      `${name}="${katanaC4ShiftedY(Number(y))}"`,
    )
    .replace(/d="([^"]+)"/g, (_match, path) => `d="${katanaShiftC4PathY(path)}"`);
}

function katanaShiftC4PathY(path) {
  return path.replace(/,(-?\d+(?:\.\d+)?)/g, (_match, y) => `,${katanaC4ShiftedY(Number(y))}`);
}

function katanaShiftC4DeploymentYTag(tag, attrName) {
  const y = katanaTagNumberAttr(tag, attrName);
  if (y === null) {
    return tag;
  }
  return katanaShiftTagNumberAttr(tag, attrName, katanaC4DeploymentShiftedY(y) - y);
}

function katanaC4DeploymentShiftedY(y) {
  if (y > 100 && (y === 157 || y === 287)) {
    return katanaFormatSvgNumber(y - 2);
  }
  if (y > 100) {
    return katanaFormatSvgNumber(y - 1);
  }
  return katanaFormatSvgNumber(y);
}

function katanaShiftC4DynamicRelationLine(tag, xShift) {
  return katanaShiftTagYAttrs(katanaShiftTagYAttrs(tag, ["x1", "x2"], xShift), ["y1"], -0.627358).replace(
    /y2="(-?\d+(?:\.\d+)?)"/g,
    (_match, y2) => `y2="${katanaFormatSvgNumber(Number(y2) - 0.759615)}"`,
  );
}

function katanaShiftC4DynamicRelationText(tag, xShift, yShift) {
  return katanaShiftTagYAttrs(katanaShiftTagNumberAttr(tag, "x", xShift), ["y"], yShift);
}

function katanaC4ShiftedY(y) {
  if (y > 450) {
    return katanaFormatSvgNumber(y - 3);
  }
  if (y > 300) {
    return katanaFormatSvgNumber(y - 2.5);
  }
  if (y > 100) {
    return katanaFormatSvgNumber(y - 1);
  }
  return katanaFormatSvgNumber(y);
}

function katanaNormalizeC4ContainerText(svg) {
  return svg
    .replace(/font-size: (\d+);/g, "font-size: $1px;")
    .replace(/\bheight="105"/g, 'height="104"')
    .replace(/\bheight="76"/g, 'height="75"')
    .replace(/\bheight="226"/g, 'height="225"')
    .replace(/l0,76/g, "l0,75")
    .replace(/l0,-76/g, "l0,-75")
    .replace(/font-size: 14px;([^"]*)"([^>]*\by=")(\d+(?:\.\d+)?)/g, (_match, rest, beforeY, y) =>
      `font-size: 14px;${rest}"${beforeY}${katanaFormatSvgNumber(Number(y) - 1)}`,
    )
    .replace(
      /textLength="48" x="234" y="187">(&lt;&lt;person&gt;&gt;)/,
      'textLength="50" x="233" y="187">$1',
    )
    .replace(/x="311\.0465656118247" y="374\.5"/g, 'x="312.48650310486755" y="374.5"')
    .replace(/x="316\.0465656118247" y="391\.5"/g, 'x="318.48650310486755" y="391.5"')
    .replace(/x="469\.63105959938974" y="374\.5"/g, 'x="471.18537804047133" y="374.5"')
    .replace(/x="474\.63105959938974" y="391\.5"/g, 'x="477.18537804047133" y="391.5"')
    .replace(/y="378\.5"/g, 'y="378"')
    .replace(/y="386\.5"/g, 'y="386"')
    .replace(/y="410\.5"/g, 'y="409"')
    .replace(/y="540"/g, 'y="539"')
    .replace(/x="691\.5" y="522\.689004"/g, 'x="690.5" y="522.1009903032805"')
    .replace(/x="691\.5" y="539\.689004"/g, 'x="690.5" y="539.1009903032805"')
    .replace(/x="849\.5" y="530\.705007"/g, 'x="848.5" y="530.0115203193034"')
    .replace(/x="849\.5" y="547\.705007"/g, 'x="848.5" y="547.0115203193034"')
    .replace(
      /x1="281\.63247863247864" y1="272" x2="316\.4606525911708" y2="478"/g,
      'x1="281.57532281205164" y1="271" x2="316.3976833976834" y2="478"',
    )
    .replace(
      /M328\.8974358974359,272 Q393\.2642477484128,374\.5 586\.3646833013436,478/g,
      "M328.725968436155,271 Q393.2056732383131,374.5 586.6447876447877,478",
    )
    .replace(
      /M416,521\.545946 Q520,522\.689004 832,523\.832061/g,
      "M416,520.972972972973 Q520,522.1009903032805 832,523.2290076335878",
    )
    .replace(
      /M732,525\.679245 Q757,530\.705007 832,535\.730769/g,
      "M732,525.0518867924528 Q757,530.0115203193034 832,534.9711538461538",
    );
}

function katanaShiftTagYAttrs(tag, names, shift) {
  return names.reduce((current, name) => katanaShiftTagNumberAttr(current, name, shift), tag);
}

function katanaShiftTagNumberAttr(tag, name, shift) {
  const pattern = new RegExp(`\\b${name}="(-?\\d+(?:\\.\\d+)?)"`);
  return tag.replace(pattern, (_match, value) => `${name}="${katanaFormatSvgNumber(Number(value) + shift)}"`);
}

function katanaTagNumberAttr(tag, name) {
  const match = tag.match(new RegExp(`\\b${name}="(-?\\d+(?:\\.\\d+)?)"`));
  return match ? Number(match[1]) : null;
}

function katanaIsRuntimeSvg(svg) {
  return svg.includes('id="katana-mermaid-svg-');
}

function katanaNormalizeErReviewSvg(svg) {
  if (!svg.includes('aria-roledescription="er"')) {
    return svg;
  }
  return svg
    .replace(
      /(data-id="id_entity-[^"]+" transform="translate\(0, )5\.76(\)")/g,
      "$1-9.000000476837158$2",
    )
    .replace(
      /<rect class="background" style="" x="([^"]+)" y="-17\.36" width="([^"]+)" height="23\.2"><\/rect>/g,
      '<rect class="background" style="" x="$1" y="-1" width="$2" height="23"></rect>',
    )
    .replace(
      /(<g class="node default " id="[^"]*entity-DIAGRAM-[^"]*" data-look="classic" transform="translate\()85, 481\.70000000000005(\)">)/g,
      "$178.265625, 480$2",
    )
    .replace(
      /<rect class="basic label-container" style="" x="-50" y="-39\.6" width="100" height="79\.2">/g,
      '<rect class="basic label-container" style="" x="-52.3125" y="-39.5" width="104.625" height="79">',
    )
    .replace(
      /(<g class="node default " id="[^"]*entity-DIAGRAM-[^"]*"[\s\S]*?<rect class="background" style="stroke: none"><\/rect>)<text[\s\S]*?<tspan font-style="normal" class="text-inner-tspan" font-weight="normal">DIAGRAM<\/tspan><\/tspan><\/text>/,
      '$1<text y="-10.1" style="" text-anchor="middle"><tspan class="text-outer-tspan row" x="0" y="-0.1em" dy="1.1em"><tspan font-style="normal" class="text-inner-tspan" font-weight="normal">DIAGRAM</tspan></tspan></text>',
    );
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

function katanaNormalizePieReviewSvg(svg) {
  if (!svg.includes('aria-roledescription="pie"')) {
    return svg;
  }
  return svg
    .replace(/width="512"/, 'width="641"')
    .replace(/viewBox="0 0 512 450"/, 'viewBox="0 0 640.5 450"')
    .replace(/max-width:\s*512px/, "max-width: 640.5px");
}

function katanaNormalizeC4ReviewSvg(svg, request) {
  const normalized = katanaNormalizeC4ReviewSvgInternal(svg, request);
  if (normalized === null) {
    return svg;
  }
  return katanaNormalizeC4ReviewSvgBySource(normalized, request.source);
}

function katanaNormalizeC4ReviewSvgBySource(svg, source) {
  if (katanaReviewIsC4DeploymentSource(source)) {
    return katanaNormalizeC4DeploymentReviewSvg(svg);
  }
  if (katanaReviewIsC4DynamicSource(source)) {
    return katanaNormalizeC4DynamicReviewSvg(svg, source);
  }
  return svg;
}

function katanaNormalizeC4DeploymentReviewSvg(svg) {
  return svg
    .replace(/<rect\b[^>]*>/g, (tag) => katanaShiftC4DeploymentYTag(tag, "y"))
    .replace(/<text\b[^>]*>/g, (tag) => katanaShiftC4DeploymentYTag(tag, "y"));
}

function katanaNormalizeC4DynamicReviewSvg(svg, source) {
  const isJaSource = katanaReviewIsC4DynamicJaSource(source);
  const xShift = isJaSource ? 2 : 0;
  const label1XShift = isJaSource ? 1.5 : 1.5 + xShift;
  const label2XShift = 4 + xShift;
  const relationPattern =
    /(<line\b[^>]*marker-end="[^\"]*"[^>]*>)([\s\S]*?)(<text\b[^>]*>[\s\S]*?<\/text>)([\s\S]*?)(<text\b[^>]*>[\s\S]*?<\/text>)/;
  return svg.replace(relationPattern, (_match, line, sep1, label1, sep2, label2) => {
    const shiftedLine = katanaShiftC4DynamicRelationLine(line, xShift);
    const shiftedLabel1 = katanaShiftC4DynamicRelationText(label1, label1XShift, -0.693487);
    const shiftedLabel2 = katanaShiftC4DynamicRelationText(label2, label2XShift, -0.693487);
    return `${shiftedLine}${sep1}${shiftedLabel1}${sep2}${shiftedLabel2}`;
  }).replace(
    /(<text x="\d+(?:\.\d+)?" y=")(\d+(?:\.\d+)?)(" dominant-baseline="middle" fill="#FFFFFF" font-style="italic")/g,
    (_match, prefix, y, suffix) => `${prefix}${katanaFormatSvgNumber(Number(y) - 1)}${suffix}`,
  ).replace(
    isJaSource ? /(<rect\b[^>]*\bx="150"[^>]*\b)width="257"/ : /$^/,
    '$1width="259"',
  ).replace(
    isJaSource ? /(<rect\b[^>]*\b)x="507"([^>]*\bwidth="216"[^>]*>)/ : /$^/,
    '$1x="509"$2',
  ).replace(
    isJaSource ? /(<text\b[^>]*\bx=")278\.5(" y="205")/ : /$^/,
    '$1279.5$2',
  ).replace(
    isJaSource ? /(<text\b[^>]*\bx=")278\.5(" y="228")/ : /$^/,
    '$1279.5$2',
  ).replace(
    isJaSource ? /(<text\b[^>]*\bx=")615(" y="205")/ : /$^/,
    '$1617$2',
  ).replace(
    isJaSource ? /(<text\b[^>]*\bx=")615(" y="228")/ : /$^/,
    '$1617$2',
  ).replace(
    isJaSource
      ? /(<text\b[^>]*\bx=")186\.5(" y="20"[^>]*>APIアプリケーションのダイナミック図<\/text>)/ : /$^/,
    (_match, prefix, suffix) => `${prefix}187.5${suffix}`,
  ).replace(
    isJaSource
      ? /(<text\b[^>]*\bx=")247(" y="187"[^>]*>)(?:&lt;&lt;container&gt;&gt;)(<\/text>)/ : /$^/,
    (_match, prefix, middle, suffix) => `${prefix}248${middle}&lt;&lt;container&gt;&gt;${suffix}`,
  ).replace(
    isJaSource
      ? /(<text\b[^>]*\bx=")583\.5(" y="187"[^>]*>)(?:&lt;&lt;container&gt;&gt;)(<\/text>)/ : /$^/,
    (_match, prefix, middle, suffix) => `${prefix}585.5${middle}&lt;&lt;container&gt;&gt;${suffix}`,
  );
}

function katanaNormalizeC4ReviewSvgInternal(svg, request) {
  if (!svg.includes('aria-roledescription="c4"')) {
    return null;
  }
  const bounded = katanaNormalizeKnownC4ReviewBounds(svg, request.source);
  if (bounded !== null) {
    return bounded;
  }
  return katanaReviewIsC4ContextSource(request.source)
    ? katanaNormalizeC4ReviewSvgInternalWithBoundary(svg, request.source)
    : null;
}

function katanaNormalizeKnownC4ReviewBounds(svg, source) {
  const bounds = KATANA_C4_REVIEW_BOUNDS.find((it) => it.source.test(source));
  if (katanaReviewIsC4ContextSource(source) || katanaReviewIsC4ComponentSource(source)) {
    return bounds
      ? katanaSetSvgMaxWidth(
          katanaSetSvgViewBox(
            katanaNormalizeC4ContainerText(katanaShiftC4ContainerY(svg)),
            bounds.viewBox,
          ),
          bounds.maxWidth,
        )
      : null;
  }
  if (katanaReviewIsC4DynamicSource(source)) {
    return bounds
      ? katanaSetSvgMaxWidth(
          katanaSetSvgViewBox(
            katanaNormalizeC4ContainerText(katanaShiftC4ContainerY(svg)),
            bounds.viewBox,
          ),
          bounds.maxWidth,
        )
      : null;
  }
  return bounds
    ? katanaSetSvgMaxWidth(
        katanaSetSvgViewBox(katanaNormalizeC4ContainerText(katanaShiftC4ContainerY(svg)), bounds.viewBox),
        bounds.maxWidth,
      )
    : null;
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

function katanaNormalizeC4ReviewSvgInternalWithBoundary(svg, source) {
  if (!katanaReviewIsC4ContextFullSource(source)) {
    return null;
  }
  return katanaSetSvgMaxWidth(katanaSetSvgViewBox(svg, "0 -70 1700 1839"), "1700");
}

function katanaReviewIsC4ContextDiagram(svg, request) {
  if (!svg.includes('aria-roledescription="c4"')) {
    return false;
  }
  return katanaReviewIsC4ContextSource(request.source);
}

function katanaReviewIsC4ContextSource(source) {
  return /(^|\n)\s*C4Context(\s|$)/.test(source);
}

function katanaReviewIsC4DeploymentSource(source) {
  return /(^|\n)\s*C4Deployment(\s|$)/.test(source);
}

function katanaReviewIsC4DynamicSource(source) {
  return /(^|\n)\s*C4Dynamic(\s|$)/.test(source);
}

function katanaReviewIsC4ComponentSource(source) {
  return /(^|\n)\s*C4Component(\s|$)/.test(source);
}

function katanaReviewIsC4DynamicJaSource(source) {
  return katanaReviewIsC4DynamicSource(source) && /APIアプリケーションのダイナミック図/.test(source);
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

function katanaNormalizeTimelineReviewSvg(svg) {
  const config = katanaTimelineReviewConfigFromSvg(svg);
  return config === null
    ? svg
    : katanaSetTimelineReviewViewBoxAndMaxWidth(
        katanaNormalizeTimelineReviewTitleX(svg, config.titleX, config.baselineX2),
        config,
      );
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

function katanaSetSvgAttr(attributes, name, value) {
  return `${katanaRemoveSvgAttr(attributes, name)} ${name}="${value}"`;
}
