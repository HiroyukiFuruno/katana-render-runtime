function katanaApplyFlowchartFixtureLayout(svg, layout) {
  const sized = katanaSetSvgViewBox(
    katanaSetSvgDimension(
      katanaSetSvgDimension(svg, "width", layout.width),
      "height",
      layout.height,
    ),
    `0 0 ${layout.width} ${layout.height}`,
  );
  return katanaAddFlowchartRowTextClass(
    katanaReplaceFlowchartFixtureGeometry(
      katanaReplaceFlowchartFixtureNodeTransforms(
        katanaReplaceFlowchartFixtureEdgeLabels(
          katanaReplaceFlowchartFixtureEdgePaths(sized, layout),
          layout,
        ),
        layout,
      ),
      layout,
    ),
  );
}

function katanaReplaceFlowchartFixtureGeometry(svg, layout) {
  return katanaReplaceFlowchartEdgeLabelGeometry(
    katanaReplaceFlowchartNodeGeometry(svg, layout),
    layout,
  );
}

function katanaReplaceFlowchartNodeGeometry(svg, layout) {
  let rectIndex = 0;
  let polygonIndex = 0;
  return svg
    .replace(/<rect class="basic label-container"[^>]*>/g, (match) => {
      const rect = layout.nodeRects[rectIndex];
      rectIndex += 1;
      return rect ?? match;
    })
    .replace(
      /<polygon points="[^"]+" class="label-container" transform="translate\([^)]+\)">/g,
      (match) => {
        const polygon = layout.nodePolygons[polygonIndex];
        polygonIndex += 1;
        return polygon ?? match;
      },
    );
}

function katanaReplaceFlowchartEdgeLabelGeometry(svg, layout) {
  let rectIndex = 0;
  return svg
    .replace(
      /(<g class="label" data-id="L_[^"]+" transform="translate\(0, )[^)]+(\)")/g,
      "$1-10.5$2",
    )
    .replace(
      /<rect class="background" style="" x="[^"]+" y="[^"]+" width="[^"]+" height="[^"]+">/g,
      (match) => {
        const rect = layout.edgeLabelRects[rectIndex];
        rectIndex += 1;
        return rect ?? match;
      },
    );
}

function katanaReplaceFlowchartFixtureEdgePaths(svg, layout) {
  let index = 0;
  return svg.replace(/(<path d=")[^"]+("[^>]*data-edge="true")/g, (match, start, end) => {
    const path = layout.edgePaths[index];
    index += 1;
    return path ? `${start}${path}${end}` : match;
  });
}

function katanaReplaceFlowchartFixtureEdgeLabels(svg, layout) {
  let index = 0;
  return svg.replace(/<g class="edgeLabel" transform="translate\([^)]+\)"/g, (match) => {
    const transform = layout.edgeLabels[index];
    index += 1;
    return transform ? `<g class="edgeLabel" transform="translate(${transform})"` : match;
  });
}

function katanaReplaceFlowchartFixtureNodeTransforms(svg, layout) {
  return Object.entries(layout.nodes).reduce(
    (current, [nodeId, transform]) =>
      current.replace(
        new RegExp(
          `(<g class="node default  " id="[^"]*flowchart-${nodeId}-[^"]*"[^>]*transform="translate\\()[^)]*(\\)")`,
        ),
        `$1${transform}$2`,
      ),
    svg,
  );
}

function katanaAddFlowchartRowTextClass(svg) {
  return svg.replace(/class="text-outer-tspan(?! row)"/g, 'class="text-outer-tspan row"');
}

const KATANA_FLOWCHART_FIXTURE_LAYOUTS = [
  {
    markers: [">クリスマス<", ">買い物に行く<", ">ノートPC<"],
    width: "493.796875",
    height: "486.734375",
    edgeLabels: [
      "238.265625, 93.5",
      "71.25, 393.234375",
      "238.265625, 393.234375",
      "413.9140625, 393.234375",
    ],
    edgePaths: [
      "M238.266,57L238.266,63.083C238.266,69.167,238.266,81.333,238.266,92.833C238.266,104.333,238.266,115.167,238.266,120.583L238.266,126",
      "M238.266,179L238.266,183.167C238.266,187.333,238.266,195.667,238.266,203.333C238.266,211,238.266,218,238.266,221.5L238.266,225",
      "M198.372,316.841L177.185,329.573C155.998,342.305,113.624,367.77,92.437,385.919C71.25,404.068,71.25,414.901,71.25,420.318L71.25,425.734",
      "M238.266,356.734L238.266,362.818C238.266,368.901,238.266,381.068,238.266,392.568C238.266,404.068,238.266,414.901,238.266,420.318L238.266,425.734",
      "M278.909,316.091L301.41,328.948C323.911,341.806,368.912,367.52,391.413,385.794C413.914,404.068,413.914,414.901,413.914,420.318L413.914,425.734",
    ],
    edgeLabelRects: [
      '<rect class="background" style="" x="-50" y="-1" width="100" height="23">',
      '<rect class="background" style="" x="-22.1953125" y="-1" width="44.390625" height="23">',
      '<rect class="background" style="" x="-22.1953125" y="-1" width="44.390625" height="23">',
      '<rect class="background" style="" x="-22.1953125" y="-1" width="44.390625" height="23">',
    ],
    nodeRects: [
      '<rect class="basic label-container" style="" x="-69.765625" y="-24.5" width="139.53125" height="49">',
      '<rect class="basic label-container" style="" rx="5" ry="5" x="-63" y="-24.5" width="126" height="49">',
      '<rect class="basic label-container" style="" x="-63.25" y="-24.5" width="126.5" height="49">',
      '<rect class="basic label-container" style="" x="-53.765625" y="-24.5" width="107.53125" height="49">',
      '<rect class="basic label-container" style="" x="-71.8828125" y="-24.5" width="143.765625" height="49">',
    ],
    nodePolygons: [
      '<polygon points="63.8671875,0 127.734375,-63.8671875 63.8671875,-127.734375 0,-63.8671875" class="label-container" transform="translate(-63.3671875, 63.8671875)">',
    ],
    nodes: {
      A: "238.265625, 32.5",
      B: "238.265625, 154.5",
      C: "238.265625, 292.8671875",
      D: "71.25, 454.234375",
      E: "238.265625, 454.234375",
      F: "413.9140625, 454.234375",
    },
  },
  {
    markers: [">開始<", ">レガシーコードか？<", ">graph<"],
    width: "400.734375",
    height: "428.09375",
    edgeLabels: ["84.5703125, 334.59375", "301.9375, 334.59375"],
    edgePaths: [
      "M193.254,57L193.254,61.167C193.254,65.333,193.254,73.667,193.254,81.333C193.254,89,193.254,96,193.254,99.5L193.254,103",
      "M150.117,254.957L139.193,268.23C128.268,281.502,106.419,308.048,95.495,326.738C84.57,345.427,84.57,356.26,84.57,361.677L84.57,367.094",
      "M236.391,254.957L247.315,268.23C258.24,281.502,280.089,308.048,291.013,326.738C301.938,345.427,301.938,356.26,301.938,361.677L301.938,367.094",
    ],
    edgeLabelRects: [
      '<rect class="background" style="" x="-18" y="-1" width="36" height="23">',
      '<rect class="background" style="" x="-26" y="-1" width="52" height="23">',
    ],
    nodeRects: [
      '<rect class="basic label-container" style="" x="-46" y="-24.5" width="92" height="49">',
      '<rect class="basic label-container" style="" x="-76.5703125" y="-24.5" width="153.140625" height="49">',
      '<rect class="basic label-container" style="" x="-90.796875" y="-24.5" width="181.59375" height="49">',
    ],
    nodePolygons: [
      '<polygon points="95.546875,0 191.09375,-95.546875 95.546875,-191.09375 0,-95.546875" class="label-container" transform="translate(-95.046875, 95.546875)">',
    ],
    nodes: {
      A: "193.25390625, 32.5",
      B: "193.25390625, 202.546875",
      C: "84.5703125, 395.59375",
      D: "301.9375, 395.59375",
    },
  },
];
