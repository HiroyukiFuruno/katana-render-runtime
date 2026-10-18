function katanaNormalizeTreeViewSvg(svg, request) {
  if (!svg.includes('aria-roledescription="treeView"')) {
    return svg;
  }
  const normalized = svg
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
  return normalized;
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
  const svgWithIcons = katanaInsertArchitectureGroupIcons(
    katanaInsertArchitectureServiceIcons(svg, request),
  );
  const svgWithRows = katanaNormalizeArchitectureTextOuterRows(svgWithIcons);
  return svgWithRows;
}

function katanaNormalizeArchitectureTextOuterRows(svg) {
  return svg.replace(/class="text-outer-tspan(?! row)"/g, 'class="text-outer-tspan row"');
}

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

function katanaFormatSvgNumber(value) {
  return Number(value.toFixed(6)).toString();
}

function katanaNormalizeBlockSvg(svg) {
  return svg;
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
