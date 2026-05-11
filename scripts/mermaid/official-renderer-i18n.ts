import os from "node:os";
import path from "node:path";

const I18N_RUNTIME_DIR = "crates/katana-canvas-forge/src/markdown/mermaid_renderer/js_runtime";
const RUNTIME_SCRIPTS = [
  "svg_bounds_attrs.js",
  "svg_path_bounds.js",
  "svg_bounds.js",
  "svg_text_bounds.js",
  "svg_architecture_icons.js",
  "svg_group_utils.js",
  "svg_viewbox_fixes.js",
  "svg_er_layout.js",
  "svg_er_fixes.js",
  "svg_class_fixes.js",
  "svg_visual_fixes.js",
  "svg_review_fixes.js",
  "svg_review_ishikawa_fixes.js",
  "svg_review_venn_fixes.js",
  "svg_review_treemap_fixes.js",
  "svg_kanban_text.js",
  "svg_kanban_viewbox.js",
  "svg_kanban_label_layout.js",
  "svg_kanban_fixes.js",
  "svg_wardley_normalizer.js",
  "svg_diagram_normalizers.js",
  "svg_dimensions.js",
  "source_i18n_context.js",
  "source_i18n_normalize.js",
  "mermaid_diagram_type.js",
  "svg_normalize.js",
];

export class MermaidRuntimeScripts {
  static paths(): string[] {
    return RUNTIME_SCRIPTS.map((filename) => path.resolve(I18N_RUNTIME_DIR, filename));
  }

  static zenumlBridgePath(): string {
    return path.resolve(I18N_RUNTIME_DIR, "mermaid_zenuml_bridge.js");
  }
}

export function expandHome(value: string) {
  return value.startsWith("~/") ? path.join(os.homedir(), value.slice(2)) : value;
}
