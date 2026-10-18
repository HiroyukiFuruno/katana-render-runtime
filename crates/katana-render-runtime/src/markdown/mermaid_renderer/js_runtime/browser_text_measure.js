/** @param {unknown} text */
function katanaTextWidth(text) {
  const chars = Array.from(katanaMeasuredTextValue(text));
  const characterWidth = chars
    .map((char) => katanaCharacterWidth(char))
    .reduce((width, charWidth) => width + charWidth, 0);
  return characterWidth + katanaTextKerningWidth(chars);
}

/** @param {unknown} text */
function katanaMeasuredTextValue(text) {
  if (globalThis.__katanaMermaidDiagramType === "block") {
    return String(text);
  }
  return String(text).replace(/&(?:amp;)?nbsp;|&#160;/g, "\u00A0");
}

/** @param {readonly string[]} chars */
function katanaTextKerningWidth(chars) {
  return chars
    .slice(0, -1)
    .map((char, index) => katanaKerningPairWidth(char, /** @type {string} */ (chars[index + 1])))
    .reduce((width, pairWidth) => width + pairWidth, 0);
}

/** @param {string} left @param {string} right */
function katanaKerningPairWidth(left, right) {
  return KATANA_BROWSER_KERNING_PAIR_WIDTH_READERS[
    /** @type {0 | 1} */ (Number(katanaHasWideKerningChar(left, right)))
  ](left, right);
}

/** @param {string} left @param {string} right */
function katanaHasWideKerningChar(left, right) {
  return [left, right].some(katanaIsWideTextChar);
}

/** @param {string} char */
function katanaIsWideTextChar(char) {
  return char.charCodeAt(0) > 255;
}

/** @param {string} char */
function katanaCharacterWidth(char) {
  return KATANA_BROWSER_CHARACTER_WIDTHS[/** @type {0 | 1} */ (Number(char.charCodeAt(0) > 255))](
    char,
  );
}

/** @param {string} char */
function katanaAsciiCharacterWidth(char) {
  if (char.charCodeAt(0) === 160) {
    return KATANA_ASCII_TEXT_WIDTHS[" "];
  }
  return /** @type {number} */ ([KATANA_ASCII_TEXT_WIDTHS[char]].filter(Boolean).concat([8])[0]);
}

/** @param {string} char */
function katanaWideCharacterWidth(char) {
  return KATANA_BROWSER_WIDE_CHARACTER_WIDTHS[
    /** @type {0 | 1} */ (Number(globalThis.__katanaMermaidDiagramType === "kanban"))
  ](char);
}

/** @type {readonly [(char: string) => number, () => number]} */
const KATANA_BROWSER_WIDE_CHARACTER_WIDTHS = [katanaDefaultWideCharacterWidth, () => 12.5];

/** @param {string} char */
function katanaDefaultWideCharacterWidth(char) {
  if (katanaIsWidePunctuation(char)) {
    return 20;
  }
  return KATANA_BROWSER_DEFAULT_WIDE_CHARACTER_WIDTHS[
    /** @type {0 | 1} */ (Number(katanaIsCjkIdeograph(char)))
  ]();
}

/** @param {string} char */
function katanaIsCjkIdeograph(char) {
  const codePoint = katanaCharacterCodePoint(char);
  return KATANA_BROWSER_CJK_IDEOGRAPH_RULES.every((rule) => rule(codePoint));
}

/** @param {string} char */
function katanaCharacterCodePoint(char) {
  return char.codePointAt(0) ?? 0;
}

/** @param {number} codePoint */
function katanaIsCjkIdeographStart(codePoint) {
  return codePoint >= 0x4e00;
}

/** @param {number} codePoint */
function katanaIsCjkIdeographEnd(codePoint) {
  return codePoint <= 0x9fff;
}

/** @param {string} char */
function katanaIsWidePunctuation(char) {
  return ["。", "、"].includes(char);
}

/** @type {readonly [() => number, () => number]} */
const KATANA_BROWSER_DEFAULT_WIDE_CHARACTER_WIDTHS = [() => 15.8, () => 16.3];
/** @type {readonly ((codePoint: number) => boolean)[]} */
const KATANA_BROWSER_CJK_IDEOGRAPH_RULES = [katanaIsCjkIdeographStart, katanaIsCjkIdeographEnd];
/** @type {readonly [(char: string) => number, (char: string) => number]} */
const KATANA_BROWSER_CHARACTER_WIDTHS = [katanaAsciiCharacterWidth, katanaWideCharacterWidth];
/** @type {readonly [(left: string, right: string) => number, () => number]} */
const KATANA_BROWSER_KERNING_PAIR_WIDTH_READERS = [
  (left, right) => KATANA_ASCII_KERNING_PAIRS[`${left}${right}`] ?? 0,
  () => 0,
];
