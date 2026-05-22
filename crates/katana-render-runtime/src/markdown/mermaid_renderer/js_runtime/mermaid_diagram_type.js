function katanaMermaidDiagramType(source) {
  const lines = String(source)
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
  return katanaMermaidDiagramTypeLine(lines)?.split(/\s+/)[0].toLowerCase() ?? "";
}

function katanaMermaidDiagramTypeLine(lines) {
  const bodyLines = katanaMermaidDiagramTypeLineReaders()[Number(lines[0] === "---")](lines);
  return bodyLines.find((line) => !katanaMermaidIsDirectiveOrComment(line));
}

function katanaMermaidDiagramTypeLineReaders() {
  return [katanaMermaidFirstDiagramTypeLine, katanaMermaidFrontmatterDiagramTypeLine];
}

function katanaMermaidFirstDiagramTypeLine(lines) {
  return lines;
}

function katanaMermaidFrontmatterDiagramTypeLine(lines) {
  const endIndex = lines.slice(1).indexOf("---");
  return lines.slice(katanaMermaidFrontmatterBodyStartIndex(endIndex));
}

function katanaMermaidFrontmatterBodyStartIndex(endIndex) {
  return [0, endIndex + 2][Number(endIndex >= 0)];
}

function katanaMermaidIsDirectiveOrComment(line) {
  return line.startsWith("%%");
}
