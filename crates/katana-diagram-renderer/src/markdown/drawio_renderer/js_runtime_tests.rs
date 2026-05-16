use super::{
    DrawioJsRuntimeOps, DrawioRenderRequest, RuntimeBundleCache, ensure_svg, lock_cache,
    read_drawio_bundle, read_drawio_bundle_with_cache, rendered_svg,
};
use crate::markdown::color_preset::DiagramColorPreset;
use std::collections::HashMap;
use std::sync::Mutex;

#[test]
fn bundle_cache_reads_once() {
    let path = temp_runtime_path("kdr-drawio-runtime-unit");
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
    let path = temp_runtime_path("kdr-drawio-runtime-validation-unit");
    assert!(std::fs::write(&path, "function GraphViewer() {}").is_ok());
    assert!(read_drawio_bundle(&path).is_ok());
    assert!(read_drawio_bundle(&path).is_ok());
    assert!(ensure_svg("plain text").is_err());
    assert!(read_drawio_bundle(std::path::Path::new("target/kdr-tests/missing.js")).is_err());
}

#[test]
fn fake_bundle_renders_svg() {
    let path = temp_runtime_path("kdr-drawio-render-unit");
    assert!(std::fs::write(&path, fake_bundle()).is_ok());

    let rendered =
        DrawioJsRuntimeOps::render("<mxGraphModel />", &path, DiagramColorPreset::light());

    assert!(rendered.as_ref().is_ok_and(|svg| svg.contains("<svg")));
}

#[test]
fn fake_bundle_preserves_html_text_foreign_object() {
    let path = temp_runtime_path("kdr-drawio-html-label-unit");
    assert!(std::fs::write(&path, fake_bundle_with_foreign_object()).is_ok());

    let rendered =
        DrawioJsRuntimeOps::render("<mxGraphModel />", &path, DiagramColorPreset::light());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("<foreignObject"))
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"<div xmlns="http://www.w3.org/1999/xhtml""#))
    );
}

#[test]
fn fake_bundle_resolves_cisco_stencil_placeholder_colors() {
    let path = temp_runtime_path("kdr-drawio-cisco-placeholder-unit");
    assert!(std::fs::write(&path, fake_bundle_with_cisco_placeholders()).is_ok());

    let source = r##"<mxGraphModel><root><mxCell id="cisco" style="shape=mxgraph.cisco.misc.access_point;html=1;fillColor=#10739E;strokeColor=#ffffff;" vertex="1" /></root></mxGraphModel>"##;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r##"fill="#54a9ce""##))
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r##"stroke="#ededed""##))
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r##"stroke="#121212""##))
    );
}

#[test]
fn fake_bundle_reports_runtime_error() {
    let path = temp_runtime_path("kdr-drawio-runtime-error-unit");
    assert!(std::fs::write(&path, "window.GraphViewer = {};").is_ok());

    let rendered =
        DrawioJsRuntimeOps::render("<mxGraphModel />", &path, DiagramColorPreset::light());

    assert!(rendered.is_err());
}

#[test]
fn render_reports_missing_bundle_through_surface_path() {
    let result = DrawioJsRuntimeOps::render(
        "<mxGraphModel />",
        std::path::Path::new("target/kdr-tests/missing-drawio-render.js"),
        DiagramColorPreset::dark(),
    );

    assert!(result.is_err());
}

#[test]
fn request_fields_come_from_preset_not_global_state() {
    DiagramColorPreset::set_dark_mode(true);
    let request = DrawioRenderRequest::new("<mxGraphModel />", DiagramColorPreset::light());

    assert!(!request.dark_mode);
    assert_eq!(request.background, "transparent");
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

fn fake_bundle_with_foreign_object() -> &'static str {
    FAKE_BUNDLE_WITH_FOREIGN_OBJECT
}

const FAKE_BUNDLE_WITH_FOREIGN_OBJECT: &str = r#"
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
  const foreignObject = document.createElementNS("http://www.w3.org/2000/svg", "foreignObject");
  foreignObject.setAttribute("width", "100%");
  foreignObject.setAttribute("height", "100%");
  const div = document.createElement("div");
  div.textContent = "html label";
  div.appendChild(document.createElement("br"));
  div.appendChild(document.createElement("hr"));
  foreignObject.appendChild(div);
  svg.appendChild(foreignObject);
  callback({
    graph: {
      getSvg() {
        return svg;
      },
    },
  });
};
"#;

fn fake_bundle_with_cisco_placeholders() -> &'static str {
    FAKE_BUNDLE_WITH_CISCO_PLACEHOLDERS
}

const FAKE_BUNDLE_WITH_CISCO_PLACEHOLDERS: &str = r#"
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
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "cisco");
  const fillPath = document.createElementNS("http://www.w3.org/2000/svg", "path");
  fillPath.setAttribute("fill", "fillcolor");
  group.appendChild(fillPath);
  const secondaryFillStroke = document.createElementNS("http://www.w3.org/2000/svg", "path");
  secondaryFillStroke.setAttribute("fill", "none");
  secondaryFillStroke.setAttribute("stroke", "fillcolor2");
  group.appendChild(secondaryFillStroke);
  const secondaryStroke = document.createElementNS("http://www.w3.org/2000/svg", "path");
  secondaryStroke.setAttribute("fill", "none");
  secondaryStroke.setAttribute("stroke", "strokecolor2");
  group.appendChild(secondaryStroke);
  svg.appendChild(group);
  callback({
    graph: {
      getSvg() {
        return svg;
      },
    },
  });
};
"#;
