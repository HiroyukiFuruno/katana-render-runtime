import { SharedDomFragments } from "../shared/dom_fragments";
import {
  RuntimeBundleDefinition,
  type RuntimeFragment,
  RuntimeFragmentFactory,
} from "../shared/runtime_bundle";

const DRAWIO_JS_RUNTIME = "crates/katana-diagram-renderer/src/markdown/drawio_renderer/js_runtime";

export const DrawioRuntimeBundleDefinition = {
  create(): RuntimeBundleDefinition {
    return new RuntimeBundleDefinition(
      "drawio",
      "drawio-runtime.min.js",
      [...SharedDomFragments.all(), ...drawioRuntimeFragments()],
      DRAWIO_RUNTIME_ENTRYPOINT,
    );
  },
};

function drawioRuntimeFragments(): RuntimeFragment[] {
  return [
    RuntimeFragmentFactory.verbatim("drawio-globals.js", `${DRAWIO_JS_RUNTIME}/drawio_globals.js`),
    RuntimeFragmentFactory.verbatim(
      "drawio-resources.js",
      `${DRAWIO_JS_RUNTIME}/drawio_resources.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-stencils.js",
      `${DRAWIO_JS_RUNTIME}/drawio_stencils.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-xml-document.js",
      `${DRAWIO_JS_RUNTIME}/drawio_xml_document.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-mx-adapter.js",
      `${DRAWIO_JS_RUNTIME}/drawio_mx_adapter.js`,
    ),
    RuntimeFragmentFactory.transformed(
      "drawio-runtime-adapter.js",
      `${DRAWIO_JS_RUNTIME}/drawio_runtime_adapter.js`,
      "drawio-runtime-adapter-without-installer",
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-svg-source-crop.js",
      `${DRAWIO_JS_RUNTIME}/drawio_svg_source_crop.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-svg-crop.js",
      `${DRAWIO_JS_RUNTIME}/drawio_svg_crop.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-svg-source-styles.js",
      `${DRAWIO_JS_RUNTIME}/drawio_svg_source_styles.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-svg-light-dark.js",
      `${DRAWIO_JS_RUNTIME}/drawio_svg_light_dark.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-svg-paint.js",
      `${DRAWIO_JS_RUNTIME}/drawio_svg_paint.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-svg-color-context.js",
      `${DRAWIO_JS_RUNTIME}/drawio_svg_color_context.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-svg-azure-colors.js",
      `${DRAWIO_JS_RUNTIME}/drawio_svg_azure_colors.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-svg-colors.js",
      `${DRAWIO_JS_RUNTIME}/drawio_svg_colors.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-svg-gcp-colors.js",
      `${DRAWIO_JS_RUNTIME}/drawio_svg_gcp_colors.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-svg-shape-colors.js",
      `${DRAWIO_JS_RUNTIME}/drawio_svg_shape_colors.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-svg-style-color-tables.js",
      `${DRAWIO_JS_RUNTIME}/drawio_svg_style_color_tables.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-svg-style-color-parse.js",
      `${DRAWIO_JS_RUNTIME}/drawio_svg_style_color_parse.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-svg-style-colors.js",
      `${DRAWIO_JS_RUNTIME}/drawio_svg_style_colors.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-svg-shadow.js",
      `${DRAWIO_JS_RUNTIME}/drawio_svg_shadow.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-svg-labels.js",
      `${DRAWIO_JS_RUNTIME}/drawio_svg_labels.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-svg-rich-text-labels.js",
      `${DRAWIO_JS_RUNTIME}/drawio_svg_rich_text_labels.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-svg-label-clip.js",
      `${DRAWIO_JS_RUNTIME}/drawio_svg_label_clip.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-svg-label-wrap.js",
      `${DRAWIO_JS_RUNTIME}/drawio_svg_label_wrap.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-svg-html-text-styles.js",
      `${DRAWIO_JS_RUNTIME}/drawio_svg_html_text_styles.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-svg-html-text-labels.js",
      `${DRAWIO_JS_RUNTIME}/drawio_svg_html_text_labels.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-svg-html-tables.js",
      `${DRAWIO_JS_RUNTIME}/drawio_svg_html_tables.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-svg-text-baseline.js",
      `${DRAWIO_JS_RUNTIME}/drawio_svg_text_baseline.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-svg-foreign-object.js",
      `${DRAWIO_JS_RUNTIME}/drawio_svg_foreign_object.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-svg-geometry.js",
      `${DRAWIO_JS_RUNTIME}/drawio_svg_geometry.js`,
    ),
    RuntimeFragmentFactory.verbatim(
      "drawio-svg-postprocess.js",
      `${DRAWIO_JS_RUNTIME}/drawio_svg_postprocess.js`,
    ),
    RuntimeFragmentFactory.transformed(
      "render-drawio.js",
      `${DRAWIO_JS_RUNTIME}/render_drawio.js`,
      "drawio-render-without-auto-call",
    ),
  ];
}

const DRAWIO_RUNTIME_ENTRYPOINT = `
function katanaRunDrawioRuntime() {
  katanaInstallDrawioRuntimeAdapter();
  return katanaRenderDrawioSvg();
}
`;
