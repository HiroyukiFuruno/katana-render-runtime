import { SharedDomFragments } from "../shared/dom_fragments";
import {
  RuntimeBundleDefinition,
  type RuntimeFragment,
  RuntimeFragmentFactory,
} from "../shared/runtime_bundle";

const MERMAID_JS_RUNTIME =
  "crates/katana-diagram-renderer/src/markdown/mermaid_renderer/js_runtime";

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
    RuntimeFragmentFactory.verbatim(
      "svg-path-bounds.js",
      `${MERMAID_JS_RUNTIME}/svg_path_bounds.js`,
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
    RuntimeFragmentFactory.verbatim("svg-er-layout.js", `${MERMAID_JS_RUNTIME}/svg_er_layout.js`),
    RuntimeFragmentFactory.verbatim("svg-er-fixes.js", `${MERMAID_JS_RUNTIME}/svg_er_fixes.js`),
    RuntimeFragmentFactory.verbatim(
      "svg-er-viewbox-fixes.js",
      `${MERMAID_JS_RUNTIME}/svg_er_viewbox_fixes.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "svg-class-layout-fixes.js",
      `${MERMAID_JS_RUNTIME}/svg_class_layout_fixes.js`,
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
      "svg-review-fixes.js",
      `${MERMAID_JS_RUNTIME}/svg_review_fixes.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "svg-review-ishikawa-fixes.js",
      `${MERMAID_JS_RUNTIME}/svg_review_ishikawa_fixes.js`,
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
      "svg-kanban-text.js",
      `${MERMAID_JS_RUNTIME}/svg_kanban_text.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "svg-kanban-viewbox.js",
      `${MERMAID_JS_RUNTIME}/svg_kanban_viewbox.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "svg-kanban-label-layout.js",
      `${MERMAID_JS_RUNTIME}/svg_kanban_label_layout.js`,
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

const MERMAID_RUNTIME_ENTRYPOINT = `
function katanaInstallMermaidZenumlRuntimeAdapter() {
  const zenumlDiagram = globalThis["mermaid-zenuml"];
  if (zenumlDiagram) {
    globalThis.__katanaMermaidZenuml = zenumlDiagram;
  }
}

function katanaRunMermaidRuntime(request) {
  return (async () => katanaRenderMermaid(request))().catch((error) => {
    const detail = error?.stack ? error.stack : String(error);
    throw new Error(detail);
  });
}

async function katanaRenderMermaid(request) {
  const mermaidValue = katanaMermaidGlobal();
  mermaidValue.initialize({
    startOnLoad: false,
    securityLevel: "loose",
    htmlLabels: false,
    theme: request.theme,
    flowchart: {
      htmlLabels: false,
      useMaxWidth: false,
    },
    sequence: {
      useMaxWidth: false,
    },
    themeVariables: katanaMermaidThemeVariables(request),
  });
  const preparedSource = katanaNormalizeMermaidSourceI18n(
    katanaPrepareMermaidSource(request.source),
  );
  const diagramType = request.diagramType || katanaMermaidDiagramType(preparedSource.source);
  globalThis.__katanaMermaidDiagramType = diagramType;
  await katanaRegisterMermaidExternalDiagram(mermaidValue, diagramType);
  const result = await mermaidValue.render(request.svgId, preparedSource.source);
  return katanaNormalizeMermaidSvg(
    katanaRestoreMermaidI18nText(result.svg, preparedSource.replacements),
    request,
  );
}

function katanaMermaidGlobal() {
  const mermaidValue = [globalThis.mermaid, globalThis.window.mermaid].find(Boolean);
  if (!mermaidValue) {
    throw new Error("Mermaid global was not registered");
  }
  return mermaidValue;
}

async function katanaRegisterMermaidExternalDiagram(mermaidValue, diagramType) {
  if (diagramType !== "zenuml") {
    return;
  }
  const zenumlDiagram = globalThis.__katanaMermaidZenuml;
  if (!zenumlDiagram) {
    throw new Error("ZenUML runtime asset was not registered");
  }
  if (typeof mermaidValue.registerExternalDiagrams !== "function") {
    throw new Error("Mermaid runtime cannot register external diagrams");
  }
  await mermaidValue.registerExternalDiagrams([zenumlDiagram]);
}

function katanaMermaidThemeVariables(request) {
  return {
    background: request.background,
    mainBkg: request.fill,
    primaryColor: request.fill,
    primaryTextColor: request.text,
    primaryBorderColor: request.stroke,
    secondaryColor: request.fill,
    secondaryTextColor: request.text,
    secondaryBorderColor: request.stroke,
    tertiaryColor: request.fill,
    tertiaryTextColor: request.text,
    tertiaryBorderColor: request.stroke,
    nodeTextColor: request.text,
    lineColor: request.arrow,
    textColor: request.text,
    edgeLabelBackground: request.fill,
    actorBkg: request.fill,
    actorTextColor: request.text,
    actorBorder: request.stroke,
    signalColor: request.arrow,
    signalTextColor: request.text,
    labelTextColor: request.text,
    noteBkgColor: request.fill,
    noteTextColor: request.text,
    noteBorderColor: request.stroke,
    clusterBkg: request.background,
    clusterBorder: request.stroke,
    titleColor: request.text,
  };
}
`;
