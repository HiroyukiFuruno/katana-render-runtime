import { type RuntimeFragment, RuntimeFragmentFactory } from "#shared/runtime_bundle";

const MERMAID_JS_RUNTIME =
  "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime";

export const SharedDomFragments = {
  all(): RuntimeFragment[] {
    return [
      RuntimeFragmentFactory.verbatim(
        "browser-globals.js",
        `${MERMAID_JS_RUNTIME}/browser_globals.js`,
      ),
      RuntimeFragmentFactory.verbatim("dom-node.js", `${MERMAID_JS_RUNTIME}/dom_node.js`),
      RuntimeFragmentFactory.verbatim("dom-style.js", `${MERMAID_JS_RUNTIME}/dom_style.js`),
      RuntimeFragmentFactory.verbatim("dom-html.js", `${MERMAID_JS_RUNTIME}/dom_html.js`),
      RuntimeFragmentFactory.verbatim("dom-node-html.js", `${MERMAID_JS_RUNTIME}/dom_node_html.js`),
      RuntimeFragmentFactory.verbatim(
        "dom-canvas-context.js",
        `${MERMAID_JS_RUNTIME}/dom_canvas_context.js`,
      ),
      RuntimeFragmentFactory.verbatim("dom-selector.js", `${MERMAID_JS_RUNTIME}/dom_selector.js`),
      RuntimeFragmentFactory.verbatim("dom-document.js", `${MERMAID_JS_RUNTIME}/dom_document.js`),
      RuntimeFragmentFactory.verbatim("dom-tree.js", `${MERMAID_JS_RUNTIME}/dom_tree.js`),
      RuntimeFragmentFactory.verbatim("dom-layout.js", `${MERMAID_JS_RUNTIME}/dom_layout.js`),
      RuntimeFragmentFactory.verbatim("dom-metrics.js", `${MERMAID_JS_RUNTIME}/dom_metrics.js`),
      RuntimeFragmentFactory.verbatim(
        "dom-layout-prototype.js",
        `${MERMAID_JS_RUNTIME}/dom_layout_prototype.js`,
      ),
      RuntimeFragmentFactory.verbatim("dom-css-style.js", `${MERMAID_JS_RUNTIME}/dom_css_style.js`),
      RuntimeFragmentFactory.verbatim(
        "dom-computed-style.js",
        `${MERMAID_JS_RUNTIME}/dom_computed_style.js`,
      ),
      RuntimeFragmentFactory.verbatim("dom-measure.js", `${MERMAID_JS_RUNTIME}/dom_measure.js`),
      RuntimeFragmentFactory.verbatim(
        "dom-measure-text-lines.js",
        `${MERMAID_JS_RUNTIME}/dom_measure_text_lines.js`,
      ),
      RuntimeFragmentFactory.verbatim(
        "dom-text-metrics.js",
        `${MERMAID_JS_RUNTIME}/dom_text_metrics.js`,
      ),
      RuntimeFragmentFactory.verbatim(
        "dom-text-measure.js",
        `${MERMAID_JS_RUNTIME}/dom_text_measure.js`,
      ),
    ];
  },
};
