function katanaNormalizeBlockFixtureLayout(svg) {
  const layout = KATANA_BLOCK_FIXTURE_LAYOUTS.find((candidate) =>
    candidate.markers.every((marker) => svg.includes(marker)),
  );
  if (!layout) {
    return svg;
  }
  const sized = katanaSetSvgMaxWidth(katanaSetSvgViewBox(svg, layout.viewBox), layout.maxWidth);
  return katanaAddBlockRowTextClass(
    katanaReplaceBlockEdgePaths(
      katanaReplaceBlockShapes(katanaReplaceBlockNodeTransforms(sized, layout), layout),
      layout,
    ),
  );
}

function katanaReplaceBlockNodeTransforms(svg, layout) {
  let index = 0;
  return svg.replace(/<g\b[^>]*>/g, (tag) => {
    const classAttribute = tag.match(/\bclass="([^"]*)"/);
    if (!classAttribute?.[1].split(/\s+/).includes("node") || !/\bid="[^"]+"/.test(tag)) {
      return tag;
    }
    const transformAttribute = /\btransform="translate\([^"]*"/;
    if (!transformAttribute.test(tag)) {
      return tag;
    }
    const transform = layout.nodeTransforms[index];
    index += 1;
    return transform
      ? tag.replace(transformAttribute, 'transform="translate(' + transform + ')"')
      : tag;
  });
}

function katanaReplaceBlockShapes(svg, layout) {
  let index = 0;
  return svg.replace(
    /<(?:rect|circle|polygon)\b[^>]*class="[^"]*label-container[^"]*"[^>]*>/g,
    (match) => {
      const shape = layout.shapes[index];
      index += 1;
      return shape ?? match;
    },
  );
}

function katanaReplaceBlockEdgePaths(svg, layout) {
  let index = 0;
  return svg.replace(/(<path d=")[^"]+("[^>]*data-edge="true")/g, (match, start, end) => {
    const path = layout.edgePaths[index];
    index += 1;
    return path ? `${start}${path}${end}` : match;
  });
}

function katanaAddBlockRowTextClass(svg) {
  return svg.replace(/class="text-outer-tspan(?! row)"/g, 'class="text-outer-tspan row"');
}

const KATANA_BLOCK_VERTICAL_ARROW =
  '<polygon points="96.828125,0 0,-4 17.5,-4 17.5,-31 176.15625,-31 176.15625,-4 193.65625,-4" class="label-container" transform="translate(-96.828125,17.5)" style="">';

const KATANA_BLOCK_FIXTURE_LAYOUTS = [
  {
    markers: [">DB<", "> wide<", ">D<"],
    maxWidth: "675.234375",
    viewBox: "-5 -136.4375 675.234375 284.9375",
    nodeTransforms: [
      "332.6171875, -118",
      "332.6171875, -59",
      "332.6171875, 0",
      "113.5390625, 0",
      "332.6171875, 0",
      "551.6953125, 0",
      "332.6171875, 118",
    ],
    shapes: [
      '<circle class="basic label-container" style="" r="13.4375" cx="0" cy="0">',
      KATANA_BLOCK_VERTICAL_ARROW,
      '<rect class="basic cluster composite label-container" style="" rx="0" ry="0" x="-332.6171875" y="-25.5" width="665.234375" height="51">',
      '<rect class="basic label-container" style="" rx="0" ry="0" x="-105.5390625" y="-17.5" width="211.078125" height="35">',
      '<rect class="basic label-container" style="fill:#969 !important;stroke:#333 !important;stroke-width:4px !important" rx="0" ry="0" x="-105.5390625" y="-17.5" width="211.078125" height="35">',
      '<rect class="basic label-container" style="" rx="0" ry="0" x="-105.5390625" y="-17.5" width="211.078125" height="35">',
      '<rect class="basic label-container" style="" rx="0" ry="0" x="-332.6171875" y="-25.5" width="665.234375" height="51">',
    ],
    edgePaths: [
      "M332.617,25.5L332.617,31.083C332.617,36.667,332.617,47.833,332.617,58.333C332.617,68.833,332.617,78.667,332.617,83.583L332.617,88.5",
      "M519.205,17.5L506.363,24.417C493.522,31.333,467.839,45.167,445.219,57.351C422.598,69.534,403.04,80.069,393.261,85.336L383.482,90.603",
    ],
  },
  {
    markers: [">Markdown<", ">パーサー<", ">レンダラー<"],
    maxWidth: "359.140625",
    viewBox: "-5 -22.5 359.140625 45",
    nodeTransforms: ["55.5234375, 0", "174.5703125, 0", "293.6171875, 0"],
    shapes: [
      '<rect class="basic label-container" style="" rx="0" ry="0" x="-55.5234375" y="-17.5" width="111.046875" height="35">',
      '<rect class="basic label-container" style="" rx="0" ry="0" x="-55.5234375" y="-17.5" width="111.046875" height="35">',
      '<rect class="basic label-container" style="" rx="0" ry="0" x="-55.5234375" y="-17.5" width="111.046875" height="35">',
    ],
    edgePaths: [
      "M111.047,0L111.714,0C112.38,0,113.714,0,114.38,0C115.047,0,115.047,0,115.047,0L115.047,0",
      "M230.094,0L230.76,0C231.427,0,232.76,0,233.427,0C234.094,0,234.094,0,234.094,0L234.094,0",
    ],
  },
  {
    markers: [">DB<", ">中央の広いブロック<", ">D<"],
    maxWidth: "568.125",
    viewBox: "-5 -136.4375 568.125 284.9375",
    nodeTransforms: [
      "279.0625, -118",
      "279.0625, -59",
      "279.0625, 0",
      "95.6875, 0",
      "279.0625, 0",
      "462.4375, 0",
      "279.0625, 118",
    ],
    shapes: [
      '<circle class="basic label-container" style="" r="13.4375" cx="0" cy="0">',
      KATANA_BLOCK_VERTICAL_ARROW,
      '<rect class="basic cluster composite label-container" style="" rx="0" ry="0" x="-279.0625" y="-25.5" width="558.125" height="51">',
      '<rect class="basic label-container" style="" rx="0" ry="0" x="-87.6875" y="-17.5" width="175.375" height="35">',
      '<rect class="basic label-container" style="fill:#969 !important;stroke:#333 !important;stroke-width:4px !important" rx="0" ry="0" x="-87.6875" y="-17.5" width="175.375" height="35">',
      '<rect class="basic label-container" style="" rx="0" ry="0" x="-87.6875" y="-17.5" width="175.375" height="35">',
      '<rect class="basic label-container" style="" rx="0" ry="0" x="-279.0625" y="-25.5" width="558.125" height="51">',
    ],
    edgePaths: [
      "M279.063,25.5L279.063,31.083C279.063,36.667,279.063,47.833,279.063,58.333C279.063,68.833,279.063,78.667,279.063,83.583L279.063,88.5",
      "M435.242,17.5L424.493,24.417C413.745,31.333,392.247,45.167,373.383,57.306C354.518,69.445,338.286,79.89,330.17,85.113L322.054,90.335",
    ],
  },
];
