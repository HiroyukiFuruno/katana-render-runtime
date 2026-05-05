use super::js_runtime_scripts::MermaidRuntimeScripts;
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
        let bundle = read_mermaid_bundle(mermaid_js)?;
        let request = MermaidRenderRequest::new(source, preset);
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
}

impl<'a> MermaidRenderRequest<'a> {
    fn new(source: &'a str, preset: &'a DiagramColorPreset) -> Self {
        Self {
            source,
            svg_id: Self::svg_id(source, preset),
            theme: preset.mermaid_theme,
            background: preset.background,
            fill: preset.fill,
            text: preset.text,
            stroke: preset.stroke,
            arrow: preset.arrow,
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
mod tests {
    use super::{
        MermaidJsRuntimeOps, MermaidRenderRequest, RuntimeBundleCache, ensure_svg, lock_cache,
        read_mermaid_bundle, rendered_svg,
    };
    use crate::markdown::color_preset::DiagramColorPreset;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[test]
    fn fake_bundle_renders_svg_and_rejects_non_svg_output() {
        let path = std::env::temp_dir().join(format!(
            "kcf-mermaid-runtime-unit-{}.js",
            std::process::id()
        ));
        assert!(std::fs::write(&path, fake_bundle()).is_ok());
        assert!(read_mermaid_bundle(&path).is_ok());
        assert!(read_mermaid_bundle(&path).is_ok());

        let rendered =
            MermaidJsRuntimeOps::render("graph TD; A-->B", &path, DiagramColorPreset::dark());
        assert!(rendered.as_ref().is_ok_and(|svg| svg.contains("<svg")));
        assert!(ensure_svg("plain text").is_err());
        assert!(read_mermaid_bundle(std::path::Path::new("target/kcf-tests/missing.js")).is_err());
    }

    #[test]
    fn render_reports_missing_bundle_through_surface_path() {
        let result = MermaidJsRuntimeOps::render(
            "graph TD; A-->B",
            std::path::Path::new("target/kcf-tests/missing-mermaid-render.js"),
            DiagramColorPreset::dark(),
        );

        assert!(result.is_err());
    }

    #[test]
    fn rendered_svg_rejects_plain_text_from_runtime() {
        assert!(rendered_svg("plain text".to_string()).is_err());
    }

    #[test]
    fn poisoned_cache_reports_lock_error() {
        let cache: RuntimeBundleCache = Mutex::new(HashMap::new());
        let poison = std::panic::catch_unwind(|| poison_cache(&cache));

        assert!(poison.is_err());
        assert!(lock_cache(&cache).is_err());
        poison_cache(&cache);
    }

    fn poison_cache(cache: &RuntimeBundleCache) {
        let _guard = match cache.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        std::panic::resume_unwind(Box::new("poison mermaid cache"));
    }

    #[test]
    fn request_id_changes_with_theme_and_source() {
        let dark = MermaidRenderRequest::new("graph TD; A-->B", DiagramColorPreset::dark());
        let light = MermaidRenderRequest::new("graph TD; A-->B", DiagramColorPreset::light());
        let other = MermaidRenderRequest::new("graph TD; B-->C", DiagramColorPreset::dark());

        assert!(dark.svg_id.starts_with("katana-mermaid-svg-"));
        assert_ne!(dark.svg_id, light.svg_id);
        assert_ne!(dark.svg_id, other.svg_id);
    }

    fn fake_bundle() -> &'static str {
        r#"
globalThis.mermaid = {
  initialize() {},
  render: async (id, source) => ({
    svg: `<svg xmlns="http://www.w3.org/2000/svg" id="${id}" width="20" height="10" viewBox="0 0 20 10"><text>${source}</text></svg>`
  })
};
"#
    }
}
