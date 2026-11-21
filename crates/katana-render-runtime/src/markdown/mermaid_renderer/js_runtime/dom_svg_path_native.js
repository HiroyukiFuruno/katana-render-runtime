const KATANA_PATH_BOUNDS_CACHE_LIMIT = 256;
const KATANA_PATH_BOUNDS_CACHE_BYTES = 65536;
const KATANA_PATH_BOUNDS_KEY_BYTES = 8192;
const katanaPathBoundsCache = new Map();
let katanaPathBoundsCacheBytes = 0;

function katanaSvgParsedPathBox(pathData) {
  if (!pathData) return null;
  const native = katanaNativeSvgPathBox(pathData);
  if (native !== null) return native;
  const bounds = katanaParsedSerializedPathDataBox(pathData);
  if (!bounds) return null;
  return katanaBox(bounds[0], bounds[1], bounds[2] - bounds[0], bounds[3] - bounds[1]);
}

function katanaNativeSvgPathBox(pathData) {
  const measure = globalThis.__katanaSvgPathBounds;
  if (typeof measure !== "function") return null;
  const cached = katanaPathBoundsCache.get(pathData);
  if (cached !== undefined) return cached;
  const bounds = measure(pathData);
  if (bounds === null) return null;
  if (
    !bounds ||
    ![bounds.x, bounds.y, bounds.width, bounds.height].every(Number.isFinite) ||
    bounds.width < 0 ||
    bounds.height < 0
  )
    throw new Error("native SVG path bounds are invalid");
  const box = katanaBox(bounds.x, bounds.y, bounds.width, bounds.height);
  katanaRememberSvgPathBox(pathData, box);
  return box;
}

function katanaRememberSvgPathBox(pathData, box) {
  // UTF-16上限で数え、内部表現に依存せず保持量を抑える。
  const keyBytes = pathData.length * 2;
  if (keyBytes > KATANA_PATH_BOUNDS_KEY_BYTES) return;
  while (
    katanaPathBoundsCache.size >= KATANA_PATH_BOUNDS_CACHE_LIMIT ||
    katanaPathBoundsCacheBytes + keyBytes > KATANA_PATH_BOUNDS_CACHE_BYTES
  ) {
    const oldest = katanaPathBoundsCache.keys().next().value;
    katanaPathBoundsCache.delete(oldest);
    katanaPathBoundsCacheBytes -= oldest.length * 2;
  }
  katanaPathBoundsCache.set(pathData, box);
  katanaPathBoundsCacheBytes += keyBytes;
}
