globalThis.__krrActualBBoxes = [];
globalThis.__krrActualBBoxTraceOverflow = false;
globalThis.__krrActualBBoxTraceFailure = null;
const nativeWidthBBoxTraceLimit = 64;
const nativeWidthSvgNamespace = "http://www.w3.org/2000/svg";
const svgProbe = document.createElementNS("http://www.w3.org/2000/svg", "svg");
const svgPrototypeCandidate = svgProbe && Object.getPrototypeOf(svgProbe);
if (!hasNativeWidthBBox(svgPrototypeCandidate))
  throw new Error("native width getBBox trace unavailable");
const originalGetBBox = svgPrototypeCandidate.getBBox;
/** @this {NativeWidthTraceNode} @param {...unknown} args @returns {NativeWidthTraceBox} */
svgPrototypeCandidate.getBBox = function (...args) {
  const box = originalGetBBox.apply(this, args);
  if (actualPhase) recordActualBBox(this, box);
  return box;
};

/** @param {NativeWidthTraceNode} node @returns {NativeWidthTraceNodeSnapshot} */
function nativeWidthNodeSnapshot(node) {
  return {
    localName: node.localName,
    namespaceURI: node.namespaceURI,
    id: node.getAttribute("id"),
    className: node.getAttribute("class"),
    text: String(node.textContent ?? ""),
    childSvgWidths: node.children
      .filter((child) => child.localName === "svg")
      .map((child) => child.getAttribute("width")),
  };
}

/** @param {unknown} value @returns {value is Pick<NativeWidthTraceNode, "getBBox">} */
function hasNativeWidthBBox(value) {
  return (
    typeof value === "object" &&
    value !== null &&
    "getBBox" in value &&
    typeof value.getBBox === "function"
  );
}

/** @param {NativeWidthTraceNode} node @param {NativeWidthTraceBox} box */
function recordActualBBox(node, box) {
  if (!nativeWidthIsArchitectureBBoxTarget(node)) return;
  if (globalThis.__krrActualBBoxes.length >= nativeWidthBBoxTraceLimit) {
    globalThis.__krrActualBBoxTraceOverflow = true;
    return;
  }
  try {
    globalThis.__krrActualBBoxes.push({
      node: nativeWidthNodeSnapshot(node),
      parent: node.parentNode ? nativeWidthNodeSnapshot(node.parentNode) : null,
      bbox: { x: box.x, y: box.y, width: box.width, height: box.height },
    });
  } catch (error) {
    globalThis.__krrActualBBoxTraceFailure = String(error);
  }
}

/** @param {NativeWidthTraceNode} node @returns {boolean} */
function nativeWidthIsArchitectureBBoxTarget(node) {
  if (node.namespaceURI !== nativeWidthSvgNamespace) return false;
  if (node.localName === "g" && nativeWidthHasArchitectureServiceClass(node)) return true;
  if (!["text", "tspan"].includes(node.localName) || !String(node.textContent ?? "").trim()) {
    return false;
  }
  for (let parent = node.parentNode; parent; parent = parent.parentNode) {
    if (nativeWidthHasArchitectureServiceClass(parent)) return true;
    if (parent.localName === "svg") return false;
  }
  return false;
}

/** @param {NativeWidthTraceNode} node @returns {boolean} */
function nativeWidthHasArchitectureServiceClass(node) {
  return (node.getAttribute("class") ?? "").split(/\s+/).includes("architecture-service");
}
