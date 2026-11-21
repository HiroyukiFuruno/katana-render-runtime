function katanaNormalizeKanbanLayoutSvg(svg, request) {
  // vendor geometryを保持し、既存のlightテーマ配色だけを適用する。
  return katanaNormalizeKanbanReadableTextSvg(svg, request);
}

function katanaNormalizeKanbanReadableTextSvg(svg, request) {
  if (request.theme === "dark") {
    return svg;
  }
  return svg.replace("</style>", `${katanaKanbanReadableTextStyle(svg)}</style>`);
}

function katanaKanbanReadableTextStyle(svg) {
  const root = katanaKanbanRootSelector(svg);
  const selectors = [
    `${root} .cluster-label text`,
    `${root} .cluster-label tspan`,
    `${root} .label text`,
    `${root} .label tspan`,
    `${root} .kanban-ticket-link text`,
    `${root} .kanban-ticket-link tspan`,
  ];
  return `${selectors.join(",")}{fill:#333333!important;color:#333333!important;}`;
}

function katanaKanbanRootSelector(svg) {
  const id = svg.match(/^<svg\b[^>]*\sid="([^"]+)"/)?.[1];
  return id ? `#${id}` : "svg";
}
