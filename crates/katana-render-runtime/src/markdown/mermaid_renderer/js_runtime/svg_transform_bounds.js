/** @typedef {{ x: number, y: number, width: number, height: number }} KatanaSvgTransformedBox */

/**
 * @param {string} transform
 * @param {number} x
 * @param {number} y
 * @param {number} width
 * @param {number} height
 * @returns {KatanaSvgTransformedBox}
 */
function katanaTransformSvgBox(transform, x, y, width, height) {
  /** @type {typeof globalThis & { __katanaTransformSvgBox?: (transform: string, x: number, y: number, width: number, height: number) => KatanaSvgTransformedBox }} */
  const host = globalThis;
  const bridge = host.__katanaTransformSvgBox;
  if (typeof bridge !== "function") throw new Error("SVG transform bounds bridge is unavailable");
  return bridge(transform, x, y, width, height);
}

/** @param {string} transform @returns {[number, number] | null} */
function katanaSimpleSvgTranslation(transform) {
  const number = "[-+]?(?:\\d+(?:\\.\\d*)?|\\.\\d+)(?:[eE][-+]?\\d+)?";
  const match = transform.match(
    new RegExp(`^translate\\(\\s*(${number})(?:[,\\s]+(${number}))?\\s*\\)$`),
  );
  if (!match) return null;
  /** @type {[number, number]} */
  const values = [Number(match[1]), Number(match[2] ?? 0)];
  return values.every(Number.isFinite) ? values : null;
}
