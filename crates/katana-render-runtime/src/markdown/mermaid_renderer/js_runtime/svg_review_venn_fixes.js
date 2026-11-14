function katanaInsertSvgBackground(svg, color) {
  return svg.replace(
    /(<svg\b[^>]*>)/,
    `$1${katanaSvgBackgroundRect(katanaBackgroundViewBox(svg), color)}`,
  );
}

function katanaBackgroundViewBox(svg) {
  if (typeof katanaReadViewBox !== "function") {
    return null;
  }
  return katanaReadViewBox(svg);
}

function katanaSvgBackgroundRect(viewBox, color) {
  if (viewBox) {
    return `<rect x="${viewBox[0]}" y="${viewBox[1]}" width="${viewBox[2]}" height="${viewBox[3]}" fill="${color}"></rect>`;
  }
  return `<rect width="100%" height="100%" fill="${color}"></rect>`;
}
