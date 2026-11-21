use super::diagram_type::MermaidDiagramType;
use super::js_runtime_scripts::MermaidRuntimeScripts;
use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::diagram_js_runtime::DiagramV8Runtime;
#[cfg(test)]
use crate::markdown::runtime_resource_loader::{
    BundledResource, RuntimeResourceLoader as runtime_resource_loader,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
#[cfg(test)]
use std::path::Path;

pub(super) struct MermaidJsRuntimeOps;

impl MermaidJsRuntimeOps {
    #[cfg(test)]
    pub(super) fn render(
        source: &str,
        mermaid_js: &Path,
        preset: &DiagramColorPreset,
    ) -> Result<String, String> {
        Self::render_with_origin(source, mermaid_js, preset, false)
    }

    #[cfg(test)]
    pub(super) fn render_with_origin(
        source: &str,
        mermaid_js: &Path,
        preset: &DiagramColorPreset,
        bundled: bool,
    ) -> Result<String, String> {
        let bundle =
            runtime_resource_loader::read(mermaid_js, bundled.then_some(BundledResource::Mermaid))
                .map_err(|error| format!("Failed to read Mermaid.js bundle: {error}"))?;
        Self::render_bundle(source, &bundle, preset)
    }

    pub(super) fn render_bundle(
        source: &str,
        bundle: &str,
        preset: &DiagramColorPreset,
    ) -> Result<String, String> {
        let request = MermaidRenderRequest::new(source, preset);
        let request_json = request.to_json_value().to_string();
        let scripts = MermaidRuntimeScripts::build(bundle, &request_json, request.diagram_type);
        tracing::event!(
            target: "katana_render_runtime::mermaid_classic_capture",
            tracing::Level::TRACE,
            phase = "request",
            request_json = request_json.as_str(),
            script_count = scripts.len(),
        );
        let svg = DiagramV8Runtime::render(&scripts)?;
        rendered_svg(svg)
    }
}

struct MermaidRenderRequest<'a> {
    source: &'a str,
    svg_id: String,
    theme: &'a str,
    background: &'a str,
    fill: &'a str,
    text: &'a str,
    stroke: &'a str,
    arrow: &'a str,
    diagram_type: MermaidDiagramType,
}

impl<'a> MermaidRenderRequest<'a> {
    fn new(source: &'a str, preset: &'a DiagramColorPreset) -> Self {
        Self {
            source,
            svg_id: Self::svg_id(source, preset),
            theme: preset.mermaid_theme.as_ref(),
            background: preset.background.as_ref(),
            fill: preset.fill.as_ref(),
            text: preset.text.as_ref(),
            stroke: preset.stroke.as_ref(),
            arrow: preset.arrow.as_ref(),
            diagram_type: MermaidDiagramType::from_source(source),
        }
    }

    fn svg_id(source: &str, preset: &DiagramColorPreset) -> String {
        let mut hasher = DefaultHasher::new();
        "mermaid-svg-id-v1".hash(&mut hasher);
        super::hash_mermaid_source_and_preset(&mut hasher, source, preset);
        format!("katana-mermaid-svg-{:016x}", hasher.finish())
    }

    fn to_json_value(&self) -> serde_json::Value {
        serde_json::json!({
            "source": self.source,
            "svgId": self.svg_id,
            "theme": self.theme,
            "background": self.background,
            "fill": self.fill,
            "text": self.text,
            "stroke": self.stroke,
            "arrow": self.arrow,
            "diagramType": self.diagram_type.request_value(),
        })
    }
}

#[cfg(test)]
fn read_mermaid_bundle(mermaid_js: &Path) -> Result<String, String> {
    runtime_resource_loader::read_file(mermaid_js)
        .map_err(|err| format!("Failed to read Mermaid.js bundle: {err}"))
}

fn ensure_svg(svg: &str) -> Result<(), String> {
    if svg.contains("<svg") && svg.contains("</svg>") {
        return Ok(());
    }
    Err("Mermaid.js did not return SVG markup".to_string())
}

fn rendered_svg(svg: String) -> Result<String, String> {
    ensure_svg(&svg)?;
    Ok(svg)
}

#[cfg(test)]
#[path = "js_runtime_dominant_baseline_tests.rs"]
mod dominant_baseline_tests;
#[cfg(test)]
#[path = "js_runtime_ishikawa_marker_tests.rs"]
mod ishikawa_marker_tests;
#[cfg(test)]
#[path = "js_runtime_native_width_tests.rs"]
mod native_width_tests;
#[cfg(test)]
#[path = "js_runtime_reference_score_tests.rs"]
mod reference_score_tests;
#[cfg(test)]
#[path = "js_runtime_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "js_runtime_geometry_contract.rs"]
mod geometry_contract;
