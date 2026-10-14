const KATANA_DRAWIO_SOURCE_PAINT_PADDING_LIMIT = 12;
function katanaDrawioSourceContentBox(svg) {
  return katanaDrawioSourceCropBox(svg, katanaDrawioSourceGeometryEntries());
}
function katanaDrawioSourcePaintBox(svg) {
  const entries = katanaDrawioSourceGeometryEntries();
  const offset = katanaDrawioSourceCropOffset(svg, entries);
  if (!offset) {
    return null;
  }
  const sourceBox = katanaDrawioSourceCropBox(svg, entries);
  const paintBox = katanaDrawioOutwardUnionBox(
    entries.map((entry) => katanaDrawioSourcePaintEntryBox(entry, offset)),
  );
  return [sourceBox, paintBox].every(Boolean)
    ? {
        x: katanaDrawioSourcePaintOrigin(sourceBox.x),
        y: sourceBox.y,
        width: paintBox.width,
        height: paintBox.height,
      }
    : null;
}
function katanaDrawioSourcePaintOrigin(value) {
  return value > 0 && value <= KATANA_DRAWIO_SOURCE_PAINT_PADDING_LIMIT ? 0 : value;
}
function katanaDrawioSourcePaintEntryBox(entry, offset) {
  const strokeRadius = katanaDrawioSourceGeometryStrokeWidth(entry) / 2;
  return {
    x: entry.x + offset.x - strokeRadius,
    y: entry.y + offset.y - strokeRadius,
    width: entry.width + strokeRadius * 2,
    height: entry.height + strokeRadius * 2,
  };
}
function katanaDrawioSourceGeometryStrokeWidth(entry) {
  const style = KATANA_DRAWIO_SOURCE_CELL_STYLE_CACHE.get(entry.id) ?? new Map();
  if (!katanaDrawioSourceGeometryHasStroke(style)) {
    return 0;
  }
  const width = Number(style.get("strokeWidth") ?? 1);
  return Number.isFinite(width) && width > 0 ? width : 1;
}
function katanaDrawioSourceGeometryHasStroke(style) {
  if (katanaDrawioColorKey(style.get("strokeColor")) === "none") {
    return false;
  }
  return [style.has("text"), style.get("shape") === "text"].some(Boolean)
    ? style.has("strokeColor")
    : true;
}
function katanaDrawioOutwardUnionBox(boxes) {
  if (boxes.length === 0) {
    return null;
  }
  const left = Math.floor(Math.min(...boxes.map((box) => box.x)));
  const top = Math.floor(Math.min(...boxes.map((box) => box.y)));
  const right = Math.ceil(Math.max(...boxes.map(katanaDrawioBoxRight)));
  const bottom = Math.ceil(Math.max(...boxes.map(katanaDrawioBoxBottom)));
  return { x: left, y: top, width: right - left, height: bottom - top };
}
function katanaDrawioSourceMinimumTop() {
  const entries = katanaDrawioSourceGeometryEntries();
  return entries.length === 0 ? Number.NaN : Math.min(...entries.map((entry) => entry.y));
}
function katanaDrawioSourceGeometryEntries() {
  return katanaDrawioAllSourceGeometryEntries().filter(katanaDrawioIsTopLevelSourceGeometryEntry);
}
function katanaDrawioAllSourceGeometryEntries() {
  return [
    ...katanaDrawioSourceCellGeometryEntries(),
    ...katanaDrawioSourceUserObjectGeometryEntries(),
  ].filter(katanaHasDrawioSourceGeometryEntry);
}
function katanaDrawioSourceVertexGeometryEntries() {
  return katanaDrawioSourceGeometryEntries().filter(katanaDrawioIsSourceVertexGeometryEntry);
}
function katanaDrawioIsSourceVertexGeometryEntry(entry) {
  return entry.vertex;
}
function katanaDrawioSourceCellGeometryEntries() {
  return Array.from(
    katanaDrawioRequestSource().matchAll(/<mxCell\b([^>]*)>\s*<mxGeometry\b([^>]*)/g),
  ).map(katanaDrawioSourceGeometryEntry);
}
function katanaDrawioSourceUserObjectGeometryEntries() {
  return Array.from(
    katanaDrawioRequestSource().matchAll(
      /<(?:UserObject|object)\b([^>]*)>\s*<mxCell\b([^>]*)>\s*<mxGeometry\b([^>]*)/g,
    ),
  ).map(katanaDrawioSourceUserObjectGeometryEntry);
}
function katanaDrawioSourceGeometryEntry(match) {
  const cellAttributes = katanaDrawioXmlAttributes(match[1]);
  const geometryAttributes = katanaDrawioXmlAttributes(match[2]);
  return katanaDrawioSourceGeometry(cellAttributes, geometryAttributes);
}
function katanaDrawioSourceUserObjectGeometryEntry(match) {
  const userObjectAttributes = katanaDrawioXmlAttributes(match[1]);
  const cellAttributes = katanaDrawioXmlAttributes(match[2]);
  const geometryAttributes = katanaDrawioXmlAttributes(match[3]);
  return katanaDrawioSourceGeometry(userObjectAttributes, geometryAttributes, cellAttributes);
}
function katanaDrawioSourceGeometry(cellAttributes, geometryAttributes, fallbackAttributes) {
  return {
    id: katanaDrawioCellAttribute(cellAttributes, "id"),
    parent: katanaDrawioSourceParentAttribute(cellAttributes, fallbackAttributes),
    vertex: katanaDrawioBooleanSourceAttribute(cellAttributes, fallbackAttributes, "vertex"),
    edge: katanaDrawioBooleanSourceAttribute(cellAttributes, fallbackAttributes, "edge"),
    relative: katanaDrawioCellAttribute(geometryAttributes, "relative") === "1",
    x: katanaDrawioCoordinateAttribute(geometryAttributes, "x"),
    y: katanaDrawioCoordinateAttribute(geometryAttributes, "y"),
    width: katanaDrawioRequiredNumberAttribute(geometryAttributes, "width"),
    height: katanaDrawioRequiredNumberAttribute(geometryAttributes, "height"),
  };
}
function katanaDrawioBooleanSourceAttribute(attributes, fallbackAttributes, name) {
  return [
    katanaDrawioCellAttribute(attributes, name),
    katanaDrawioOptionalCellAttribute(fallbackAttributes, name),
  ].includes("1");
}
function katanaDrawioSourceParentAttribute(cellAttributes, fallbackAttributes) {
  return [
    katanaDrawioOptionalCellAttribute(cellAttributes, "parent"),
    katanaDrawioOptionalCellAttribute(fallbackAttributes, "parent"),
  ]
    .filter(Boolean)
    .concat([""])[0];
}
function katanaDrawioOptionalCellAttribute(attributes, name) {
  return attributes ? katanaDrawioCellAttribute(attributes, name) : "";
}
function katanaHasDrawioSourceGeometryEntry(entry) {
  return [
    entry.id,
    Number.isFinite(entry.x),
    Number.isFinite(entry.y),
    entry.width > 0,
    entry.height > 0,
  ].every(Boolean);
}
function katanaDrawioIsTopLevelSourceGeometryEntry(entry) {
  return [
    katanaDrawioIsRootLevelSourceParent(entry.parent),
    katanaDrawioSourceCellParentMap().get(entry.parent) === "0",
  ].some(Boolean);
}
function katanaDrawioIsRootLevelSourceParent(parent) {
  return ["", "1"].includes(parent);
}
function katanaDrawioSourceCellParentMap() {
  return new Map(
    Array.from(katanaDrawioRequestSource().matchAll(/<mxCell\b([^>]*)/g))
      .map((match) => katanaDrawioXmlAttributes(match[1]))
      .filter(katanaDrawioHasSourceCellParent)
      .map(katanaDrawioSourceCellParentEntry),
  );
}
function katanaDrawioHasSourceCellParent(attributes) {
  return [attributes.has("id"), attributes.has("parent")].every(Boolean);
}
function katanaDrawioSourceCellParentEntry(attributes) {
  return [
    katanaDrawioCellAttribute(attributes, "id"),
    katanaDrawioCellAttribute(attributes, "parent"),
  ];
}
function katanaDrawioCoordinateAttribute(attributes, name) {
  return katanaDrawioOptionalNumberAttribute(attributes, name, 0);
}
function katanaDrawioRequiredNumberAttribute(attributes, name) {
  return katanaDrawioOptionalNumberAttribute(attributes, name, Number.NaN);
}
function katanaDrawioOptionalNumberAttribute(attributes, name, fallback) {
  const value = katanaDrawioCellAttribute(attributes, name);
  return value === "" ? fallback : Number(value);
}
function katanaDrawioSourceCropBox(svg, entries) {
  const sourceBox = katanaDrawioUnionBox(entries);
  const offset = katanaDrawioSourceCropOffset(svg, entries);
  return [sourceBox, offset].every(Boolean)
    ? katanaDrawioShiftedSourceCropBox(sourceBox, offset)
    : null;
}
function katanaDrawioSourceCropOffset(svg, entries) {
  return katanaDrawioMedianOffset(katanaDrawioSourceCropOffsets(svg, entries));
}
function katanaDrawioPreciseSourceCellOffset(svg, entry) {
  const group = katanaDrawioCellGroup(svg, entry.id);
  const box = group ? katanaDrawioPreciseCellShapeBox(group) : null;
  return box && katanaDrawioSimilarSourceBox(box, entry)
    ? { x: box.x - entry.x, y: box.y - entry.y }
    : null;
}
function katanaDrawioPreciseCellShapeBox(group) {
  const boxes = katanaDrawioCellShapeElements(group)
    .map(katanaDrawioElementBoxWithoutCrispTranslate)
    .filter(katanaDrawioHasArea);
  if (boxes.length === 0) {
    return null;
  }
  const left = Math.min(...boxes.map((box) => box.x));
  const top = Math.min(...boxes.map((box) => box.y));
  const right = Math.max(...boxes.map(katanaDrawioBoxRight));
  const bottom = Math.max(...boxes.map(katanaDrawioBoxBottom));
  return { x: left, y: top, width: right - left, height: bottom - top };
}
function katanaDrawioElementBoxWithoutCrispTranslate(element) {
  const box = katanaDrawioElementBox(element);
  const crisp = katanaDrawioElementAncestors(element)
    .map(katanaDrawioCrispTranslate)
    .reduce(katanaDrawioAddPoint, { x: 0, y: 0 });
  return { x: box.x - crisp.x, y: box.y - crisp.y, width: box.width, height: box.height };
}
function katanaDrawioCrispTranslate(node) {
  return String(node.getAttribute?.("transform") ?? "").replaceAll(" ", "") === "translate(0.5,0.5)"
    ? { x: 0.5, y: 0.5 }
    : { x: 0, y: 0 };
}
function katanaDrawioAddPoint(left, right) {
  return { x: left.x + right.x, y: left.y + right.y };
}
function katanaDrawioMeasuredSourceOrigin(svg, entries) {
  const offset = katanaDrawioSourceCropOffset(svg, entries);
  return offset ? { x: -offset.x, y: -offset.y } : null;
}
function katanaDrawioSourceCropOffsets(svg, entries) {
  return entries.map((entry) => katanaDrawioSourceCellOffset(svg, entry)).filter(Boolean);
}
function katanaDrawioSourceCellOffset(svg, entry) {
  const box = katanaDrawioMeasuredSourceCellBox(svg, entry);
  return box ? { x: box.x - entry.x, y: box.y - entry.y } : null;
}
function katanaDrawioMeasuredSourceCellBox(svg, entry) {
  return [katanaDrawioCellGroup(svg, entry.id)]
    .filter(Boolean)
    .map(katanaDrawioCellShapeBox)
    .filter(Boolean)
    .filter((box) => katanaDrawioSimilarSourceBox(box, entry))
    .concat([null])[0];
}
function katanaDrawioSimilarSourceBox(box, entry) {
  return [
    katanaDrawioSimilarSourceSize(box.width, entry.width),
    katanaDrawioSimilarSourceSize(box.height, entry.height),
  ].every(Boolean);
}
function katanaDrawioSimilarSourceSize(actual, expected) {
  return Math.abs(actual - expected) <= Math.max(4, expected * 0.02);
}
function katanaDrawioMedianOffset(offsets) {
  return offsets.length === 0
    ? null
    : {
        x: katanaDrawioMedianValue(offsets.map((offset) => offset.x)),
        y: katanaDrawioMedianValue(offsets.map((offset) => offset.y)),
      };
}
function katanaDrawioMedianValue(values) {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.floor(sorted.length / 2)];
}
function katanaDrawioShiftedSourceCropBox(box, offset) {
  const precise = [katanaDrawioSourceIsDeviceTemplate(), katanaDrawioIsGanttGridSource()].some(
    Boolean,
  );
  const preciseY = box.y + offset.y - Number(katanaDrawioIsGanttGridSource());
  return {
    x: precise ? box.x + offset.x : Math.floor(box.x + offset.x),
    y: precise ? preciseY : Math.floor(box.y + offset.y),
    width: Math.ceil(box.width),
    height: Math.ceil(box.height),
  };
}
function katanaDrawioIsGanttGridSource() {
  const source = katanaDrawioRequestSource();
  return [
    source.includes("shape=mxgraph.arrows.bent_right_arrow"),
    (source.match(/shape=mxgraph\.flowchart\.process/g) ?? []).length > 20,
  ].every(Boolean);
}
