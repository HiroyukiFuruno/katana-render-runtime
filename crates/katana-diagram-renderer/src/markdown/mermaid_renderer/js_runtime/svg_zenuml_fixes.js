function katanaNormalizeZenumlRuntimeSvg(svg, request) {
  if (!katanaShouldNormalizeZenumlRuntimeSvg(svg, request)) {
    return svg;
  }
  return katanaZenumlRuntimeShiftThresholds(svg, request.source).reduce(
    (current, threshold, index) =>
      katanaApplyZenumlRuntimeVerticalShift(current, threshold + index * 20, 20),
    svg,
  );
}

function katanaShouldNormalizeZenumlRuntimeSvg(svg, request) {
  return [
    svg.includes('id="katana-mermaid-svg-'),
    svg.includes('aria-roledescription="zenuml"'),
    katanaMermaidDiagramType(request.source) === "zenuml",
    katanaZenumlCommentedContainerCount(request.source) > 0,
  ].every(Boolean);
}

function katanaZenumlRuntimeShiftThresholds(svg, source) {
  return katanaZenumlCommentYValues(svg)
    .slice(0, katanaZenumlCommentedContainerCount(source))
    .map((y) => y + 22);
}

function katanaZenumlCommentYValues(svg) {
  return Array.from(
    svg.matchAll(/<text class="comment-text">[\s\S]*?<tspan\b[^>]*\by="(-?\d+(?:\.\d+)?)"/g),
  )
    .map((match) => Number(match[1]))
    .filter(Number.isFinite);
}

function katanaZenumlCommentedContainerCount(source) {
  let pendingComment = false;
  let count = 0;
  for (const line of source.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (trimmed.startsWith("//")) {
      pendingComment = true;
      continue;
    }
    if (!trimmed) {
      pendingComment = false;
      continue;
    }
    if (pendingComment && katanaZenumlLineStartsContainerMessage(trimmed)) {
      count += 1;
    }
    pendingComment = false;
  }
  return count;
}

function katanaZenumlLineStartsContainerMessage(line) {
  return [
    line.endsWith("{"),
    !line.startsWith("@"),
    !/^(alt|critical|else|finally|group|if|loop|opt|par|section|try)\b/.test(line),
  ].every(Boolean);
}

function katanaApplyZenumlRuntimeVerticalShift(svg, threshold, shift) {
  return katanaShiftZenumlYAttributes(
    katanaShiftZenumlTransforms(
      katanaShiftZenumlPolylinePoints(
        katanaExpandZenumlCrossingRects(
          katanaExpandZenumlRootHeight(svg, shift),
          threshold,
          shift,
        ),
        threshold,
        shift,
      ),
      threshold,
      shift,
    ),
    threshold,
    shift,
  );
}

function katanaExpandZenumlRootHeight(svg, shift) {
  return svg
    .replace(/(<svg\b[^>]*\bheight=")(-?\d+(?:\.\d+)?)/, (_match, before, height) =>
      `${before}${katanaFormatZenumlNumber(Number(height) + shift)}`,
    )
    .replace(/(<svg\b[^>]*\bviewBox=")([^"]+)(")/, (_match, before, viewBox, after) => {
      const values = viewBox.split(/\s+/).map(Number);
      if (values.length !== 4 || values.some((value) => !Number.isFinite(value))) {
        return `${before}${viewBox}${after}`;
      }
      values[3] += shift;
      return `${before}${values.map(katanaFormatZenumlNumber).join(" ")}${after}`;
    });
}

function katanaExpandZenumlCrossingRects(svg, threshold, shift) {
  return svg.replace(/<rect\b[^>]*>/g, (tag) => {
    const y = katanaZenumlAttrNumber(tag, "y");
    const height = katanaZenumlAttrNumber(tag, "height");
    if (y === null || height === null || y >= threshold || y + height <= threshold) {
      return tag;
    }
    return katanaReplaceZenumlAttrNumber(tag, "height", height + shift);
  });
}

function katanaShiftZenumlYAttributes(svg, threshold, shift) {
  return svg.replace(/\b(y|y1|y2)="(-?\d+(?:\.\d+)?)"/g, (_match, name, value) =>
    Number(value) > threshold
      ? `${name}="${katanaFormatZenumlNumber(Number(value) + shift)}"`
      : `${name}="${value}"`,
  );
}

function katanaShiftZenumlPolylinePoints(svg, threshold, shift) {
  return svg.replace(/\bpoints="([^"]+)"/g, (_match, points) => {
    const shifted = points.replace(
      /(-?\d+(?:\.\d+)?),(-?\d+(?:\.\d+)?)/g,
      (_point, x, y) =>
        Number(y) > threshold
          ? `${x},${katanaFormatZenumlNumber(Number(y) + shift)}`
          : `${x},${y}`,
    );
    return `points="${shifted}"`;
  });
}

function katanaShiftZenumlTransforms(svg, threshold, shift) {
  return svg.replace(
    /transform="translate\((-?\d+(?:\.\d+)?)(,\s*|\s+)(-?\d+(?:\.\d+)?)(\)[^"]*)"/g,
    (_match, x, separator, y, suffix) =>
      Number(y) > threshold
        ? `transform="translate(${x}${separator}${katanaFormatZenumlNumber(
            Number(y) + shift,
          )}${suffix}"`
        : `transform="translate(${x}${separator}${y}${suffix}"`,
  );
}

function katanaZenumlAttrNumber(tag, name) {
  const match = tag.match(new RegExp(`\\b${name}="(-?\\d+(?:\\.\\d+)?)"`));
  return match ? Number(match[1]) : null;
}

function katanaReplaceZenumlAttrNumber(tag, name, value) {
  const pattern = new RegExp(`\\b${name}="(-?\\d+(?:\\.\\d+)?)"`);
  return tag.replace(pattern, `${name}="${katanaFormatZenumlNumber(value)}"`);
}

function katanaFormatZenumlNumber(value) {
  return Number(value.toFixed(6)).toString();
}
