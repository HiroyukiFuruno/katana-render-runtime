function katanaShouldNormalizeVennReviewSvg(svg) {
  return [svg.includes('aria-roledescription="venn"'), katanaIsRendererScopeVenn(svg)].every(
    Boolean,
  );
}

function katanaIsRendererScopeVenn(svg) {
  return [svg.includes('data-venn-sets="official"'), svg.includes('data-venn-sets="rust"')].every(
    Boolean,
  );
}

function katanaNormalizeVennReviewPaths(svg) {
  return katanaVennPath(svg, "venn-set-0", "rgb(122,122,122)", "0.1").replace(
    /(<g class="venn-area venn-circle venn-set-1"[\s\S]*?<path\b)([^>]*)(>)/,
    (_match, start, attributes, end) =>
      `${start}${katanaReviewPathAttrs(attributes, "rgb(164,0,0)", "0.1")}${end}`,
  );
}

function katanaNormalizeVennReviewTheme(svg, request) {
  if (request.theme !== "dark") {
    return svg;
  }
  return katanaNormalizeVennReviewTextColors(katanaInsertSvgBackground(svg, "#1e1e1e"));
}

function katanaNormalizeVennReviewTextColors(svg) {
  return katanaVennTextColor(
    katanaVennTextColor(
      katanaVennTextColor(katanaVennTitleColor(svg), "venn-set-0", "rgb(198, 198, 198)"),
      "venn-set-1",
      "rgb(255, 62, 62)",
    ),
    "venn-intersection",
    "rgb(224, 224, 224)",
  );
}

function katanaVennTitleColor(svg) {
  return svg.replace(
    /<text class="venn-title"([^>]*)>/,
    (_match, attributes) =>
      `<text class="venn-title"${attributes.replace(/\sstyle="[^"]*"/, "")} style="fill: rgb(224, 224, 224);">`,
  );
}

function katanaVennTextColor(svg, className, color) {
  const pattern = new RegExp(
    `(<g class="venn-area [^"]*${className}[^"]*"[\\s\\S]*?<text class="label")([^>]*)(>)`,
  );
  return svg.replace(
    pattern,
    (_match, start, attributes, end) =>
      `${start}${attributes.replace(/\sstyle="[^"]*"/, "")} style="fill: ${color}; font-size: 24px;"${end}`,
  );
}

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

function katanaVennPath(svg, className, color, opacity) {
  const pattern = new RegExp(
    `(<g class="venn-area venn-circle ${className}"[\\s\\S]*?<path\\b)([^>]*)(>)`,
  );
  return svg.replace(
    pattern,
    (_match, start, attributes, end) =>
      `${start}${katanaReviewPathAttrs(attributes, color, opacity)}${end}`,
  );
}
