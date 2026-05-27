import { SharedDomFragments } from "#shared/dom_fragments";
import { RuntimeBundleDefinition } from "#shared/runtime_bundle";

export const ZenumlRuntimeBundleDefinition = {
  create(): RuntimeBundleDefinition {
    return new RuntimeBundleDefinition(
      "zenuml",
      "zenuml-runtime.min.js",
      SharedDomFragments.all(),
      ZENUML_RUNTIME_ENTRYPOINT,
    );
  },
};

const ZENUML_RUNTIME_ENTRYPOINT = String.raw`
function katanaInstallMermaidZenumlRuntimeAdapter() {
  const zenumlDiagram = globalThis["mermaid-zenuml"];
  if (zenumlDiagram) {
    globalThis.__katanaMermaidZenuml = zenumlDiagram;
  }
}

globalThis["katanaRunZenumlRuntime"] = function katanaRunZenumlRuntime(source, isDark) {
  const stripped = String(source).replace(/^zenuml[^\n]*\n?/, "");
  let svg = zenuml.renderToSvg(stripped).svg;
  if (isDark) {
    const darkStyle = '<style>' +
      '.frame-border-outer{fill:#4a5170}' +
      '.frame-border-inner{fill:#111628}' +
      '.frame-header-bg{fill:#1d2240}' +
      '.frame-header-line{stroke:#cecfd2}' +
      '.frame-title{fill:#cecfd2}' +
      '.participant-box{fill:#5964f2;stroke:#cecfd2}' +
      '.participant-label{fill:#cecfd2}' +
      '.participant-icon{color:#cecfd2}' +
      '.participant-icon [fill="currentColor"]:not([stroke]){stroke:#cecfd2}' +
      '.lifeline{stroke:#cecfd2}' +
      '.message-line{stroke:#536fff}' +
      '.message-label{fill:#cecfd2}' +
      '.arrow-head{fill:#536fff;stroke:#536fff}' +
      '.occurrence{fill:#5964f2;stroke:#cecfd2}' +
      '.fragment-border{stroke:#cecfd2}' +
      '.fragment-header{fill:#5964f2}' +
      '.fragment-label{fill:#cecfd2}' +
      '.fragment-condition{fill:#cecfd2}' +
      '.fragment-separator{stroke:#cecfd2}' +
      '.fragment-section-label{fill:#cecfd2}' +
      '.return-line{stroke:#536fff}' +
      '.return-arrow{stroke:#536fff}' +
      '.return-label{fill:#cecfd2}' +
      '.return-icon{fill:#cecfd2}' +
      '.divider-line{stroke:#536fff}' +
      '.divider-bg{fill:#1d2240;stroke:#536fff}' +
      '.divider-label{fill:#cecfd2}' +
      '.comment-text{fill:#cecfd2}' +
      '.seq-number{fill:#8890b0}' +
      '.group-outline{stroke:#cecfd2}' +
      '.group-title-bg{fill:#111628}' +
      '.group-title-text{fill:#cecfd2}' +
    '</style>';
    svg = svg.replace('</svg>', darkStyle + '</svg>');
  }
  return svg;
};

katanaInstallMermaidZenumlRuntimeAdapter();
`;
