/** @param {[number, number, number, number] | null} box @param {string} transform */
function katanaTransformSvgBounds(box, transform) {
  if (!box) return null;
  const translation = katanaSimpleSvgTranslation(transform);
  if (translation) {
    return [
      box[0] + translation[0],
      box[1] + translation[1],
      box[2] + translation[0],
      box[3] + translation[1],
    ];
  }
  const transformed = katanaTransformSvgBox(
    transform,
    box[0],
    box[1],
    box[2] - box[0],
    box[3] - box[1],
  );
  return [
    transformed.x,
    transformed.y,
    transformed.x + transformed.width,
    transformed.y + transformed.height,
  ];
}

/** @param {string} tag @param {string} name @returns {number | null} */
function katanaAttrNumber(tag, name) {
  const match = tag.match(new RegExp(`(?:^|\\s)${name}="([^"]+)"`));
  if (!match) {
    return null;
  }
  return katanaFiniteAttrNumber(Number(match[1]));
}

/** @param {number} value @returns {number | null} */
function katanaFiniteAttrNumber(value) {
  if (Number.isFinite(value)) {
    return value;
  }
  return null;
}

/** @param {string} tag @param {string} name @returns {string | null} */
function katanaAttrText(tag, name) {
  return tag.match(new RegExp(`(?:^|\\s)${name}="([^"]+)"`))?.[1] ?? null;
}
