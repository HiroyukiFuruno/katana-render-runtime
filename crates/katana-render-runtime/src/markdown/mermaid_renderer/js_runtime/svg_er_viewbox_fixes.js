function katanaNormalizeErViewBox(svg) {
  const viewBox = katanaReadViewBox(svg);
  if (!katanaShouldNormalizeErViewBox(viewBox)) {
    return svg;
  }
  const bounds = katanaErEntityBounds(svg);
  if (!bounds) {
    return svg;
  }
  return katanaSetNormalizedSvgSize(svg, [
    0,
    0,
    katanaFormatErNumber(viewBox[2] + 4.5),
    katanaFormatErNumber(bounds[3] + 8),
  ]);
}

function katanaShouldNormalizeErViewBox(viewBox) {
  return [
    viewBox,
    Math.abs(viewBox?.[0] + 8) < 0.01,
    Math.abs(viewBox?.[1] + 8) < 0.01,
    viewBox?.[2] > 0,
    viewBox?.[3] > 0,
  ].every(Boolean);
}

function katanaErEntityBounds(svg) {
  const boxes = katanaErEntityBoxes(svg);
  if (boxes.length === 0) {
    return null;
  }
  return [
    Math.min(...boxes.map((it) => it[0])),
    Math.min(...boxes.map((it) => it[1])),
    Math.max(...boxes.map((it) => it[2])),
    Math.max(...boxes.map((it) => it[3])),
  ];
}

function katanaErEntityBoxes(svg) {
  const boxes = [];
  katanaRewriteBalancedGroups(
    svg,
    /<g class="node default " id="[^"]*entity-[^"]*"/g,
    (node) => katanaCollectErEntityBox(node, boxes),
  );
  return boxes;
}

function katanaCollectErEntityBox(node, boxes) {
  const box = katanaErEntityBox(node);
  if (box) {
    boxes.push(box);
  }
  return node;
}

function katanaErEntityBox(node) {
  const origin = katanaErEntityOrigin(node);
  const localBox = katanaErEntityLocalBox(node);
  if (![origin, localBox].every(Boolean)) {
    return null;
  }
  return [
    origin[0] + localBox[0],
    origin[1] + localBox[1],
    origin[0] + localBox[2],
    origin[1] + localBox[3],
  ];
}

function katanaErEntityOrigin(node) {
  const match = node.match(/<g class="node default "[^>]*transform="translate\(([-\d.]+),\s*([-\d.]+)\)"/);
  if (!match) {
    return null;
  }
  return [Number(match[1]), Number(match[2])];
}

function katanaErEntityLocalBox(node) {
  return katanaErEntityOuterPathBox(node) ?? katanaErEntityRectBox(node);
}

function katanaErEntityOuterPathBox(node) {
  const match = node.match(
    /<path d="M([-\d.]+) ([-\d.]+) L([-\d.]+) [-\d.]+ L[-\d.]+ ([-\d.]+) L/,
  );
  if (!match) {
    return null;
  }
  return [Number(match[1]), Number(match[2]), Number(match[3]), Number(match[4])];
}

function katanaErEntityRectBox(node) {
  const match = node.match(
    /<rect class="basic label-container"[^>]*x="([-\d.]+)" y="([-\d.]+)" width="([-\d.]+)" height="([-\d.]+)"/,
  );
  if (!match) {
    return null;
  }
  const x = Number(match[1]);
  const y = Number(match[2]);
  return [x, y, x + Number(match[3]), y + Number(match[4])];
}
