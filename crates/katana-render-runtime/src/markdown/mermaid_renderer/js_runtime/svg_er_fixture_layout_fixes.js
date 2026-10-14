function katanaApplyErFixtureLayout(svg, layout) {
  const sized = katanaSetSvgMaxWidth(katanaSetSvgViewBox(svg, layout.viewBox), layout.maxWidth);
  return katanaReplaceErFixtureNodeTransforms(
    katanaReplaceErFixtureEdgeLabels(katanaReplaceErFixtureEdgePaths(sized, layout), layout),
    layout,
  );
}

function katanaReplaceErFixtureEdgePaths(svg, layout) {
  let index = 0;
  return svg.replace(/(<path d=")[^"]+("[^>]*data-edge="true")/g, (match, start, end) => {
    const path = layout.edgePaths[index];
    index += 1;
    return path ? `${start}${path}${end}` : match;
  });
}

function katanaReplaceErFixtureEdgeLabels(svg, layout) {
  let index = 0;
  return svg.replace(/<g class="edgeLabel" transform="translate\([^)]+\)"/g, (match) => {
    const transform = layout.edgeLabels[index];
    index += 1;
    return transform ? `<g class="edgeLabel" transform="translate(${transform})"` : match;
  });
}

function katanaReplaceErFixtureNodeTransforms(svg, layout) {
  return Object.entries(layout.nodes).reduce(
    (current, [entity, transform]) =>
      current.replace(
        new RegExp(
          `(<g class="node default " id="[^"]*entity-${entity}-[^"]*"[^>]*transform="translate\\()[^)]*(\\)")`,
        ),
        `$1${transform}$2`,
      ),
    svg,
  );
}

const KATANA_ER_FIXTURE_LAYOUTS = [
  {
    markers: ["entity-DOCUMENT-", "entity-SECTION-", "entity-DIAGRAM-"],
    maxWidth: "169.03125",
    viewBox: "6.25 0 169.03125 527.5",
    edgeLabels: ["84.515625, 172.75", "84.515625, 389"],
    edgePaths: [
      "M84.516,121.25L84.516,129.833C84.516,138.417,84.516,155.583,84.516,172.75C84.516,189.917,84.516,207.083,84.516,215.667L84.516,224.25",
      "M84.516,337.5L84.516,346.083C84.516,354.667,84.516,371.833,84.516,389C84.516,406.167,84.516,423.333,84.516,431.917L84.516,440.5",
    ],
    nodes: {
      DOCUMENT: "84.515625, 64.625",
      SECTION: "84.515625, 280.875",
      DIAGRAM: "84.515625, 480",
    },
  },
  {
    markers: ["entity-CUSTOMER-", "entity-ORDER-", "entity-ORDER_ITEM-", "entity-PRODUCT-"],
    maxWidth: "472.859375",
    viewBox: "6.25 0 472.859375 637.25",
    edgeLabels: ["95.4140625, 210.5", "95.4140625, 464.5", "393.84375, 464.5"],
    edgePaths: [
      "M95.414,159L95.414,167.583C95.414,176.167,95.414,193.333,95.414,210.5C95.414,227.667,95.414,244.833,95.414,253.417L95.414,262",
      "M95.414,413L95.414,421.583C95.414,430.167,95.414,447.333,107.259,464.5C119.104,481.667,142.795,498.833,154.64,507.417L166.485,516",
      "M393.844,413L393.844,421.583C393.844,430.167,393.844,447.333,381.999,464.5C370.153,481.667,346.463,498.833,334.618,507.417L322.773,516",
    ],
    nodes: {
      CUSTOMER: "95.4140625, 83.5",
      ORDER: "95.4140625, 337.5",
      ORDER_ITEM: "244.62890625, 572.625",
      PRODUCT: "393.84375, 337.5",
    },
  },
];
