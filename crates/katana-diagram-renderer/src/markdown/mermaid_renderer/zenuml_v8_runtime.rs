use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::diagram_js_runtime::{DiagramRuntimeScript, DiagramV8Runtime};
use crate::markdown::runtime_assets::RuntimeAsset;

const ZENUML_RUNTIME: &str = include_str!("../diagram_runtime/generated/zenuml-runtime.min.js");

pub(super) struct ZenumlV8RenderOps;

impl ZenumlV8RenderOps {
    pub(super) fn render(
        source: &str,
        preset: &DiagramColorPreset,
        _svg_id: String,
    ) -> Result<String, String> {
        let zenuml_asset = RuntimeAsset::zenuml_core();
        let zenuml_bundle = materialize_and_read(&zenuml_asset)?;
        let scripts = vec![
            DiagramRuntimeScript::borrowed("zenuml-runtime.min.js", ZENUML_RUNTIME),
            DiagramRuntimeScript::owned("zenuml.js", zenuml_bundle),
            DiagramRuntimeScript::owned(
                "render-zenuml.js",
                render_script(source, preset.dark_mode),
            ),
        ];
        DiagramV8Runtime::render(&scripts)
    }
}

fn materialize_and_read(asset: &RuntimeAsset) -> Result<String, String> {
    let path = asset.materialize_at(asset.materialized_path())?;
    read_asset_file(&path)
}

fn read_asset_file(path: &std::path::Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("Failed to read zenuml.js: {e}"))
}

fn render_script(source: &str, is_dark: bool) -> String {
    let source_json = serde_json::Value::String(source.to_owned()).to_string();
    format!("katanaRunZenumlRuntime({source_json}, {is_dark});")
}

#[cfg(test)]
#[path = "zenuml_v8_runtime_tests.rs"]
mod tests;
