function katanaMermaidDiagramType(source) {
  const lines = String(source)
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
  return katanaMermaidDiagramTypeLine(lines)?.split(/\s+/)[0].toLowerCase() ?? "";
}

function katanaMermaidDiagramTypeLine(lines) {
  return katanaMermaidDiagramTypeLineReaders()[Number(lines[0] === "---")](lines);
}

function katanaMermaidDiagramTypeLineReaders() {
  return [katanaMermaidFirstDiagramTypeLine, katanaMermaidFrontmatterDiagramTypeLine];
}

function katanaMermaidFirstDiagramTypeLine(lines) {
  return lines[0];
}

function katanaMermaidFrontmatterDiagramTypeLine(lines) {
  const endIndex = lines.slice(1).indexOf("---");
  return lines[katanaMermaidFrontmatterBodyStartIndex(endIndex)];
}

function katanaMermaidFrontmatterBodyStartIndex(endIndex) {
  return [0, endIndex + 2][Number(endIndex >= 0)];
}
