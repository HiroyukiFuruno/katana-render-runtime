globalThis.__krrActualWidths = [];
globalThis.__krrProbeWidths = [];
globalThis.__krrBridgeCalls = 0;
let actualPhase = true;
const bridge = globalThis.__katanaMeasureText;
const canvasBridge = globalThis.__katanaMeasureCanvasText;
const svgBridge = globalThis.__katanaMeasureSvgTextMetrics;
globalThis.__katanaMeasureText = (...args) => {
  globalThis.__krrBridgeCalls += 1;
  const width = bridge(...args);
  if (Number.isFinite(width)) {
    const widths = actualPhase ? globalThis.__krrActualWidths : globalThis.__krrProbeWidths;
    widths.push({ kind: "css", args, width });
  }
  return width;
};

/**
 * @param {unknown[]} args
 * @returns {string[] | null | undefined}
 */
function canvasSegmentsForTrace(args) {
  if (args.length <= 5) return undefined;
  const candidate = args[5];
  if (!Array.isArray(candidate)) return null;
  /** @type {string[]} */
  const segments = [];
  for (const segment of candidate) {
    if (typeof segment !== "string") return null;
    segments.push(segment);
  }
  return segments;
}

globalThis.__katanaMeasureCanvasText = (...args) => {
  globalThis.__krrBridgeCalls += 1;
  const width = canvasBridge(...args);
  if (Number.isFinite(width)) {
    const widths = actualPhase ? globalThis.__krrActualWidths : globalThis.__krrProbeWidths;
    const segments = canvasSegmentsForTrace(args);
    widths.push(
      segments === undefined
        ? { kind: "canvas", args, width }
        : { kind: "canvas", args, width, segments },
    );
  }
  return width;
};

globalThis.__katanaMeasureSvgTextMetrics = (...args) => {
  globalThis.__krrBridgeCalls += 1;
  const metrics = svgBridge(...args);
  if (metrics && [metrics.advance, metrics.ascent, metrics.descent].every(Number.isFinite)) {
    const widths = actualPhase ? globalThis.__krrActualWidths : globalThis.__krrProbeWidths;
    widths.push({
      kind: "svg",
      args,
      width: metrics.advance,
      metrics: { advance: metrics.advance, ascent: metrics.ascent, descent: metrics.descent },
    });
  }
  return metrics;
};

/**
 * @param {string | undefined} name
 * @param {string | undefined} value
 * @returns {NativeWidthTraceNode}
 */
function propertyNode(name, value) {
  const node = document.createElementNS("http://www.w3.org/2000/svg", "text");
  node.textContent = "AV";
  node.style.setProperty(name, value);
  return node;
}

function nativePropertiesSkipped() {
  const controlStart = globalThis.__krrProbeWidths.length;
  propertyNode("letter-spacing", "normal").getBBox();
  const control = globalThis.__krrProbeWidths
    .slice(controlStart)
    .some((call) => call.kind === "svg");
  const before = globalThis.__krrBridgeCalls;
  for (const [name, value] of [
    ["letter-spacing", "2px"],
    ["font-feature-settings", '"kern" 0'],
    ["font-variation-settings", '"wdth" 75'],
  ]) {
    propertyNode(name, value).getBBox();
  }
  return control && globalThis.__krrBridgeCalls === before;
}

function inheritedCjkProbe() {
  const ns = "http://www.w3.org/2000/svg";
  const parent = document.createElementNS(ns, "g");
  setProbeFont(parent, "700");
  const child = document.createElementNS(ns, "text");
  child.textContent = "日本語";
  parent.appendChild(child);
  document.documentElement.appendChild(parent);
  const before = globalThis.__krrProbeWidths.length;
  const width = child.getBBox().width;
  const calls = globalThis.__krrProbeWidths.slice(before);
  // 既存のlength判定が1件を保証する枝だけで、標準配列のindex型を狭める。
  const args = calls.length === 1 ? /** @type {NativeWidthTraceCall} */ (calls[0]).args : [];
  return { kind: calls[0]?.kind, family: args[1], weight: args[3], text: args[0], width };
}

/** @param {NativeWidthTraceNode} node @param {string} [weight] */
function setProbeFont(node, weight = "400") {
  node.style.setProperty("font-family", "Noto Sans");
  node.style.setProperty("font-size", "16px");
  node.style.setProperty("font-weight", weight);
  node.style.setProperty("font-style", "normal");
}

/** @param {NativeWidthTraceNode} node */
function nativeProbeUsed(node) {
  const before = globalThis.__krrProbeWidths.length;
  node.getBBox();
  return globalThis.__krrProbeWidths.length > before;
}

function inheritedEligibilityProbe() {
  const parent = propertyNode("font-variation-settings", '"wdth" 75');
  setProbeFont(parent);
  parent.style.setProperty("letter-spacing", "2px");
  parent.style.setProperty("font-feature-settings", '"kern" 0');
  const child = propertyNode("letter-spacing", "inherit");
  parent.appendChild(child);
  const inherited = nativeProbeUsed(child);
  for (const name of ["letter-spacing", "font-feature-settings", "font-variation-settings"]) {
    child.style.setProperty(name, "initial");
  }
  return [inherited, nativeProbeUsed(child)];
}

function classRunProbeNode() {
  const node = document.createElementNS("http://www.w3.org/2000/svg", "text");
  setProbeFont(node);
  const first = document.createElementNS(node.namespaceURI, "tspan");
  first.textContent = "A";
  first.style.setProperty("letter-spacing", "2px");
  first.style.setProperty("font-feature-settings", '"kern" 0');
  first.style.setProperty("font-variation-settings", '"wdth" 75');
  const second = document.createElementNS(node.namespaceURI, "tspan");
  second.textContent = "V";
  node.appendChild(first);
  node.appendChild(second);
  return node;
}

/** @param {NativeWidthTraceNode} node */
function measuredProbeRuns(node) {
  const before = globalThis.__krrProbeWidths.length;
  node.getBBox();
  return globalThis.__krrProbeWidths
    .slice(before)
    .map((call) => ({ kind: call.kind, text: call.args[0] }));
}

function classCompatibilityProbe() {
  const saved = globalThis.__katanaMermaidDiagramType;
  globalThis.__katanaMermaidDiagramType = "classdiagram";
  const node = propertyNode("letter-spacing", "2px");
  setProbeFont(node);
  node.style.setProperty("font-feature-settings", '"kern" 0');
  node.style.setProperty("font-variation-settings", '"wdth" 75');
  const measured = node.getBBox().width;
  const expected = bridge("AV", "Noto Sans", 16, 400, false);
  const runs = classRunProbeNode();
  const classRuns = measuredProbeRuns(runs);
  globalThis.__katanaMermaidDiagramType = "flowchart";
  const nonclassRuns = measuredProbeRuns(runs);
  globalThis.__katanaMermaidDiagramType = saved;
  // WHY: 異なるadvance属性はclassでも別runとし、default runだけraw SVGへ渡す。
  return (
    Number.isFinite(measured) &&
    measured === expected &&
    JSON.stringify(classRuns) ===
      JSON.stringify([
        { kind: "css", text: "A" },
        { kind: "svg", text: "V" },
      ]) &&
    JSON.stringify(nonclassRuns) === JSON.stringify([{ kind: "svg", text: "V" }])
  );
}

function nativeWidthProbeResult() {
  // 実renderで設定されたtypeを使い、probeを実描画の呼出証拠へ混ぜない。
  actualPhase = false;
  return {
    noto: bridge("AV", "Noto Sans", 16, 400, false),
    inherited: inheritedCjkProbe(),
    skipped: nativePropertiesSkipped(),
    eligibility: inheritedEligibilityProbe(),
    classCompatible: classCompatibilityProbe(),
  };
}
