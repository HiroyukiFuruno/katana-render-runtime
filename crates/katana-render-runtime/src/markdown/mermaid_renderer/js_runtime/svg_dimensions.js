function katanaReadViewBox(svg) {
  const match = svg.match(/viewBox="([^"]+)"/);
  if (!match) {
    return null;
  }
  return katanaSvgDimensionViewBoxValues(match[1]);
}

function katanaSvgDimensionViewBoxValues(value) {
  const values = value.split(/\s+/).map((it) => Number(it));
  if (!katanaValidViewBoxValues(values)) {
    return null;
  }
  return values;
}

function katanaValidViewBoxValues(values) {
  return [values.length === 4, values.every((value) => Number.isFinite(value))].every(Boolean);
}

function katanaSetSvgDimension(svg, name, value) {
  const pattern = new RegExp(`\\s${name}="[^"]*"`);
  return svg.replace(/<svg\b[^>]*>/, (tag) => katanaSvgTagWithDimension(tag, pattern, name, value));
}

function katanaSvgTagWithDimension(tag, pattern, name, value) {
  if (pattern.test(tag)) {
    return tag.replace(pattern, ` ${name}="${value}"`);
  }
  return tag.replace("<svg", `<svg ${name}="${value}"`);
}

function katanaSetSvgViewBox(svg, value) {
  return svg.replace(/viewBox="[^"]*"/, `viewBox="${value}"`);
}

function katanaSetSvgMaxWidth(svg, width) {
  if (/style="[^"]*max-width:/.test(svg)) {
    return svg.replace(/max-width:\s*[^;"]+;?/g, `max-width: ${width}px;`);
  }
  return svg;
}

function katanaEnsureSvgAttr(tag, name, value) {
  const pattern = new RegExp(`\\s${name}="`);
  if (pattern.test(tag)) {
    return tag;
  }
  return tag.replace(/>$/, ` ${name}="${value}">`);
}
