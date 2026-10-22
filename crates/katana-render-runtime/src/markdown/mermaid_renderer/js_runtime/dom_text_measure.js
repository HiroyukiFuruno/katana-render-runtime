function katanaAnchoredTextNodeBox(node, text, x, y, parts) {
  const isSvgText = node?.namespaceURI === "http://www.w3.org/2000/svg";
  const font = isSvgText ? katanaResolvedNodeTextFont(node) : null;
  if (
    !parts?.length &&
    isSvgText &&
    katanaMermaidNativeAdvanceRuntime() &&
    typeof globalThis.__katanaMeasureSvgTextMetrics === "function"
  ) {
    const metrics = katanaNativeSvgRunMetrics(node, text, font);
    if (metrics !== null) {
      return katanaSvgRawMetricCell(metrics, x, y, katanaTextAnchor(node), font);
    }
  }
  const width = Math.max(1, katanaTextLineWidth(node, text, parts));
  const height = katanaLineHeight(node);
  const baselineOffset = font ? katanaSvgDominantBaselineOffset(font) : 0;
  return katanaBox(
    katanaAnchoredTextLeft(katanaTextAnchor(node), x, width),
    y + baselineOffset - height * katanaTextBoxBaselineRatio(),
    width,
    height,
  );
}

function katanaTextLineWidth(node, text, parts) {
  const runs = parts?.length ? parts : [{ node, text }];
  return runs
    .map((run) => {
      const width = run.width ?? katanaTextNodeWidth(run.node ?? node, run.text, run.font);
      return katanaRunLayoutAdvance(run, width);
    })
    .reduce((sum, width) => sum + width, 0);
}

function katanaAnchoredTextLeft(anchor, x, width) {
  return (KATANA_TEXT_ANCHORS[anchor] ?? KATANA_TEXT_ANCHORS.start)(x, width);
}

const KATANA_TEXT_ANCHORS = {
  end: (x, width) => x - width,
  middle: (x, width) => x - width / 2,
  start: (x) => x,
};

function katanaTextAnchor(node) {
  return String(katanaTextAnchorValues(node).find(Boolean) ?? "start");
}

function katanaTextAnchorValues(node) {
  return [
    node.getAttribute?.("text-anchor"),
    node.style?.getPropertyValue?.("text-anchor"),
    katanaCssComputedStyleValue(node, "text-anchor"),
    node.parentNode?.getAttribute?.("text-anchor"),
    katanaCssComputedStyleValue(node.parentNode, "text-anchor"),
  ];
}

function katanaLineHeight(node) {
  const fontSize = Number(String(katanaLineHeightFontSize(node)).replace("px", ""));
  const resolvedFontSize = katanaFiniteFontSize(fontSize);
  return (
    katanaExplicitHtmlLineHeight(node, resolvedFontSize) ??
    katanaBrowserTextBoxHeight(resolvedFontSize)
  );
}

function katanaExplicitHtmlLineHeight(node, fontSize) {
  if (node?.namespaceURI !== KATANA_HTML_NAMESPACE) {
    return null;
  }
  const raw = String(node.style?.getPropertyValue?.("line-height") ?? "").trim();
  const match = raw.match(/^(-?\d+(?:\.\d+)?)(px)?$/);
  if (!match) {
    return null;
  }
  const value = Number(match[1]);
  return match[2] === "px" ? value : value * fontSize;
}

function katanaBrowserTextBoxHeight(fontSize) {
  const measured = KATANA_TEXT_BOX_HEIGHTS[String(fontSize)];
  if (measured) {
    return measured;
  }
  return Math.max(12, Math.ceil(fontSize * 1.15));
}

const KATANA_TEXT_BOX_HEIGHTS = {
  10: 11,
  10.5: 12.250732421875,
  11: 12.25,
  12: 14,
  14: 16,
  16: 19,
  18: 21,
  20: 23,
};

function katanaLineHeightFontSize(node) {
  return String(katanaInheritedFontSize(node));
}

function katanaInheritedFontSize(node) {
  return katanaResolvedNodeTextFontSize(node);
}

function katanaFiniteFontSize(fontSize) {
  if (Number.isFinite(fontSize)) {
    return fontSize;
  }
  return 16;
}

function katanaTextNodeWidth(node, text, font) {
  const svgMetrics = katanaNativeSvgRunMetrics(node, text, font);
  if (svgMetrics !== null) return svgMetrics.advance;
  const nativeWidth = katanaNativeTextWidth(node, text, font);
  if (nativeWidth !== null) {
    return nativeWidth;
  }
  if (katanaIsC4Diagram()) {
    return katanaC4TextWidth(node, text);
  }
  return katanaTextWidth(text) * katanaTextWidthScale(node);
}

function katanaNativeTextWidth(node, text, font) {
  if (
    typeof globalThis.__katanaMeasureText !== "function" ||
    !katanaMermaidNativeAdvanceRuntime()
  ) {
    return null;
  }
  const resolved = font ?? katanaResolvedNodeTextFont(node);
  if (!katanaIsMermaidClassDiagram() && !resolved.nativeAdvanceEligible) {
    return null;
  }
  const measured = Number(
    globalThis.__katanaMeasureText(
      String(text),
      resolved.family,
      resolved.size,
      resolved.weight,
      resolved.italic,
    ),
  );
  return Number.isFinite(measured) && measured >= 0 ? measured : null;
}

function katanaMermaidNativeAdvanceRuntime() {
  const type = globalThis.__katanaMermaidDiagramType;
  return (
    typeof type === "string" &&
    type.length > 0 &&
    type !== "zenuml" &&
    globalThis.__katanaDrawioRequest === undefined
  );
}

function katanaC4TextWidth(node, text) {
  const chars = Array.from(katanaMeasuredTextValue(text));
  const bold = katanaHasBoldTextWeight(node);
  const asciiScale = bold ? KATANA_C4_BOLD_ASCII_WIDTH_SCALE : KATANA_C4_NORMAL_ASCII_WIDTH_SCALE;
  const width = chars.reduce(
    (total, char) =>
      total + katanaC4CharacterWidth(char) * (katanaIsWideTextChar(char) ? 1 : asciiScale),
    0,
  );
  return (width + katanaC4TextKerningWidth(chars)) * katanaTextWidthScale(node);
}

function katanaC4CharacterWidth(char) {
  if (katanaIsWideTextChar(char)) {
    return katanaCharacterWidth(char);
  }
  if (char.charCodeAt(0) === 160) {
    return KATANA_CANVAS_ASCII_TEXT_WIDTHS[" "];
  }
  return (
    KATANA_CANVAS_ASCII_TEXT_WIDTHS[char] ??
    KATANA_C4_CANVAS_EXTRA_ASCII_TEXT_WIDTHS[char] ??
    katanaAsciiCharacterWidth(char)
  );
}

function katanaC4TextKerningWidth(chars) {
  return chars
    .slice(0, -1)
    .map((char, index) => katanaC4KerningPairWidth(char, chars[index + 1]))
    .reduce((width, pairWidth) => width + pairWidth, 0);
}

function katanaC4KerningPairWidth(left, right) {
  if ([left, right].every(katanaIsWideTextChar)) {
    return KATANA_C4_WIDE_KERNING_PAIRS[`${left}${right}`] ?? 0;
  }
  return 0;
}

const KATANA_C4_WIDE_KERNING_PAIRS = {
  ナル: -0.796875,
  タベ: -0.46875,
  ベー: -0.625,
  ース: -1.783482142857143,
  スキ: -0.46875,
};

const KATANA_C4_NORMAL_ASCII_WIDTH_SCALE = 1;
const KATANA_C4_BOLD_ASCII_WIDTH_SCALE = 1.0848594580154938;

function katanaTextWidthScale(node) {
  const fontSize =
    katanaFiniteFontSize(Number(String(katanaLineHeightFontSize(node)).replace("px", ""))) / 16;
  return fontSize * katanaTextWeightScale(node);
}

function katanaTextBoxBaselineRatio() {
  if (katanaIsC4Diagram()) {
    return 0.8125;
  }
  return KATANA_TEXT_BOX_BASELINE_RATIOS[Number(katanaIsRequirementDiagram())]();
}

function katanaTextWeightScale(node) {
  return KATANA_TEXT_WEIGHT_SCALES[
    Number([katanaIsRequirementDiagram(), katanaHasBoldTextWeight(node)].every(Boolean))
  ]();
}

function katanaHasBoldTextWeight(node) {
  const descendantValue = katanaDescendantExplicitTextWeight(node);
  const inheritedValue = katanaTextWeightValues(node).find(katanaHasTextWeightValue);
  return katanaIsBoldTextWeight(descendantValue ?? inheritedValue);
}

function katanaDescendantExplicitTextWeight(node) {
  return (node?.children ?? [])
    .map((child) => katanaExplicitTextWeight(child) ?? katanaDescendantExplicitTextWeight(child))
    .find(katanaHasTextWeightValue);
}

function katanaExplicitTextWeight(node) {
  return [node?.style?.getPropertyValue?.("font-weight"), node?.getAttribute?.("font-weight")].find(
    katanaHasTextWeightValue,
  );
}

function katanaHasTextWeightValue(value) {
  return String(value ?? "").trim() !== "";
}

function katanaTextWeightValues(node) {
  if (!node) {
    return [];
  }
  return [
    node.style?.getPropertyValue?.("font-weight"),
    katanaCssComputedStyleValue(node, "font-weight"),
    node.getAttribute?.("font-weight"),
  ].concat(katanaTextWeightValues(node.parentNode));
}

function katanaIsBoldTextWeight(value) {
  const normalized = String(value ?? "")
    .trim()
    .toLowerCase();
  return normalized === "bold" || normalized === "bolder" || Number(normalized) >= 600;
}

function katanaTextWidth(text) {
  const chars = Array.from(katanaMeasuredTextValue(text));
  const characterWidth = chars
    .map((char) => katanaCharacterWidth(char))
    .reduce((width, charWidth) => width + charWidth, 0);
  return characterWidth + katanaTextKerningWidth(chars);
}

function katanaMeasuredTextValue(text) {
  return String(text);
}

function katanaTextKerningWidth(chars) {
  return chars
    .slice(0, -1)
    .map((char, index) => katanaKerningPairWidth(char, chars[index + 1]))
    .reduce((width, pairWidth) => width + pairWidth, 0);
}

function katanaKerningPairWidth(left, right) {
  return KATANA_KERNING_PAIR_WIDTH_READERS[Number(katanaHasWideKerningChar(left, right))](
    left,
    right,
  );
}

function katanaHasWideKerningChar(left, right) {
  return [left, right].some(katanaIsWideTextChar);
}

function katanaIsWideTextChar(char) {
  return char.charCodeAt(0) > 255;
}

function katanaCharacterWidth(char) {
  return KATANA_CHARACTER_WIDTHS[Number(char.charCodeAt(0) > 255)](char);
}

function katanaAsciiCharacterWidth(char) {
  if (char.charCodeAt(0) === 160) {
    return KATANA_ASCII_TEXT_WIDTHS[" "];
  }
  return [KATANA_ASCII_TEXT_WIDTHS[char]].filter(Boolean).concat([8])[0];
}

function katanaWideCharacterWidth(char) {
  if (globalThis.__katanaMermaidDiagramType === "kanban") {
    return 12.5;
  }
  if (String(globalThis.__katanaMermaidDiagramType).startsWith("statediagram")) {
    return 16;
  }
  if (katanaIsC4Diagram()) {
    return 16;
  }
  return katanaDefaultWideCharacterWidth(char);
}

function katanaIsC4Diagram() {
  return String(globalThis.__katanaMermaidDiagramType).startsWith("c4");
}

function katanaDefaultWideCharacterWidth(char) {
  if (katanaIsWidePunctuation(char)) {
    return 20;
  }
  return KATANA_DEFAULT_WIDE_CHARACTER_WIDTHS[Number(katanaIsCjkIdeograph(char))]();
}

function katanaIsCjkIdeograph(char) {
  const codePoint = katanaCharacterCodePoint(char);
  return KATANA_CJK_IDEOGRAPH_RULES.every((rule) => rule(codePoint));
}

function katanaCharacterCodePoint(char) {
  return char.codePointAt(0) ?? 0;
}

function katanaIsCjkIdeographStart(codePoint) {
  return codePoint >= 0x4e00;
}

function katanaIsCjkIdeographEnd(codePoint) {
  return codePoint <= 0x9fff;
}

function katanaIsWidePunctuation(char) {
  return ["。", "、"].includes(char);
}

const KATANA_DEFAULT_WIDE_CHARACTER_WIDTHS = [() => 15.8, () => 16.3];
const KATANA_CJK_IDEOGRAPH_RULES = [katanaIsCjkIdeographStart, katanaIsCjkIdeographEnd];

const KATANA_CHARACTER_WIDTHS = [katanaAsciiCharacterWidth, katanaWideCharacterWidth];

const KATANA_KERNING_PAIR_WIDTH_READERS = [
  (left, right) => KATANA_ASCII_KERNING_PAIRS[`${left}${right}`] ?? 0,
  () => 0,
];

const KATANA_TEXT_WEIGHT_SCALES = [() => 1, () => 1.055];

const KATANA_TEXT_BOX_BASELINE_RATIOS = [() => 0.8, () => 0.8421052631578947];

function katanaIsRequirementDiagram() {
  return ["requirement", "requirementdiagram"].includes(globalThis.__katanaMermaidDiagramType);
}

KatanaNode.prototype.getComputedTextLength = function getComputedTextLength() {
  let width;
  if (
    this.localName === "text" ||
    (this.localName === "tspan" &&
      !katanaIsMermaidClassDiagram() &&
      katanaMermaidNativeAdvanceRuntime() &&
      (!katanaIsBlockDiagram() || katanaCanMergeNativeTextOwners(this)))
  ) {
    const lines = katanaTextLines(this);
    width = Math.max(1, ...lines.map((line) => katanaTextLineWidth(this, line.text, line.parts)));
  } else {
    width = Math.max(1, katanaTextNodeWidth(this, katanaTextContent(this)));
  }
  return katanaNativeSvgComputedLength(this, width);
};

function katanaNativeSvgComputedLength(node, width) {
  if (node?.namespaceURI !== "http://www.w3.org/2000/svg" || !katanaMermaidNativeAdvanceRuntime()) {
    return width;
  }
  const resolved = katanaResolvedNodeTextFont(node);
  return resolved.nativeAdvanceEligible && resolved.size !== 0 ? katanaSvgAdvance(width) : width;
}

function katanaNativeSvgRunMetrics(node, text, font) {
  if (
    node?.namespaceURI !== "http://www.w3.org/2000/svg" ||
    !katanaMermaidNativeAdvanceRuntime() ||
    typeof globalThis.__katanaMeasureSvgTextMetrics !== "function"
  )
    return null;
  const resolved = font ?? katanaResolvedNodeTextFont(node);
  if (!resolved.nativeAdvanceEligible || resolved.size === 0) return null;
  const measured = globalThis.__katanaMeasureSvgTextMetrics(
    String(text),
    resolved.family,
    resolved.size,
    resolved.weight,
    resolved.italic,
  );
  const valid =
    measured &&
    [measured.advance, measured.ascent, measured.descent].every(Number.isFinite) &&
    (!Object.hasOwn(measured, "layoutAscent") ||
      (Number.isFinite(measured.layoutAscent) && measured.layoutAscent >= 0)) &&
    measured.advance >= 0 &&
    measured.ascent + measured.descent >= 0 &&
    (String(text).length === 0 || measured.ascent + measured.descent > 0);
  if (!valid) throw new Error("native SVG font metrics are invalid");
  const ink = [measured.inkLeft, measured.inkTop, measured.inkRight, measured.inkBottom];
  if (
    ink.some((value) => value !== undefined) &&
    (!ink.every(Number.isFinite) || ink[0] > ink[2] || ink[1] > ink[3])
  )
    throw new Error("native SVG glyph bounds are invalid");
  return measured;
}

function katanaSvgRawMetricCell(metrics, x, y, anchor, font) {
  // 字形が行領域や送り幅からはみ出す部分も、同じshape結果の境界として含める。
  const advance = katanaSvgAdvance(metrics.advance);
  const left = Math.min(0, metrics.inkLeft ?? 0);
  const right = Math.max(advance, metrics.inkRight ?? 0);
  const baselineOffset = katanaSvgDominantBaselineOffset(font);
  const hasLayoutAscent = Object.hasOwn(metrics, "layoutAscent");
  const layoutAscent = hasLayoutAscent ? metrics.layoutAscent : Math.round(metrics.ascent);
  if (!Number.isFinite(layoutAscent) || layoutAscent < 0) {
    throw new Error("native SVG layout ascent is invalid");
  }
  const top = Math.min(-layoutAscent, metrics.inkTop ?? 0);
  const bottom = Math.max(Math.round(metrics.descent), metrics.inkBottom ?? 0);
  return katanaBox(
    katanaAnchoredTextLeft(anchor, x, advance) + left,
    y + baselineOffset + top,
    right - left,
    bottom - top,
  );
}

function katanaSvgDominantBaselineOffset(font) {
  if (font?.dominantBaseline !== "middle" || font.size === 0) return 0;
  /* WHY: SVG 1.1のmiddle baselineをx-height中央へ置き、他の値には補正しない。
   * 仕様: https://www.w3.org/TR/SVG11/text.html#DominantBaselineProperty
   */
  const measure = globalThis.__katanaMeasureSvgFontXHeight;
  if (typeof measure !== "function") throw new Error("SVG font x-height bridge unavailable");
  const xHeight = measure(font.family, font.size, font.weight, font.italic);
  if (xHeight === undefined) return font.size * 0.25;
  if (!Number.isFinite(xHeight) || xHeight <= 0) {
    throw new Error("SVG font x-height metric is invalid");
  }
  return xHeight / 2;
}

function katanaSvgAdvance(value) {
  return Math.ceil(value * 64) / 64;
}

function katanaRunLayoutAdvance(run, width = run.width) {
  return run.svgMetrics ? katanaSvgAdvance(width) : width;
}

function katanaSvgRawPositionedRunCell(run, anchorOffset) {
  return katanaSvgRawMetricCell(run.svgMetrics, run.x + anchorOffset, run.y, "start", run.font);
}
