function katanaNormalizeTreemapReviewSvg(svg) {
  if (!svg.includes('aria-roledescription="treemap"')) {
    return svg;
  }
  return katanaNormalizeTreemapValueText(svg);
}

function katanaNormalizeTreemapValueText(svg) {
  return svg.replace(
    /<text\b([^>]*class="treemapValue"[^>]*)>([^<]*)<\/text>/g,
    (_match, attributes, text) => `<text${katanaTreemapValueAttributes(attributes)}>${text}</text>`,
  );
}

function katanaTreemapValueAttributes(attributes) {
  return attributes.replace(/fill:\s*lightgrey/g, "fill: lightgrey");
}
