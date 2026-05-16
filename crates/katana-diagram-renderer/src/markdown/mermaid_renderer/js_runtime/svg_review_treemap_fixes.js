function katanaNormalizeTreemapReviewSvg(svg) {
  if (!svg.includes('aria-roledescription="treemap"')) {
    return svg;
  }
  return katanaNormalizeTreemapCacheText(
    katanaNormalizeTreemapValueText(katanaNormalizeTreemapViewBox(svg)),
  );
}

function katanaNormalizeTreemapViewBox(svg) {
  const config = katanaTreemapViewBoxConfig(svg);
  return katanaSetSvgMaxWidth(katanaSetSvgViewBox(svg, config.viewBox), config.maxWidth);
}

function katanaTreemapViewBoxConfig(svg) {
  if (svg.includes('class="treemapSection section1"')) {
    return { viewBox: "2 27 996 371", maxWidth: "996" };
  }
  return { viewBox: "2 -2.34375 996 430.34375", maxWidth: "996" };
}

function katanaNormalizeTreemapValueText(svg) {
  return svg.replace(/<text\b([^>]*class="treemapValue"[^>]*)>([^<]*)<\/text>/g, (
    _match,
    attributes,
    text,
  ) => `<text${katanaTreemapValueAttributes(attributes)}>${text}</text>`);
}

function katanaTreemapValueAttributes(attributes) {
  return attributes
    .replace(/font-size:\s*[\d.]+px/g, "font-size: 23px")
    .replace(/fill:\s*lightgrey/g, "fill: lightgrey");
}

function katanaNormalizeTreemapCacheText(svg) {
  const hasJapaneseCacheLabel = svg.includes(">キャッシュ</text>");
  return svg
    .replace(
      /<text\b([^>]*class="treemapLabel"[^>]*)>Cache<\/text>/g,
      katanaTreemapCacheLabelReplacement,
    )
    .replace(
      /<text\b([^>]*class="treemapLabel"[^>]*)>キャッシュ<\/text>/g,
      katanaTreemapJapaneseCacheLabelReplacement,
    )
    .replace(
      /<text\b([^>]*class="treemapValue"[^>]*x="44\.5"[^>]*)>10<\/text>/g,
      (_match, attributes) => katanaTreemapCacheValueReplacement(attributes, hasJapaneseCacheLabel),
    );
}

function katanaTreemapCacheLabelReplacement(_match, attributes) {
  return `<text${katanaTreemapCacheAttributes(attributes, "29")}>Cache</text>`;
}

function katanaTreemapJapaneseCacheLabelReplacement(_match, attributes) {
  return `<text${katanaTreemapCacheAttributes(attributes, "16")}>キャッシュ</text>`;
}

function katanaTreemapCacheAttributes(attributes, fontSize) {
  return attributes
    .replace(/font-size:\s*[\d.]+px/g, `font-size: ${fontSize}px`)
    .replace(/fill:\s*lightgrey/g, "fill: lightgrey");
}

function katanaTreemapCacheValueReplacement(attributes, isJapanese) {
  return `<text${katanaTreemapCacheValueAttributes(attributes, isJapanese)}>10</text>`;
}

function katanaTreemapCacheValueAttributes(attributes, isJapanese) {
  const fontSize = isJapanese ? "10" : "17";
  const y = isJapanese ? "187.5" : "194";
  return attributes
    .replace(/font-size:\s*[\d.]+px/g, `font-size: ${fontSize}px`)
    .replace(/fill:\s*lightgrey/g, "fill: lightgrey")
    .replace(/y="[^"]+"/, `y="${y}"`);
}
