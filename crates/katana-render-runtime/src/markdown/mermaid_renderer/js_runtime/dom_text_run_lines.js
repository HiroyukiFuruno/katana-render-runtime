function katanaOrderedTextLines(node) {
  const context = katanaTextFontContext(node);
  const presence = new WeakMap();
  katanaTextRunPresence(node, presence);
  const state = { lines: [], current: null, pending: null, x: 0, y: 0, css: context.css, presence };
  katanaVisitTextRuns(node, state, context.inherited);
  katanaFlushOrderedTextRun(state);
  return state.lines.filter((line) => line.text);
}

function katanaTextRunChild(child) {
  return (
    child.nodeType === 3 ||
    (child.nodeType === 1 && !KATANA_HIDDEN_LAYOUT_TAGS.has(child.localName))
  );
}

function katanaTextRunPresence(node, presence) {
  let rendered = katanaOwnText(node).length > 0;
  for (const child of node.childNodes ?? []) {
    if (katanaTextRunChild(child)) rendered = katanaTextRunPresence(child, presence) || rendered;
  }
  presence.set(node, rendered);
  return rendered;
}

function katanaVisitTextRuns(node, state, inherited) {
  if (!state.presence.get(node)) return;
  const font = katanaResolveTextFont(node, inherited, state.css);
  if (["text", "tspan"].includes(node.localName)) katanaPositionTextRuns(node, state, font);
  const own = katanaOwnText(node);
  if (own) katanaAppendOrderedTextRun(node, own, font, state);
  for (const child of node.childNodes ?? []) {
    if (katanaTextRunChild(child)) katanaVisitTextRuns(child, state, font);
  }
}

function katanaPositionTextRuns(node, state, font) {
  const x = katanaRunTextPosition(node, "x", font);
  const y = katanaRunTextPosition(node, "y", font);
  const dx = katanaRunTextPosition(node, "dx", font);
  const dy = katanaRunTextPosition(node, "dy", font);
  if ([x, y, dx, dy].some((value) => value !== null)) katanaFlushOrderedTextRun(state);
  state.x = (x ?? state.x) + (dx ?? 0);
  state.y = (y ?? state.y) + (dy ?? 0);
  if (!state.current || x !== null || y !== null || (dy ?? 0) !== 0) {
    state.current = { node, text: "", parts: [], x: state.x, y: state.y, anchor: font.anchor };
    state.lines.push(state.current);
  }
}

function katanaRunTextPosition(node, name, font) {
  const value = String(node.getAttribute?.(name) ?? "").trim();
  const match = value.match(/^([+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:e[+-]?\d+)?)(px|em)?$/i);
  if (!match) return null;
  const pixels = Number(match[1]) * (match[2]?.toLowerCase() === "em" ? font.size : 1);
  return Number.isFinite(pixels) ? pixels : null;
}

function katanaSameTextRunFont(left, right) {
  const keys = ["family", "size", "weight", "italic", "dominantBaseline"];
  if (katanaMermaidNativeAdvanceRuntime()) {
    keys.push(
      "letterSpacing",
      "fontFeatureSettings",
      "fontVariationSettings",
      "nativeAdvanceEligible",
    );
  }
  return keys.every((key) => left[key] === right[key]);
}

function katanaAppendOrderedTextRun(node, text, font, state) {
  const owner = node.nodeType === 3 ? node.parentNode : node;
  const lastFragment = state.pending?.fragments.at(-1);
  let nativeWordBoundary = false;
  if (
    state.pending &&
    lastFragment?.node !== owner &&
    katanaIsMermaidClassDiagram() &&
    state.pending.svgNativeEligible &&
    font.nativeAdvanceEligible === true &&
    katanaCanMergeNativeTextOwners(state.pending) &&
    katanaCanMergeNativeTextOwners(owner) &&
    (/^\s/u.test(text) || /\s$/u.test(lastFragment.texts.join("")))
  )
    nativeWordBoundary = true;
  if (nativeWordBoundary) katanaFlushOrderedTextRun(state);
  if (
    state.pending &&
    (!katanaSameTextRunFont(state.pending.font, font) ||
      ((!katanaCanMergeNativeTextRunOwners(state.pending.node) ||
        !katanaCanMergeNativeTextRunOwners(owner)) &&
        state.pending.node !== owner))
  )
    katanaFlushOrderedTextRun(state);
  state.pending ??= { node: owner, texts: [], fragments: [], font, svgNativeEligible: true };
  state.pending.svgNativeEligible &&= font.nativeAdvanceEligible === true;
  state.pending.texts.push(text);
  const last = state.pending.fragments.at(-1);
  if (last?.node === owner) last.texts.push(text);
  else state.pending.fragments.push({ node: owner, texts: [text] });
}

/* WHY: 同じfontと位置chunkの文字を一度に測り、DOM境界だけでkerningを失わせない。 */
function katanaFlushOrderedTextRun(state) {
  const pending = state.pending;
  if (!pending) return;
  const text = pending.texts.join("");
  const runs = katanaMeasuredPendingTextRuns(pending, text);
  for (const run of runs) katanaAppendMeasuredTextRun(run, pending.font, state);
  state.pending = null;
}

function katanaMeasuredPendingTextRuns(pending, text) {
  if (pending.fragments.length === 1) {
    return [katanaMeasuredTextRun(pending.node, text, pending.font)];
  }
  const svgMetrics =
    pending.svgNativeEligible && katanaCanMergeNativeTextRunOwners(pending)
      ? katanaNativeSvgRunMetrics(pending.node, text, pending.font)
      : null;
  if (svgMetrics !== null)
    return [{ node: pending.node, text, width: svgMetrics.advance, svgMetrics }];
  const native = katanaCanMergeNativeTextOwners(pending)
    ? katanaNativeTextWidth(pending.node, text, pending.font)
    : null;
  return native !== null
    ? [{ node: pending.node, text, width: native }]
    : pending.fragments.map((run) => katanaFallbackTextFragment(run, pending.font));
}

function katanaCanMergeNativeTextRunOwners(nodeOrPending) {
  return (
    katanaCanMergeNativeTextOwners(nodeOrPending) ||
    katanaCanMergeNativeSvgTextOwners(nodeOrPending)
  );
}

/* WHY: C4の連続tspanはSVG shapingだけを共有し、fallbackのowner補正は共有しない。 */
function katanaCanMergeNativeSvgTextOwners(nodeOrPending) {
  if (
    !katanaIsC4Diagram() ||
    !katanaMermaidNativeAdvanceRuntime() ||
    typeof globalThis.__katanaMeasureSvgTextMetrics !== "function"
  )
    return false;
  if (Array.isArray(nodeOrPending?.fragments)) {
    const nodes = nodeOrPending.fragments.map((fragment) => fragment.node);
    return nodes.length > 1 && nodes.every(katanaIsNativeSvgTextOwnerMergeSafe);
  }
  return katanaIsNativeSvgTextOwnerMergeSafe(nodeOrPending);
}

function katanaIsNativeSvgTextOwnerMergeSafe(node) {
  if (node?.namespaceURI !== "http://www.w3.org/2000/svg") return false;
  for (const name of ["x", "y", "dx", "dy", "rotate", "textLength", "lengthAdjust"]) {
    if (String(node.getAttribute?.(name) ?? "").trim()) return false;
  }
  return katanaIsNativeTextOwnerDirectionSafe(node);
}

/* WHY: fallback/C4の幅補正はowner依存なので、Nativeで測れない文字を別ownerへ波及させない。 */
function katanaCanMergeNativeTextOwners(nodeOrPending) {
  if (
    katanaIsC4Diagram() ||
    katanaIsRequirementDiagram() ||
    typeof globalThis.__katanaMeasureText !== "function"
  )
    return false;
  if (katanaIsMermaidClassDiagram()) return true;
  if (!katanaIsBlockDiagram()) return false;
  const nodes = Array.isArray(nodeOrPending?.fragments)
    ? nodeOrPending.fragments.map((fragment) => fragment.node)
    : [nodeOrPending];
  return nodes.every(katanaIsBlockTextOwnerMergeSafe);
}

function katanaIsBlockDiagram() {
  const diagramType = String(globalThis.__katanaMermaidDiagramType ?? "")
    .trim()
    .toLowerCase();
  return diagramType === "block" || diagramType === "block-beta";
}

function katanaIsBlockTextOwnerMergeSafe(node) {
  return katanaIsNativeTextOwnerDirectionSafe(node);
}

function katanaIsNativeTextOwnerDirectionSafe(node) {
  for (let current = node; current; current = current.parentNode) {
    const direction = String(
      current.style?.getPropertyValue?.("direction") ||
        katanaCssComputedStyleValue(current, "direction") ||
        current.getAttribute?.("direction") ||
        "",
    )
      .trim()
      .toLowerCase();
    if (direction && direction !== "ltr") return false;
    const unicodeBidi = String(
      current.style?.getPropertyValue?.("unicode-bidi") ||
        katanaCssComputedStyleValue(current, "unicode-bidi") ||
        current.getAttribute?.("unicode-bidi") ||
        "",
    )
      .trim()
      .toLowerCase();
    if (unicodeBidi && unicodeBidi !== "normal") return false;
  }
  return true;
}

function katanaFallbackTextFragment(fragment, font) {
  const text = fragment.texts.join("");
  return katanaMeasuredTextRun(fragment.node, text, font);
}

function katanaAppendMeasuredTextRun(run, font, state) {
  state.current.text += run.text;
  state.current.parts.push({ ...run, font, x: state.x, y: state.y });
  state.x += katanaRunLayoutAdvance(run);
}

function katanaPositionedTextLineBox(line) {
  const end = line.parts.at(-1);
  const advance = end.x + katanaRunLayoutAdvance(end) - line.x;
  const left = katanaAnchoredTextLeft(line.anchor, line.x, advance);
  return katanaUnionBox(line.parts.map((run) => katanaPositionedTextRunBox(run, left - line.x)));
}

function katanaPositionedTextRunBox(run, offset) {
  if (run.svgMetrics) return katanaSvgRawPositionedRunCell(run, offset);
  const height =
    katanaExplicitHtmlLineHeight(run.node, run.font.size) ??
    katanaBrowserTextBoxHeight(run.font.size);
  const baselineOffset =
    run.node?.namespaceURI === "http://www.w3.org/2000/svg"
      ? katanaSvgDominantBaselineOffset(run.font)
      : 0;
  return katanaBox(
    run.x + offset,
    run.y + baselineOffset - height * katanaTextBoxBaselineRatio(),
    Math.max(1, run.width),
    height,
  );
}

function katanaMeasuredTextRun(node, text, font) {
  const svgMetrics = katanaNativeSvgRunMetrics(node, text, font);
  return svgMetrics !== null
    ? { node, text, width: svgMetrics.advance, svgMetrics }
    : { node, text, width: katanaTextNodeWidth(node, text, font) };
}
