use super::js_runtime_resources::{DrawioResource, DrawioResourceCatalog};
use super::js_runtime_scripts::DrawioRuntimeScripts;
use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::diagram_js_runtime::DiagramV8Runtime;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

type RuntimeBundleCache = Mutex<HashMap<PathBuf, Arc<str>>>;
type RuntimeBundleGuard<'a> = MutexGuard<'a, HashMap<PathBuf, Arc<str>>>;

static BUNDLE_CACHE: OnceLock<RuntimeBundleCache> = OnceLock::new();

pub(super) struct DrawioJsRuntimeOps;

impl DrawioJsRuntimeOps {
    pub(super) fn render(
        source: &str,
        drawio_js: &Path,
        preset: &DiagramColorPreset,
    ) -> Result<String, String> {
        let bundle = read_drawio_bundle(drawio_js)?;
        let request = DrawioRenderRequest::new(source, preset);
        let request_json = request.to_json_value().to_string();
        let scripts = DrawioRuntimeScripts::build(&bundle, &request_json);
        let svg = DiagramV8Runtime::render(&scripts)?;
        rendered_svg(svg)
    }
}

struct DrawioRenderRequest<'a> {
    source: &'a str,
    dark_mode: bool,
    background: &'a str,
    resources: Vec<DrawioResource>,
}

impl<'a> DrawioRenderRequest<'a> {
    fn new(source: &'a str, preset: &'a DiagramColorPreset) -> Self {
        Self {
            source,
            dark_mode: DiagramColorPreset::is_dark_mode(),
            background: preset.background,
            resources: DrawioResourceCatalog::builtin(source),
        }
    }

    fn to_json_value(&self) -> serde_json::Value {
        let resources = self.resources.iter().map(resource_json).collect::<Vec<_>>();
        serde_json::json!({
            "source": self.source,
            "dark_mode": self.dark_mode,
            "background": self.background,
            "resources": resources,
        })
    }
}

fn resource_json(resource: &DrawioResource) -> serde_json::Value {
    serde_json::json!({
        "path": resource.path,
        "mime_type": resource.mime_type,
        "content": resource.content,
        "encoding": resource.encoding.as_str(),
    })
}

fn read_drawio_bundle(drawio_js: &Path) -> Result<Arc<str>, String> {
    let cache = BUNDLE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    read_drawio_bundle_with_cache(drawio_js, cache)
}

fn read_drawio_bundle_with_cache(
    drawio_js: &Path,
    cache: &RuntimeBundleCache,
) -> Result<Arc<str>, String> {
    let path = drawio_js.to_path_buf();
    let mut bundles = lock_cache(cache)?;
    if let Some(bundle) = bundles.get(&path) {
        return Ok(bundle.clone());
    }

    let bundle = match std::fs::read_to_string(drawio_js) {
        Ok(bundle) => bundle,
        Err(error) => {
            return Err(format!("Failed to read Draw.io JavaScript bundle: {error}"));
        }
    };
    let bundle = Arc::<str>::from(bundle);
    bundles.insert(path, bundle.clone());
    Ok(bundle)
}

fn lock_cache(cache: &RuntimeBundleCache) -> Result<RuntimeBundleGuard<'_>, String> {
    match cache.lock() {
        Ok(cache) => Ok(cache),
        Err(error) => Err(error.to_string()),
    }
}

fn ensure_svg(svg: &str) -> Result<(), String> {
    if svg.contains("<svg") && svg.contains("</svg>") {
        return Ok(());
    }
    Err("Draw.io JavaScript did not return SVG markup".to_string())
}

fn rendered_svg(svg: String) -> Result<String, String> {
    match ensure_svg(&svg) {
        Ok(()) => Ok(svg),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
#[path = "js_runtime_tests.rs"]
mod tests;
