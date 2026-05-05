use super::{
    DrawioJsRuntimeOps, RuntimeBundleCache, ensure_svg, lock_cache, read_drawio_bundle,
    read_drawio_bundle_with_cache, rendered_svg,
};
use crate::markdown::color_preset::DiagramColorPreset;
use std::collections::HashMap;
use std::sync::Mutex;

#[test]
fn bundle_cache_reads_once() {
    let path = temp_runtime_path("kcf-drawio-runtime-unit");
    assert!(std::fs::write(&path, "function GraphViewer() {}").is_ok());

    let cache: RuntimeBundleCache = Mutex::new(HashMap::new());
    let first = read_drawio_bundle_with_cache(&path, &cache);
    assert!(matches!(first.as_deref(), Ok("function GraphViewer() {}")));
    assert!(std::fs::write(&path, "changed").is_ok());
    let second = read_drawio_bundle_with_cache(&path, &cache);
    assert!(matches!(second.as_deref(), Ok("function GraphViewer() {}")));
}

#[test]
fn bundle_reading_and_svg_validation_report_errors() {
    let path = temp_runtime_path("kcf-drawio-runtime-validation-unit");
    assert!(std::fs::write(&path, "function GraphViewer() {}").is_ok());
    assert!(read_drawio_bundle(&path).is_ok());
    assert!(read_drawio_bundle(&path).is_ok());
    assert!(ensure_svg("plain text").is_err());
    assert!(read_drawio_bundle(std::path::Path::new("target/kcf-tests/missing.js")).is_err());
}

#[test]
fn fake_bundle_renders_svg() {
    let path = temp_runtime_path("kcf-drawio-render-unit");
    assert!(std::fs::write(&path, fake_bundle()).is_ok());

    let rendered =
        DrawioJsRuntimeOps::render("<mxGraphModel />", &path, DiagramColorPreset::light());

    assert!(rendered.as_ref().is_ok_and(|svg| svg.contains("<svg")));
}

#[test]
fn fake_bundle_reports_runtime_error() {
    let path = temp_runtime_path("kcf-drawio-runtime-error-unit");
    assert!(std::fs::write(&path, "window.GraphViewer = {};").is_ok());

    let rendered =
        DrawioJsRuntimeOps::render("<mxGraphModel />", &path, DiagramColorPreset::light());

    assert!(rendered.is_err());
}

#[test]
fn render_reports_missing_bundle_through_surface_path() {
    let result = DrawioJsRuntimeOps::render(
        "<mxGraphModel />",
        std::path::Path::new("target/kcf-tests/missing-drawio-render.js"),
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
    assert!(read_drawio_bundle_with_cache(std::path::Path::new("drawio.js"), &cache).is_err());
    poison_cache(&cache);
}

fn poison_cache(cache: &RuntimeBundleCache) {
    let _guard = match cache.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    std::panic::resume_unwind(Box::new("poison drawio cache"));
}

fn temp_runtime_path(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}.js", std::process::id()))
}

fn fake_bundle() -> &'static str {
    r#"
function Graph() {}
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "20");
  svg.setAttribute("height", "10");
  svg.setAttribute("viewBox", "0 0 20 10");
  const text = document.createElementNS("http://www.w3.org/2000/svg", "text");
  text.textContent = "drawio";
  svg.appendChild(text);
  callback({
    graph: {
      getSvg() {
        return svg;
      },
    },
  });
};
"#
}
