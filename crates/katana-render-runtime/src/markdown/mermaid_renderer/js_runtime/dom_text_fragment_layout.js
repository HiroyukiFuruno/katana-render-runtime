function katanaC4FragmentRow(owner, font, state) {
  if (
    !katanaIsC4Diagram() ||
    !font.nativeAdvanceEligible ||
    owner?.localName !== "tspan" ||
    owner.namespaceURI !== "http://www.w3.org/2000/svg" ||
    typeof globalThis.__katanaMeasureSvgFragmentLayout !== "function"
  )
    return null;
  const row = owner.parentNode;
  if (row?.localName !== "tspan" || !katanaIsNativeTextOwnerDirectionSafe(owner)) return null;
  state.c4FragmentRows ??= new WeakMap();
  if (!state.c4FragmentRows.has(row))
    state.c4FragmentRows.set(
      row,
      katanaC4FragmentPositionSupported(row, font) &&
        katanaC4FragmentTransformSupported(row) &&
        katanaC4FragmentChildrenSupported(row, font, state.css),
    );
  return state.c4FragmentRows.get(row) ? row : null;
}

function katanaC4FragmentChildrenSupported(row, font, css) {
  const children = row.children ?? [];
  if (
    children.length < 2 ||
    katanaOwnText(row) !== "" ||
    (row.childNodes ?? []).some((child) => child.nodeType !== 1)
  )
    return false;
  const className = children[0]?.getAttribute?.("class");
  return children.every(
    (child) =>
      child.localName === "tspan" &&
      child.getAttribute("class") === className &&
      child.attributes.every((attribute) => attribute.name === "class") &&
      !(child.children ?? []).some((node) => node.nodeType === 1) &&
      katanaOwnText(child).length > 0 &&
      katanaIsNativeTextOwnerDirectionSafe(child) &&
      katanaC4FragmentTextStyleSupported(child) &&
      katanaSameTextRunFont(font, katanaResolveTextFont(child, font, css)),
  );
}

function katanaC4FragmentPositionSupported(row, font) {
  for (let node = row; node; node = node.parentNode) {
    if (
      ["textLength", "lengthAdjust", "rotate", "dx"].some((name) => {
        const value = node.getAttribute?.(name);
        return value !== null && value !== undefined;
      })
    )
      return false;
    if (!["text", "tspan"].includes(node.localName)) continue;
    for (const name of ["x", "y", "dy"]) {
      if (node.getAttribute(name) !== null && katanaRunTextPosition(node, name, font) === null)
        return false;
    }
  }
  return true;
}

function katanaC4FragmentTransformSupported(row) {
  let svgFound = false;
  for (let node = row; node; node = node.parentNode) {
    const transform = katanaTransformValue(node).trim();
    if (transform && katanaSimpleSvgTranslation(transform) === null) return false;
    const cssTransform = String(
      node.style?.getPropertyValue?.("transform") ||
        katanaCssComputedStyleValue(node, "transform") ||
        "",
    ).trim();
    if (cssTransform && cssTransform !== "none") return false;
    if (node.localName === "svg") {
      svgFound = true;
      if (node.getAttribute("viewBox") !== null) return false;
    }
  }
  return svgFound;
}

function katanaC4FragmentTextStyleSupported(node) {
  /** @type {Array<[string, RegExp]>} */
  const rules = [
    ["word-spacing", /^(|normal|0(?:px)?)$/i],
    ["text-decoration", /^(|none)$/i],
    ["font-kerning", /^(|auto|normal)$/i],
    ["font-stretch", /^(|normal|100%)$/i],
    ["font-variant", /^(|normal)$/i],
    ["font-variant-caps", /^(|normal)$/i],
    ["font-variant-ligatures", /^(|normal)$/i],
    ["font-feature-settings", /^(|normal)$/i],
    ["font-variation-settings", /^(|normal)$/i],
    ["letter-spacing", /^(|normal|0(?:px)?)$/i],
    ["writing-mode", /^(|horizontal-tb)$/i],
  ];
  for (let current = node; current; current = current.parentNode) {
    if (
      !rules.every(([name, accepted]) => {
        const values = [
          current.style?.getPropertyValue?.(name),
          current.getAttribute?.(name),
          katanaCssComputedStyleValue(current, name),
        ];
        return values.every((value) => accepted.test(String(value ?? "").trim()));
      })
    )
      return false;
  }
  return true;
}

function katanaC4FragmentRun(pending, text) {
  if (!pending.c4Row || !pending.svgNativeEligible) return null;
  const fragments = pending.fragments.map((fragment) => fragment.texts.join(""));
  const font = pending.font;
  const layout = globalThis.__katanaMeasureSvgFragmentLayout(
    fragments,
    font.family,
    font.size,
    font.weight,
    font.italic,
  );
  if (layout === null) return null;
  if (
    !layout ||
    !Array.isArray(layout.advances) ||
    layout.advances.length !== fragments.length ||
    ![layout.advance, layout.ascent, layout.descent].every(Number.isFinite) ||
    layout.advance < 0 ||
    layout.ascent + layout.descent <= 0 ||
    layout.advances.some((value) => !Number.isFinite(value) || value < 0) ||
    (Object.hasOwn(layout, "layoutAscent") &&
      (!Number.isFinite(layout.layoutAscent) || layout.layoutAscent < 0))
  )
    throw new Error("native SVG fragment layout is invalid");
  const ink = [layout.inkLeft, layout.inkTop, layout.inkRight, layout.inkBottom];
  if (
    ink.some((value) => value !== undefined) &&
    (!ink.every(Number.isFinite) ||
      ink[0] > ink[2] ||
      ink[1] > ink[3] ||
      ink[0] < 0 ||
      ink[2] > layout.advance)
  )
    throw new Error("native SVG fragment layout ink is invalid");
  const width = layout.advances.reduce((sum, advance) => sum + katanaSvgAdvance(advance), 0);
  if (!Number.isFinite(width) || width < layout.advance)
    throw new Error("native SVG fragment layout advance is inconsistent");
  return {
    node: pending.node,
    text,
    width,
    svgMetrics: { ...layout, advance: width },
    fragmentTexts: fragments,
    fragmentAdvances: layout.advances,
  };
}
