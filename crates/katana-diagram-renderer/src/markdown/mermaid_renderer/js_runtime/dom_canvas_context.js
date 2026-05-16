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
  const fontScale = katanaCanvasFontScale(font);
  const chars = Array.from(katanaMeasuredTextValue(text));
  return chars
    .map((char) => katanaCanvasCharacterWidth(char))
    .reduce((width, charWidth) => width + charWidth * fontScale, 0);
}

function katanaCanvasFontScale(font) {
  return katanaFiniteFontSize(katanaCanvasFontSize(font)) / 16;
}

function katanaCanvasFontSize(font) {
  const match = String(font ?? "").match(/(\d+(?:\.\d+)?)px/);
  return match ? Number(match[1]) : 16;
}

function katanaCanvasCharacterWidth(char) {
  if (char.charCodeAt(0) === 160) {
    return KATANA_CANVAS_ASCII_TEXT_WIDTHS[" "];
  }
  return KATANA_CANVAS_ASCII_TEXT_WIDTHS[char] ?? katanaTextWidth(char);
}

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
