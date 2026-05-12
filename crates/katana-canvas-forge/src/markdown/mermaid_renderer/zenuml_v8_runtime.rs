use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::diagram_js_runtime::{DiagramRuntimeScript, DiagramV8Runtime};
use crate::markdown::runtime_assets::RuntimeAsset;

const BROWSER_GLOBALS: &str = include_str!("js_runtime/browser_globals.js");
const DOM_NODE: &str = include_str!("js_runtime/dom_node.js");
const DOM_STYLE: &str = include_str!("js_runtime/dom_style.js");
const DOM_HTML: &str = include_str!("js_runtime/dom_html.js");
const DOM_NODE_HTML: &str = include_str!("js_runtime/dom_node_html.js");
const DOM_SELECTOR: &str = include_str!("js_runtime/dom_selector.js");
const DOM_DOCUMENT: &str = include_str!("js_runtime/dom_document.js");
const DOM_TREE: &str = include_str!("js_runtime/dom_tree.js");
const DOM_LAYOUT: &str = include_str!("js_runtime/dom_layout.js");
const DOM_METRICS: &str = include_str!("js_runtime/dom_metrics.js");
const DOM_LAYOUT_PROTOTYPE: &str = include_str!("js_runtime/dom_layout_prototype.js");
const DOM_CSS_STYLE: &str = include_str!("js_runtime/dom_css_style.js");
const DOM_COMPUTED_STYLE: &str = include_str!("js_runtime/dom_computed_style.js");
const DOM_MEASURE: &str = include_str!("js_runtime/dom_measure.js");
const DOM_MEASURE_TEXT_LINES: &str = include_str!("js_runtime/dom_measure_text_lines.js");
const DOM_TEXT_METRICS: &str = include_str!("js_runtime/dom_text_metrics.js");
const DOM_TEXT_MEASURE: &str = include_str!("js_runtime/dom_text_measure.js");
const BRIDGE_SCRIPT: &str = include_str!("js_runtime/render_zenuml_v8.js");

pub(super) struct ZenumlV8RenderOps;

impl ZenumlV8RenderOps {
    pub(super) fn render(
        source: &str,
        preset: &DiagramColorPreset,
        _svg_id: String,
    ) -> Result<String, String> {
        let zenuml_asset = RuntimeAsset::zenuml_core();
        let zenuml_bundle = materialize_and_read(&zenuml_asset)?;
        let preamble = build_preamble(source, preset.dark_mode);
        let mut scripts = dom_scripts();
        scripts.push(DiagramRuntimeScript::owned("zenuml-preamble.js", preamble));
        scripts.push(DiagramRuntimeScript::owned("zenuml.js", zenuml_bundle));
        scripts.push(DiagramRuntimeScript::borrowed(
            "render-zenuml-v8.js",
            BRIDGE_SCRIPT,
        ));
        DiagramV8Runtime::render(&scripts)
    }
}

fn dom_scripts<'a>() -> Vec<DiagramRuntimeScript<'a>> {
    vec![
        DiagramRuntimeScript::borrowed("browser-globals.js", BROWSER_GLOBALS),
        DiagramRuntimeScript::borrowed("dom-node.js", DOM_NODE),
        DiagramRuntimeScript::borrowed("dom-style.js", DOM_STYLE),
        DiagramRuntimeScript::borrowed("dom-html.js", DOM_HTML),
        DiagramRuntimeScript::borrowed("dom-node-html.js", DOM_NODE_HTML),
        DiagramRuntimeScript::borrowed("dom-selector.js", DOM_SELECTOR),
        DiagramRuntimeScript::borrowed("dom-document.js", DOM_DOCUMENT),
        DiagramRuntimeScript::borrowed("dom-tree.js", DOM_TREE),
        DiagramRuntimeScript::borrowed("dom-layout.js", DOM_LAYOUT),
        DiagramRuntimeScript::borrowed("dom-metrics.js", DOM_METRICS),
        DiagramRuntimeScript::borrowed("dom-layout-prototype.js", DOM_LAYOUT_PROTOTYPE),
        DiagramRuntimeScript::borrowed("dom-css-style.js", DOM_CSS_STYLE),
        DiagramRuntimeScript::borrowed("dom-computed-style.js", DOM_COMPUTED_STYLE),
        DiagramRuntimeScript::borrowed("dom-measure.js", DOM_MEASURE),
        DiagramRuntimeScript::borrowed("dom-measure-text-lines.js", DOM_MEASURE_TEXT_LINES),
        DiagramRuntimeScript::borrowed("dom-text-metrics.js", DOM_TEXT_METRICS),
        DiagramRuntimeScript::borrowed("dom-text-measure.js", DOM_TEXT_MEASURE),
    ]
}

fn materialize_and_read(asset: &RuntimeAsset) -> Result<String, String> {
    let path = asset.materialize_at(asset.materialized_path())?;
    read_asset_file(&path)
}

fn read_asset_file(path: &std::path::Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("Failed to read zenuml.js: {e}"))
}

fn build_preamble(source: &str, is_dark: bool) -> String {
    let source_json = serde_json::Value::String(source.to_owned()).to_string();
    format!("var __zenuml_source__ = {source_json};\nvar __zenuml_dark__ = {is_dark};")
}

#[cfg(test)]
#[path = "zenuml_v8_runtime_tests.rs"]
mod tests;
