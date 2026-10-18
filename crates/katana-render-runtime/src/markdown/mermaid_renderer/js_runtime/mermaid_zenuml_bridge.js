(() => {
  const zenumlDiagram =
    /** @type {import("../../../../../../scripts/mermaid/official-renderer-browser-globals").MermaidZenumlBrowserGlobals} */ (
      globalThis
    )["mermaid-zenuml"];
  if (zenumlDiagram) {
    /** @type {import("../../../../../../scripts/mermaid/official-renderer-browser-globals").MermaidZenumlBrowserGlobals} */ (
      globalThis
    ).__katanaMermaidZenuml = zenumlDiagram;
  }
})();
