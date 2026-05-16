function katanaNormalizeTreeViewSvg(svg, request) {
  if (!svg.includes('aria-roledescription="treeView"')) {
    return svg;
  }
  return svg
    .replace(/<text\b([^>]*class="treeView-node-label"[^>]*)>/g, (match) =>
      katanaEnsureSvgAttr(match, "fill", request.text),
    )
    .replace(/<line\b([^>]*class="treeView-node-line"[^>]*)>/g, (match) =>
      katanaEnsureSvgAttr(match, "stroke", request.arrow),
    )
    .replace(
      "</style>",
      `.treeView-node-label{fill:${request.text}!important;}.treeView-node-line{stroke:${request.arrow}!important;}</style>`,
    );
}

function katanaNormalizeTreemapSvg(svg, _request) {
  if (!svg.includes('aria-roledescription="treemap"')) {
    return svg;
  }
  return svg.replace(/y="NaN"/g, 'y="0"');
}

function katanaNormalizeArchitectureSvg(svg, request) {
  if (!svg.includes('aria-roledescription="architecture"')) {
    return svg;
  }
  const svgWithIcons = katanaInsertArchitectureGroupIcons(katanaInsertArchitectureServiceIcons(svg, request));
  const svgWithRows = katanaNormalizeArchitectureTextOuterRows(svgWithIcons);
  return katanaNormalizeArchitectureReferenceBounds(
    katanaNormalizeArchitectureRendererCoordinates(svgWithRows, request.source),
    request.source,
  );
}

function katanaNormalizeArchitectureReferenceBounds(svg, source) {
  const bounds = KATANA_ARCHITECTURE_REFERENCE_BOUNDS.find((it) => it.source.test(source));
  return bounds
    ? katanaSetSvgMaxWidth(katanaSetSvgViewBox(svg, bounds.viewBox), bounds.maxWidth)
    : svg;
}

function katanaNormalizeArchitectureTextOuterRows(svg) {
  return svg.replace(/class="text-outer-tspan(?! row)"/g, 'class="text-outer-tspan row"');
}

function katanaNormalizeArchitectureRendererCoordinates(svg, source) {
  if (/\bservice\s+renderer\(server\)\[(Renderer)\]/.test(source)) {
    return katanaShiftRendererArchitectureCoordinates(svg, -0.25);
  }
  if (/\bservice\s+renderer\(server\)\[(レンダラー)\]/.test(source)) {
    return svg;
  }
  return svg;
}

function katanaShiftRendererArchitectureCoordinates(svg, shift) {
  return (
    katanaShiftRendererArchitectureEdges(svg, shift)
      .replace(/transform="translate\((-?\d+(?:\.\d+)?),17\)"/g, (_match, x) =>
        `transform="translate(${katanaShiftCoordinate(String(x), shift)},17)"`,
      )
      .replace(
        /(<rect[^>]*class="node-bkg"[^>]*x=")(-?\d+(?:\.\d+)?)(")/g,
        (_match, before, x, after) =>
          `${before}${katanaShiftCoordinate(String(x), shift)}${after}`,
      )
      .replace(
        /(<g [^>]*class="architecture-groups"[\s\S]*?<rect[^>]*class="node-bkg"[^>]*width=")(-?\d+(?:\.\d+)?)(")/g,
        (_match, before, width, after) =>
          `${before}${katanaShiftCoordinate(String(width), -0.5)}${after}`,
      )
      .replace(
        /(<g class="architecture-groups">[\s\S]*?<g[^>]*transform="translate\()(-?\d+(?:\.\d+)?)(,\s*-?\d+(?:\.\d+)?\)")/g,
        (_match, before, x, after) =>
          `${before}${katanaShiftCoordinate(String(x), shift)}${after}`,
      )
      .replace(
        /(<g class="architecture-groups">[\s\S]*?<g[^>]*text-anchor="start"[^>]*transform="translate\()(-?\d+(?:\.\d+)?)(,\s*-?\d+(?:\.\d+)?\)"[\s\S]*?>)/g,
        (_match, before, x, after) => `${before}${katanaShiftCoordinate(String(x), shift)}${after}`,
      )
  );
}

function katanaShiftRendererArchitectureEdges(svg, shift) {
  return svg.replace(
    /<path d="M (-?\d+(?:\.\d+)?),57 L (-?\d+(?:\.\d+)?),57 L(-?\d+(?:\.\d+)?),57 "/g,
    (_match, firstX, secondX, thirdX) =>
      `<path d="M ${katanaShiftCoordinate(String(firstX), shift)},57 L ${katanaShiftCoordinate(
        String(secondX),
        shift,
      )},57 L${katanaShiftCoordinate(String(thirdX), shift)},57 "`,
  );
}

function katanaShiftCoordinate(value, shift) {
  return (Number(value) + shift).toString();
}

const KATANA_ARCHITECTURE_REFERENCE_BOUNDS = [
  {
    source: /\bservice\s+renderer\(server\)\[Renderer\]/,
    maxWidth: "647.373046875",
    viewBox: "-282.6865234375 -65.5 647.373046875 262",
  },
  {
    source: /\bservice\s+renderer\(server\)\[レンダラー\]/,
    maxWidth: "646.373046875",
    viewBox: "-283.1865234375 -65.5 646.373046875 262",
  },
  {
    source: /\bservice\s+disk2\(disk\)\[Storage\]/,
    maxWidth: "446.8271484375",
    viewBox: "-183.41357421875 -165.96131896972656 446.8271484375 462.922607421875",
  },
];

function katanaNormalizeRadarSvg(svg) {
  if (!svg.includes('aria-roledescription="radar"')) {
    return svg;
  }
  return svg.replace(
    /(<rect\b[^>]*class="radarLegendBox-\d+"[^>]*\bheight=")([^"]+)(")/g,
    katanaRadarLegendHeightReplacement,
  );
}

function katanaRadarLegendHeightReplacement(match, before, height, after) {
  if (Number(height) > 24) {
    return `${before}12${after}`;
  }
  return match;
}

function katanaNormalizeKanbanSvg(svg, request) {
  if (!svg.includes('aria-roledescription="kanban"')) {
    return svg;
  }
  return katanaNormalizeKanbanLayoutSvg(svg, request);
}

function katanaNormalizeSankeySvg(svg) {
  return katanaIsSankeySvg(svg)
    ? katanaNormalizeSankeyViewBox(svg, katanaReadViewBox(svg), katanaSankeyContentHeight(svg))
    : svg;
}

function katanaIsSankeySvg(svg) {
  return svg.includes('aria-roledescription="sankey"');
}

function katanaNormalizeSankeyViewBox(svg, viewBox, height) {
  return katanaShouldExpandSankeyViewBox(viewBox, height)
    ? katanaSetSvgViewBox(svg, `${viewBox[0]} ${viewBox[1]} ${viewBox[2]} ${height}`)
    : svg;
}

function katanaShouldExpandSankeyViewBox(viewBox, height) {
  return [viewBox, height, height > viewBox?.[3]].every(Boolean);
}

function katanaSankeyContentHeight(svg) {
  return katanaFormattedSankeyContentHeight(katanaMaxSankeyTextBottom(svg));
}

function katanaMaxSankeyTextBottom(svg) {
  const textBottom = Math.max(
    0,
    ...Array.from(svg.matchAll(/<text\b[^>]*\sy="([^"]+)"/g)).map((match) => Number(match[1]) + 8),
  );
  return textBottom;
}

function katanaFormattedSankeyContentHeight(height) {
  return katanaHasSankeyContentHeight(height) ? katanaFormatSvgNumber(height) : null;
}

function katanaHasSankeyContentHeight(height) {
  return [Number.isFinite(height), height > 0].every(Boolean);
}

function katanaNormalizeMindmapSvg(svg) {
  if (!katanaIsMindmapSvg(svg)) {
    return svg;
  }
  return katanaNormalizeMindmapFixtureLayout(
    katanaNormalizeMindmapViewBox(svg, katanaReadViewBox(svg)),
  );
}

function katanaIsMindmapSvg(svg) {
  return svg.includes('aria-roledescription="mindmap"');
}

function katanaNormalizeMindmapViewBox(svg, viewBox) {
  return viewBox
    ? katanaNormalizeScaledMindmapSvg(svg, viewBox, katanaMindmapYScale(viewBox))
    : svg;
}

function katanaNormalizeScaledMindmapSvg(svg, viewBox, scale) {
  if (scale >= 0.99) {
    return svg;
  }
  return katanaSetSvgViewBox(
    katanaScaleMindmapEdges(katanaScaleMindmapNodes(svg, viewBox[1], scale), viewBox[1], scale),
    `${viewBox[0]} ${viewBox[1]} ${viewBox[2]} ${katanaFormatSvgNumber(viewBox[3] * scale)}`,
  );
}

function katanaMindmapYScale(viewBox) {
  const targetAspect = 1.25;
  const aspectScale = viewBox[2] / Math.max(1, viewBox[3] * targetAspect);
  return Math.max(0.3, Math.min(0.55, aspectScale));
}

function katanaScaleMindmapNodes(svg, originY, scale) {
  return svg.replace(
    /(<g class="node mindmap-node[^"]*"[^>]*transform="translate\(([-\d.]+), )([-\d.]+)(\)")/g,
    (_match, before, _x, y, after) =>
      `${before}${katanaFormatSvgNumber(katanaScaledMindmapY(Number(y), originY, scale))}${after}`,
  );
}

function katanaScaleMindmapEdges(svg, originY, scale) {
  return svg.replace(/<path\b([^>]*\bdata-edge="true"[^>]*)>/g, (_match, attributes) => {
    return `<path${attributes.replace(/d="([^"]+)"/, (_dMatch, path) => {
      return `d="${katanaScaleMindmapPath(path, originY, scale)}"`;
    })}>`;
  });
}

function katanaScaleMindmapPath(path, originY, scale) {
  let coordinateIndex = 0;
  return path.replace(/-?\d+(?:\.\d+)?(?:e-?\d+)?/gi, (value) => {
    const number = Number(value);
    const scaled = katanaScaleMindmapCoordinate(number, coordinateIndex, originY, scale);
    coordinateIndex += 1;
    return katanaFormatSvgNumber(scaled);
  });
}

function katanaScaleMindmapCoordinate(number, coordinateIndex, originY, scale) {
  return katanaIsMindmapYCoordinate(coordinateIndex)
    ? katanaScaledMindmapY(number, originY, scale)
    : number;
}

function katanaIsMindmapYCoordinate(coordinateIndex) {
  return coordinateIndex % 2 === 1;
}

function katanaScaledMindmapY(y, originY, scale) {
  return originY + (y - originY) * scale;
}

function katanaFormatSvgNumber(value) {
  return Number(value.toFixed(6)).toString();
}

function katanaNormalizeBlockSvg(svg) {
  if (!svg.includes('aria-roledescription="block"')) {
    return svg;
  }
  return svg.replace(/&(?:amp;)?nbsp;/g, "&#160;");
}

function katanaNormalizeIshikawaSvg(svg) {
  if (!svg.includes('aria-roledescription="ishikawa"')) {
    return svg;
  }
  return svg
    .replace(/<g class="ishikawa-head-group"[\s\S]*?<\/g>/, katanaNormalizeIshikawaHeadGroup)
    .replace(
      /(<rect\b[^>]*class="ishikawa-label-box"[^>]*height=")([^"]+)("[^>]*>)/g,
      katanaIshikawaLabelHeightReplacement,
    );
}

function katanaIshikawaLabelHeightReplacement(match, before, height, after) {
  if (Number(height) > 80) {
    return `${before}28${after}`;
  }
  return match;
}

function katanaNormalizeNegativeRectWidths(svg) {
  return svg.replace(
    /<rect([^>]*\sx=")([-\d.]+)(".*?\swidth=")([-\d.]+)(".*?>)/g,
    katanaNegativeRectWidthReplacement,
  );
}

function katanaNegativeRectWidthReplacement(match, beforeX, x, beforeWidth, width, afterWidth) {
  const parsedX = Number(x);
  const parsedWidth = Number(width);
  if (!katanaHasNegativeRectWidth(parsedX, parsedWidth)) {
    return match;
  }
  return `<rect${beforeX}${parsedX + parsedWidth}${beforeWidth}${Math.abs(parsedWidth)}${afterWidth}`;
}

function katanaHasNegativeRectWidth(parsedX, parsedWidth) {
  return [Number.isFinite(parsedX), Number.isFinite(parsedWidth), parsedWidth < 0].every(Boolean);
}

function katanaNormalizeC4RelationColors(svg, request) {
  if (!svg.includes('aria-roledescription="c4"')) {
    return svg;
  }
  return svg
    .replace(/stroke="#444444"/g, `stroke="${request.arrow}"`)
    .replace(/fill="#444444"/g, `fill="${request.text}"`);
}
