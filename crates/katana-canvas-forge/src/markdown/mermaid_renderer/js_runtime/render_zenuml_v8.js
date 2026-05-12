// Bridge script for @zenuml/core V8 rendering.
// Input:  __zenuml_source__ (global var set by preamble script)
//         __zenuml_dark__   (boolean, set by preamble script)
// Output: SVG string (last expression, captured by DiagramV8Runtime)
(function () {
  var source = __zenuml_source__;
  // Strip the leading "zenuml" keyword line.
  // Mermaid fenced code blocks include it as the diagram-type token,
  // but @zenuml/core's renderToSvg() treats it as a participant name.
  var stripped = source.replace(/^zenuml[^\n]*\n?/, "");
  var svg = zenuml.renderToSvg(stripped).svg;
  if (__zenuml_dark__) {
    // renderToSvg in @zenuml/core 3.47.9 (GZ function) ignores the options
    // argument entirely. The generated SVG contains an embedded <defs><style>
    // block (the WZ stylesheet) with literal colors such as
    // `.participant-box { fill: #ffffff; }`. CSS variable injection before
    // <defs> loses the cascade. Instead, inject an override <style> block
    // immediately before </svg> so it comes after WZ and wins via cascade order.
    // Class selectors (specificity 0,1,0) also beat SVG presentation attributes
    // (fill="#666"), so all literal colors are covered.
    var darkStyle = '<style>' +
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
})();
