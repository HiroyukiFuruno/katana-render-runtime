use super::diagram_type::MermaidDiagramType;
use super::js_runtime_scripts::MermaidRuntimeScripts;
use super::zenuml_v8_runtime::ZenumlV8RenderOps;
use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::diagram_js_runtime::DiagramV8Runtime;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

type RuntimeBundleCache = Mutex<HashMap<PathBuf, Arc<str>>>;
type RuntimeBundleGuard<'a> = MutexGuard<'a, HashMap<PathBuf, Arc<str>>>;

static BUNDLE_CACHE: OnceLock<RuntimeBundleCache> = OnceLock::new();

pub(super) struct MermaidJsRuntimeOps;

impl MermaidJsRuntimeOps {
    pub(super) fn render(
        source: &str,
        mermaid_js: &Path,
        preset: &DiagramColorPreset,
    ) -> Result<String, String> {
        let request = MermaidRenderRequest::new(source, preset);
        if request.diagram_type == MermaidDiagramType::Zenuml {
            return ZenumlV8RenderOps::render(source, preset, request.svg_id);
        }
        let bundle = read_mermaid_bundle(mermaid_js)?;
        let request_json = request.to_json_value().to_string();
        let scripts = MermaidRuntimeScripts::build(&bundle, &request_json);
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
        source.hash(&mut hasher);
        preset.mermaid_theme.hash(&mut hasher);
        preset.background.hash(&mut hasher);
        preset.text.hash(&mut hasher);
        preset.fill.hash(&mut hasher);
        preset.stroke.hash(&mut hasher);
        preset.arrow.hash(&mut hasher);
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

fn read_mermaid_bundle(mermaid_js: &Path) -> Result<Arc<str>, String> {
    let cache = BUNDLE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let path = mermaid_js.to_path_buf();
    if let Some(bundle) = lock_cache(cache)?.get(&path) {
        return Ok(bundle.clone());
    }

    let bundle = std::fs::read_to_string(mermaid_js)
        .map_err(|err| format!("Failed to read Mermaid.js bundle: {err}"))?;
    let bundle = Arc::<str>::from(bundle);
    lock_cache(cache)?.insert(path, bundle.clone());
    Ok(bundle)
}

fn lock_cache(cache: &RuntimeBundleCache) -> Result<RuntimeBundleGuard<'_>, String> {
    cache.lock().map_err(|err| err.to_string())
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
#[path = "js_runtime_tests.rs"]
mod tests;
