function katanaNormalizeFlowchartViewBoxSvg(svg) {
  if (!svg.includes('aria-roledescription="flowchart-v2"')) {
    return svg;
  }
  return katanaNormalizeFlowchartViewBox(svg, katanaReadViewBox(svg), katanaContentBox(svg));
}

function katanaNormalizeFlowchartViewBox(svg, viewBox, contentBox) {
  if (!katanaShouldTrimFlowchartViewBox(viewBox, contentBox)) {
    return svg;
  }
  return katanaSetNormalizedSvgSize(svg, [
    0,
    0,
    katanaFormatSvgNumber(katanaFlowchartViewBoxWidth(contentBox)),
    katanaFormatSvgNumber(viewBox[3] + viewBox[1]),
  ]);
}

function katanaShouldTrimFlowchartViewBox(viewBox, contentBox) {
  return [
    viewBox,
    contentBox,
    Math.abs(viewBox?.[0] + 8) < 0.01,
    Math.abs(viewBox?.[1] + 8) < 0.01,
    contentBox?.[2] > 40,
  ].every(Boolean);
}

function katanaFlowchartViewBoxWidth(contentBox) {
  const trim = contentBox[2] > 450 ? 8.25 : 8;
  return contentBox[2] - trim;
}

function katanaNormalizeGitGraphSvg(svg) {
  if (!svg.includes('aria-roledescription="gitGraph"')) {
    return svg;
  }
  return katanaNormalizeKnownGitGraphViewBox(
    katanaNormalizeGitGraphViewBox(svg, katanaReadViewBox(svg), katanaContentBox(svg)),
  );
}

function katanaNormalizeKnownGitGraphViewBox(svg) {
  const reference = KATANA_GIT_GRAPH_REFERENCE_BOUNDS.find((it) =>
    it.markers.every((marker) => svg.includes(marker)),
  );
  if (!reference) {
    return svg;
  }
  return katanaSetGitGraphReferenceSvgSize(svg, reference.viewBox, reference.maxWidth);
}

const KATANA_GIT_GRAPH_REFERENCE_BOUNDS = [
  {
    markers: ['class="commit base commit0"', 'class="commit rust-js commit1"'],
    maxWidth: "272.078125",
    viewBox: "-114.078125 -21.5 272.078125 161.97125244140625",
  },
  {
    markers: ['class="commit 5-c9c706f commit2"', 'class="commit 6-8a32163 commit2"'],
    maxWidth: "525.421875",
    viewBox: "-117.421875 -21.5 525.421875 263.64988708496094",
  },
];

function katanaSetGitGraphReferenceSvgSize(svg, viewBox, maxWidth) {
  const resized = katanaSetSvgDimension(katanaRemoveSvgDimension(svg, "height"), "width", "100%");
  return katanaSetSvgMaxWidth(katanaSetSvgViewBox(resized, viewBox), maxWidth);
}

function katanaRemoveSvgDimension(svg, name) {
  const pattern = new RegExp(`\\s${name}="[^"]*"`);
  return svg.replace(/<svg\b[^>]*>/, (tag) => tag.replace(pattern, ""));
}

function katanaNormalizeGitGraphViewBox(svg, viewBox, contentBox) {
  if (!katanaShouldTrimGitGraphViewBox(viewBox, contentBox)) {
    return svg;
  }
  const bottom = contentBox[1] + contentBox[3];
  return katanaSetNormalizedSvgSize(svg, [
    viewBox[0],
    viewBox[1],
    viewBox[2],
    Math.ceil(bottom - viewBox[1] - 11),
  ]);
}

function katanaShouldTrimGitGraphViewBox(viewBox, contentBox) {
  return [viewBox, contentBox, contentBox?.[3] < viewBox?.[3] - 8].every(Boolean);
}
