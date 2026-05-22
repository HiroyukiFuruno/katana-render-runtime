function katanaTextWidth(text) {
  const chars = Array.from(katanaMeasuredTextValue(text));
  const characterWidth = chars
    .map((char) => katanaCharacterWidth(char))
    .reduce((width, charWidth) => width + charWidth, 0);
  return characterWidth + katanaTextKerningWidth(chars);
}

function katanaMeasuredTextValue(text) {
  if (globalThis.__katanaMermaidDiagramType === "block") {
    return String(text);
  }
  return String(text).replace(/&(?:amp;)?nbsp;|&#160;/g, "\u00A0");
}

function katanaTextKerningWidth(chars) {
  return chars
    .slice(0, -1)
    .map((char, index) => katanaKerningPairWidth(char, chars[index + 1]))
    .reduce((width, pairWidth) => width + pairWidth, 0);
}

function katanaKerningPairWidth(left, right) {
  return KATANA_BROWSER_KERNING_PAIR_WIDTH_READERS[
    Number(katanaHasWideKerningChar(left, right))
  ](left, right);
}

function katanaHasWideKerningChar(left, right) {
  return [left, right].some(katanaIsWideTextChar);
}

function katanaIsWideTextChar(char) {
  return char.charCodeAt(0) > 255;
}

function katanaCharacterWidth(char) {
  return KATANA_BROWSER_CHARACTER_WIDTHS[Number(char.charCodeAt(0) > 255)](char);
}

function katanaAsciiCharacterWidth(char) {
  if (char.charCodeAt(0) === 160) {
    return KATANA_ASCII_TEXT_WIDTHS[" "];
  }
  return [KATANA_ASCII_TEXT_WIDTHS[char]].filter(Boolean).concat([8])[0];
}

function katanaWideCharacterWidth(char) {
  return KATANA_BROWSER_WIDE_CHARACTER_WIDTHS[
    Number(globalThis.__katanaMermaidDiagramType === "kanban")
  ](char);
}

const KATANA_BROWSER_WIDE_CHARACTER_WIDTHS = [
  katanaDefaultWideCharacterWidth,
  () => 12.5,
];

function katanaDefaultWideCharacterWidth(char) {
  if (katanaIsWidePunctuation(char)) {
    return 20;
  }
  return KATANA_BROWSER_DEFAULT_WIDE_CHARACTER_WIDTHS[Number(katanaIsCjkIdeograph(char))]();
}

function katanaIsCjkIdeograph(char) {
  const codePoint = katanaCharacterCodePoint(char);
  return KATANA_BROWSER_CJK_IDEOGRAPH_RULES.every((rule) => rule(codePoint));
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

const KATANA_BROWSER_DEFAULT_WIDE_CHARACTER_WIDTHS = [() => 15.8, () => 16.3];
const KATANA_BROWSER_CJK_IDEOGRAPH_RULES = [
  katanaIsCjkIdeographStart,
  katanaIsCjkIdeographEnd,
];
const KATANA_BROWSER_CHARACTER_WIDTHS = [katanaAsciiCharacterWidth, katanaWideCharacterWidth];
const KATANA_BROWSER_KERNING_PAIR_WIDTH_READERS = [
  (left, right) => KATANA_ASCII_KERNING_PAIRS[`${left}${right}`] ?? 0,
  () => 0,
];
