function katanaContentBox(svg) {
  const drawable = svg
    .replace(/<style[\s\S]*?<\/style>/g, "")
    .replace(/<defs[\s\S]*?<\/defs>/g, "")
    .replace(/<marker[\s\S]*?<\/marker>/g, "");
  const boxes = katanaScannedBoxes(drawable).filter(katanaHasDrawableBoxArea);
  if (boxes.length === 0) {
    return null;
  }
  return katanaPaddedContentBox(boxes, 12);
}

function katanaHasDrawableBoxArea(box) {
  return [box[2] > box[0], box[3] > box[1]].every(Boolean);
}

function katanaPaddedContentBox(boxes, padding) {
  const minX = Math.min(...boxes.map((box) => box[0]));
  const minY = Math.min(...boxes.map((box) => box[1]));
  const maxX = Math.max(...boxes.map((box) => box[2]));
  const maxY = Math.max(...boxes.map((box) => box[3]));
  return katanaContentBoxWithPadding(minX, minY, maxX, maxY, padding);
}

function katanaContentBoxWithPadding(minX, minY, maxX, maxY, padding) {
  return [
    Math.floor(minX - padding),
    Math.floor(minY - padding),
    Math.ceil(maxX - minX + padding * 2),
    Math.ceil(maxY - minY + padding * 2),
  ];
}

function katanaScannedBoxes(svg) {
  const boxes = [];
  const transforms = [""];
  const tags = svg.matchAll(/<\/?([a-zA-Z][\w:-]*)([^>]*)>/g);
  for (const match of tags) {
    katanaScanSvgTag(svg, match, transforms, boxes);
  }
  return boxes;
}

function katanaScanSvgTag(svg, match, transforms, boxes) {
  const tag = katanaScannedTag(match);
  if (tag.fullTag.startsWith("</")) {
    katanaPopScannedTransform(tag.name, transforms);
    return;
  }
  katanaScanOpeningTag(svg, tag, transforms, boxes, match.index);
}

function katanaScannedTag(match) {
  return { fullTag: match[0], name: match[1].toLowerCase(), attributes: match[2] ?? "" };
}

function katanaPopScannedTransform(tagName, transforms) {
  if (katanaIsOffsetContainer(tagName)) {
    transforms.pop();
  }
}

function katanaScanOpeningTag(svg, tag, transforms, boxes, index) {
  const transform = katanaNestedTransform(transforms[transforms.length - 1] ?? "", tag.attributes);
  katanaPushOptionalBox(boxes, katanaOpeningTextBox(svg, tag, transform, index));
  katanaPushOptionalBox(boxes, katanaTagBox(tag.name, tag.attributes, transform));
  katanaPushScannedTransform(tag.name, transforms, transform);
}

function katanaNestedTransform(parentTransform, attributes) {
  const ownTransform = katanaAttrText(attributes, "transform") ?? "";
  if (!parentTransform || !ownTransform) return parentTransform || ownTransform;
  const parentTranslation = katanaSimpleSvgTranslation(parentTransform);
  const ownTranslation = katanaSimpleSvgTranslation(ownTransform);
  if (parentTranslation && ownTranslation) {
    return `translate(${parentTranslation[0] + ownTranslation[0]} ${parentTranslation[1] + ownTranslation[1]})`;
  }
  return `${parentTransform} ${ownTransform}`;
}

function katanaOpeningTextBox(svg, tag, transform, index) {
  if (tag.name === "text") {
    const box = katanaSvgTextBox(svg, index + tag.fullTag.length, tag.attributes, [0, 0]);
    return katanaTransformBounds(box, transform);
  }
  return null;
}

function katanaPushOptionalBox(boxes, box) {
  if (box) {
    boxes.push(box);
  }
}

function katanaPushScannedTransform(tagName, transforms, transform) {
  if (katanaIsOffsetContainer(tagName)) {
    transforms.push(transform);
  }
}

function katanaIsOffsetContainer(tagName) {
  return ["g", "svg"].includes(tagName);
}

function katanaTagBox(tagName, tag, transform) {
  const box = (KATANA_TAG_BOXES[tagName] ?? katanaNullBox)(tag, [0, 0]);
  return katanaTransformBounds(box, transform);
}

function katanaTransformBounds(box, transform) {
  if (!box || !transform) return box;
  return katanaTransformSvgBounds(box, transform);
}

const KATANA_TAG_BOXES = {
  circle: katanaSvgCircleBox,
  ellipse: katanaSvgCircleBox,
  image: katanaSvgRectBox,
  line: katanaSvgLineBox,
  path: katanaSerializedSvgPathBox,
  polygon: katanaSvgPolygonBox,
  polyline: katanaSvgPolygonBox,
  rect: katanaSvgRectBox,
};

function katanaNullBox() {
  return null;
}

function katanaSvgRectBox(tag, offset) {
  const x = katanaAttrNumberOrDefault(tag, "x", 0);
  const y = katanaAttrNumberOrDefault(tag, "y", 0);
  const width = katanaAttrNumberOrDefault(tag, "width", 0);
  const height = katanaAttrNumberOrDefault(tag, "height", 0);
  if (katanaPositiveSize(width, height)) {
    return katanaOffsetBox([x, y, x + width, y + height], offset);
  }
  return null;
}

function katanaAttrNumberOrDefault(tag, name, fallback) {
  return katanaAttrNumber(tag, name) ?? fallback;
}

function katanaPositiveSize(width, height) {
  return [width > 0, height > 0].every(Boolean);
}

function katanaSvgLineBox(tag, offset) {
  const x1 = katanaAttrNumber(tag, "x1");
  const y1 = katanaAttrNumber(tag, "y1");
  const x2 = katanaAttrNumber(tag, "x2");
  const y2 = katanaAttrNumber(tag, "y2");
  if ([x1, y1, x2, y2].some((value) => value === null)) {
    return null;
  }
  return katanaOffsetBox(
    [Math.min(x1, x2), Math.min(y1, y2), Math.max(x1, x2), Math.max(y1, y2)],
    offset,
  );
}

function katanaSvgCircleBox(tag, offset) {
  const cx = katanaAttrNumberOrDefault(tag, "cx", 0);
  const cy = katanaAttrNumberOrDefault(tag, "cy", 0);
  const rx = katanaRadiusAttrNumber(tag, "rx");
  const ry = katanaRadiusAttrNumber(tag, "ry");
  if (katanaPositiveSize(rx, ry)) {
    return katanaOffsetBox([cx - rx, cy - ry, cx + rx, cy + ry], offset);
  }
  return null;
}

function katanaRadiusAttrNumber(tag, name) {
  return katanaAttrNumber(tag, name) ?? katanaAttrNumberOrDefault(tag, "r", 0);
}

function katanaSvgPolygonBox(tag, offset) {
  const points = katanaAttrText(tag, "points");
  if (!points) {
    return null;
  }
  return katanaOffsetNumberListBox(points, offset);
}

function katanaOffsetBox(box, offset) {
  if (!box) {
    return null;
  }
  return [box[0] + offset[0], box[1] + offset[1], box[2] + offset[0], box[3] + offset[1]];
}
