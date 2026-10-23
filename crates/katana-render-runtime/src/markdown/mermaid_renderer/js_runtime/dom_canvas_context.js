class KatanaCanvasContext {
  constructor() {
    this.font = "";
  }

  measureText(value) {
    return { width: katanaCanvasTextWidth(String(value ?? ""), this.font) };
  }
}

KatanaNode.prototype.getContext = function getContext() {
  return new KatanaCanvasContext();
};

function katanaCanvasTextWidth(text, font) {
  const nativeWidth = katanaCanvasNativeTextWidth(text, font);
  if (nativeWidth !== null) {
    return nativeWidth;
  }
  const fontScale = katanaCanvasFontScale(font);
  const chars = Array.from(katanaMeasuredTextValue(text));
  return chars
    .map((char) => katanaCanvasCharacterWidth(char, font))
    .reduce((width, charWidth) => width + charWidth * fontScale, 0);
}

function katanaCanvasNativeTextWidth(text, font) {
  if (
    typeof katanaMermaidNativeAdvanceRuntime !== "function" ||
    !katanaMermaidNativeAdvanceRuntime() ||
    /* C4は既存のASCII補正と幅表を使うため、native測定を適用しない。 */
    (typeof katanaIsC4Diagram === "function" && katanaIsC4Diagram())
  ) {
    return null;
  }
  const resolved = katanaCanvasFontMetrics(font);
  if (!resolved) {
    return null;
  }
  const value = String(text);
  const canvasMeasure = globalThis.__katanaMeasureCanvasText;
  const useCanvas = typeof canvasMeasure === "function";
  const segmented = useCanvas && /\P{ASCII}/u.test(value);
  const measure = useCanvas ? canvasMeasure : globalThis.__katanaMeasureText;
  if (typeof measure !== "function") {
    return null;
  }
  const segments = segmented ? katanaCanvasTextSegments(value) : undefined;
  const measured = Number(
    measure(
      value,
      resolved.family,
      resolved.size,
      resolved.weight,
      resolved.italic,
      ...(segments ? [segments] : []),
    ),
  );
  if (!Number.isFinite(measured) || measured < 0) {
    throw new Error("native Canvas font measurement is invalid");
  }
  return measured;
}

const KATANA_CANVAS_CJK_RE =
  /[\p{Script_Extensions=Han}\p{Script_Extensions=Hiragana}\p{Script_Extensions=Katakana}\p{Script_Extensions=Bopomofo}]/u;
const KATANA_CANVAS_COMMON_RE = /[\p{Script=Common}\p{Script=Inherited}]/u;
const KATANA_CANVAS_MARK_RE = /[\p{Mark}\p{Modifier_Letter}\p{Modifier_Symbol}]/u;
const KATANA_CANVAS_EMOJI_RE = /[\p{Emoji}\p{Extended_Pictographic}]/u;
const KATANA_CANVAS_EMOJI_COMPONENT_RE = /\p{Emoji_Component}/u;
const KATANA_CANVAS_PICTOGRAPHIC_RE = /\p{Extended_Pictographic}/u;
const KATANA_CANVAS_CJK_MIN_CODE_POINT = 0x2c7;

function katanaCanvasTextSegments(text) {
  // Canvas の CJK 単位の文字組みを、通常の HTML 全文計測へ混入させない。
  const chars = Array.from(text);
  const segments = [];
  for (let start = 0; start < chars.length; ) {
    const end = katanaCanvasNextWordEnd(chars, start);
    segments.push(chars.slice(start, end).join(""));
    start = end;
  }
  return segments;
}

function katanaCanvasNextWordEnd(chars, start) {
  const first = chars[start];
  if (katanaCanvasIsDelimiter(first)) return start + 1;
  if (!katanaCanvasIsCjkOrSymbol(first)) {
    return katanaCanvasNonCjkWordEnd(chars, start + 1);
  }
  return katanaCanvasCjkWordEnd(chars, start + 1, !KATANA_CANVAS_COMMON_RE.test(first));
}

function katanaCanvasNonCjkWordEnd(chars, end) {
  for (; end < chars.length; end += 1) {
    const char = chars[end];
    if (
      katanaCanvasIsDelimiter(char) ||
      (katanaCanvasIsCjkOrSymbol(char) && !KATANA_CANVAS_MARK_RE.test(char))
    ) {
      return end;
    }
  }
  return end;
}

function katanaCanvasCjkWordEnd(chars, end, hasScript) {
  for (; end < chars.length; end += 1) {
    const char = chars[end];
    if (katanaCanvasCjkContinuation(char)) continue;
    if (!katanaCanvasIsCjkOrSymbol(char)) return end;
    if (KATANA_CANVAS_COMMON_RE.test(char)) continue;
    if (hasScript) return end;
    hasScript = true;
  }
  return end;
}

function katanaCanvasCjkContinuation(char) {
  return (
    KATANA_CANVAS_MARK_RE.test(char) ||
    char === "\u200d" ||
    KATANA_CANVAS_EMOJI_COMPONENT_RE.test(char) ||
    KATANA_CANVAS_PICTOGRAPHIC_RE.test(char)
  );
}

function katanaCanvasIsCjkOrSymbol(char) {
  return (
    char.codePointAt(0) >= KATANA_CANVAS_CJK_MIN_CODE_POINT &&
    (KATANA_CANVAS_CJK_RE.test(char) || KATANA_CANVAS_EMOJI_RE.test(char))
  );
}

function katanaCanvasIsDelimiter(char) {
  return char === " " || char === "\t" || char === "\u200b";
}

function katanaCanvasFontMetrics(font) {
  const value = String(font ?? "").trim();
  const sizeMatch = value.match(/(?:^|\s)(\d+(?:\.\d+)?|\.\d+)px(?:\/[^\s]+)?(?=\s|$)/i);
  if (!sizeMatch) {
    return null;
  }
  const size = Number(sizeMatch[1]);
  const family = value.slice(sizeMatch.index + sizeMatch[0].length).trim();
  if (!Number.isFinite(size) || size <= 0 || !family) {
    return null;
  }
  const prefix = value.slice(0, sizeMatch.index).trim();
  const weightTokens = Array.from(
    prefix.matchAll(/(?:^|\s)(normal|bold|bolder|lighter|[1-9]\d{0,2}|1000)(?=\s|$)/gi),
    (match) => match[1],
  );
  const weightToken =
    weightTokens.find((token) => token.toLowerCase() !== "normal") ?? weightTokens.at(-1);
  if (typeof katanaResolvedTextFontWeight !== "function") {
    return null;
  }
  return {
    family,
    size,
    weight: katanaResolvedTextFontWeight(weightToken ?? "", 400),
    italic: /(?:^|\s)(?:italic|oblique)(?=\s|$)/i.test(prefix),
  };
}

function katanaCanvasFontScale(font) {
  return katanaFiniteFontSize(katanaCanvasFontSize(font)) / 16;
}

function katanaCanvasFontSize(font) {
  const match = String(font ?? "").match(/(\d+(?:\.\d+)?)px/);
  return match ? Number(match[1]) : 16;
}

function katanaCanvasCharacterWidth(char, font) {
  if (char.charCodeAt(0) === 160) {
    return katanaCanvasAsciiCharacterWidth(" ", font);
  }
  if (char.charCodeAt(0) <= 255) {
    return katanaCanvasAsciiCharacterWidth(char, font);
  }
  return katanaTextWidth(char);
}

function katanaCanvasAsciiCharacterWidth(char, font) {
  const width =
    KATANA_CANVAS_ASCII_TEXT_WIDTHS[char] ??
    (katanaIsC4Diagram() ? KATANA_C4_CANVAS_EXTRA_ASCII_TEXT_WIDTHS[char] : undefined) ??
    katanaTextWidth(char);
  return katanaIsC4Diagram() ? width * katanaC4CanvasAsciiWidthScale(font) : width;
}

function katanaC4CanvasAsciiWidthScale(font) {
  return /(?:^|\s)(?:bold|[6-9]00)(?:\s|$)/i.test(String(font))
    ? KATANA_C4_CANVAS_BOLD_ASCII_WIDTH_SCALE
    : KATANA_C4_CANVAS_NORMAL_ASCII_WIDTH_SCALE;
}

const KATANA_C4_CANVAS_NORMAL_ASCII_WIDTH_SCALE = 1.0000378195990014;
const KATANA_C4_CANVAS_BOLD_ASCII_WIDTH_SCALE = 1.0853877396807405;
const KATANA_C4_CANVAS_EXTRA_ASCII_TEXT_WIDTHS = {
  "[": 4.445,
  "]": 4.445,
};

const KATANA_CANVAS_ASCII_TEXT_WIDTHS = {
  " ": 4.445,
  "!": 4.445,
  '"': 5.68,
  "'": 3.055,
  "(": 5.328,
  ")": 5.328,
  "-": 5.328,
  ".": 4.445,
  "/": 4.445,
  ":": 4.445,
  0: 8.898,
  1: 8.898,
  2: 8.898,
  3: 8.898,
  4: 8.898,
  5: 8.898,
  6: 8.898,
  7: 8.898,
  8: 8.898,
  9: 8.898,
  "=": 9.344,
  A: 10.672,
  B: 10.672,
  C: 11.555,
  D: 11.555,
  E: 10.672,
  F: 9.773,
  G: 12.445,
  H: 11.555,
  I: 4.445,
  J: 8,
  K: 10.672,
  L: 8.898,
  M: 13.328,
  N: 11.555,
  O: 12.445,
  P: 10.672,
  Q: 12.445,
  R: 11.555,
  S: 10.672,
  T: 9.773,
  U: 11.555,
  V: 10.672,
  W: 15.102,
  X: 10.672,
  Y: 10.672,
  Z: 9.773,
  _: 8.898,
  a: 8.898,
  b: 8.898,
  c: 8,
  d: 8.898,
  e: 8.898,
  f: 4.445,
  g: 8.898,
  h: 8.898,
  i: 3.555,
  j: 3.555,
  k: 8,
  l: 3.555,
  m: 13.328,
  n: 8.898,
  o: 8.898,
  p: 8.898,
  q: 8.898,
  r: 5.328,
  s: 8,
  t: 4.445,
  u: 8.898,
  v: 8,
  w: 11.555,
  x: 8,
  y: 8,
  z: 8,
};
