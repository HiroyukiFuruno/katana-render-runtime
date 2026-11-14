import { MERMAID_RUNTIME_ENTRYPOINT } from "#mermaid/runtime_entrypoint";
import { SharedDomFragments } from "#shared/dom_fragments";
import {
  RuntimeBundleDefinition,
  type RuntimeFragment,
  RuntimeFragmentFactory,
} from "#shared/runtime_bundle";

const MERMAID_JS_RUNTIME = "crates/katana-render-runtime/src/markdown/mermaid_renderer/js_runtime";

export const MermaidRuntimeBundleDefinition = {
  create(): RuntimeBundleDefinition {
    return new RuntimeBundleDefinition(
      "mermaid",
      "mermaid-runtime.min.js",
      [...SharedDomFragments.all(), ...mermaidRuntimeFragments()],
      MERMAID_RUNTIME_ENTRYPOINT,
    );
  },
};

function mermaidRuntimeFragments(): RuntimeFragment[] {
  return [
    RuntimeFragmentFactory.verbatim(
      "svg-bounds-attrs.js",
      `${MERMAID_JS_RUNTIME}/svg_bounds_attrs.js`,
    ),
    RuntimeFragmentFactory.verbatim("svg-bounds.js", `${MERMAID_JS_RUNTIME}/svg_bounds.js`),
    RuntimeFragmentFactory.verbatim(
      "svg-text-bounds.js",
      `${MERMAID_JS_RUNTIME}/svg_text_bounds.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "svg-architecture-icons.js",
      `${MERMAID_JS_RUNTIME}/svg_architecture_icons.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "svg-group-utils.js",
      `${MERMAID_JS_RUNTIME}/svg_group_utils.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "svg-viewbox-fixes.js",
      `${MERMAID_JS_RUNTIME}/svg_viewbox_fixes.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "svg-flowchart-fixture-layout-fixes.js",
      `${MERMAID_JS_RUNTIME}/svg_flowchart_fixture_layout_fixes.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "svg-block-fixture-layout-fixes.js",
      `${MERMAID_JS_RUNTIME}/svg_block_fixture_layout_fixes.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "svg-er-fixture-layout-fixes.js",
      `${MERMAID_JS_RUNTIME}/svg_er_fixture_layout_fixes.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "svg-class-fixes.js",
      `${MERMAID_JS_RUNTIME}/svg_class_fixes.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "svg-visual-fixes.js",
      `${MERMAID_JS_RUNTIME}/svg_visual_fixes.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "svg-sequence-activation-fixes.js",
      `${MERMAID_JS_RUNTIME}/svg_sequence_activation_fixes.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "svg-requirement-localized-fixes.js",
      `${MERMAID_JS_RUNTIME}/svg_requirement_localized_fixes.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "svg-c4-context-full-fixture-layout.js",
      `${MERMAID_JS_RUNTIME}/svg_c4_context_full_fixture_layout.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "svg-review-fixes.js",
      `${MERMAID_JS_RUNTIME}/svg_review_fixes.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "svg-review-venn-fixes.js",
      `${MERMAID_JS_RUNTIME}/svg_review_venn_fixes.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "svg-review-treemap-fixes.js",
      `${MERMAID_JS_RUNTIME}/svg_review_treemap_fixes.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "svg-kanban-fixes.js",
      `${MERMAID_JS_RUNTIME}/svg_kanban_fixes.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "svg-wardley-normalizer.js",
      `${MERMAID_JS_RUNTIME}/svg_wardley_normalizer.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "svg-mindmap-layout-fixes.js",
      `${MERMAID_JS_RUNTIME}/svg_mindmap_layout_fixes.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "svg-diagram-normalizers.js",
      `${MERMAID_JS_RUNTIME}/svg_diagram_normalizers.js`,
    ),
    RuntimeFragmentFactory.verbatim("svg-dimensions.js", `${MERMAID_JS_RUNTIME}/svg_dimensions.js`),
    RuntimeFragmentFactory.verbatim(
      "source-i18n-context.js",
      `${MERMAID_JS_RUNTIME}/source_i18n_context.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "source-i18n-normalize.js",
      `${MERMAID_JS_RUNTIME}/source_i18n_normalize.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "mermaid-diagram-type.js",
      `${MERMAID_JS_RUNTIME}/mermaid_diagram_type.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "svg-zenuml-fixes.js",
      `${MERMAID_JS_RUNTIME}/svg_zenuml_fixes.js`,
    ),
    RuntimeFragmentFactory.verbatim("svg-normalize.js", `${MERMAID_JS_RUNTIME}/svg_normalize.js`),
  ];
}
