/** @param {string} tag @returns {[number, number]} */
function katanaTranslate(tag) {
  const match = tag.match(/transform="translate\(([-\d.]+)[,\s]+([-\d.]+)\)/);
  if (match) {
    return [Number(match[1]), Number(match[2])];
  }
  return [0, 0];
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
