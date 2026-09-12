use super::stencil_test_support::{
    fake_bundle_with_cisco_placeholders, fake_bundle_with_unresolved_stencil_color,
};
use super::test_support::{
    OFFICIAL_REFERENCE_VIEWPORT_BUNDLE_HOOK, fake_bundle, temp_runtime_path,
};
use super::{
    DrawioJsRuntimeOps, DrawioRenderRequest, RuntimeBundleCache, ensure_svg, lock_cache,
    read_drawio_bundle, read_drawio_bundle_with_cache, rendered_svg,
};
use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::runtime_assets::RuntimeAsset;
use std::collections::HashMap;
use std::sync::Mutex;

const NAMESPACED_DESCENDANTS_BUNDLE: &str = r#"
function Graph() {}
const mxUtils = {};
const Editor = { convertHtmlToText(value) { return String(value); } };
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svgNamespace = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(svgNamespace, "svg");
  svg.setAttribute("width", "20");
  svg.setAttribute("height", "10");
  svg.setAttribute("viewBox", "0 0 20 10");
  const group = document.createElementNS(svgNamespace, "g");
  const foreignObject = document.createElementNS(svgNamespace, "foreignObject");
  const htmlForeignObject = document.createElement("foreignObject");
  group.appendChild(foreignObject);
  group.appendChild(htmlForeignObject);
  svg.appendChild(group);
  const xml = mxUtils.createXmlDocument();
  const xmlRoot = xml.createElementNS(svgNamespace, "svg");
  xmlRoot.appendChild(xml.createElementNS(svgNamespace, "foreignObject"));
  xmlRoot.appendChild(xml.createElement("foreignObject"));
  xml.appendChild(xmlRoot);
  svg.setAttribute("data-node-namespace-count", group.getElementsByTagNameNS(svgNamespace, "foreignObject").length);
  svg.setAttribute("data-document-namespace-count", xml.getElementsByTagNameNS(svgNamespace, "foreignObject").length);
  svg.setAttribute("data-any-namespace-count", group.getElementsByTagNameNS("*", "foreignObject").length);
  svg.setAttribute("data-any-local-name-count", group.getElementsByTagNameNS(svgNamespace, "*").length);
  svg.setAttribute("data-all-descendant-count", group.getElementsByTagNameNS("*", "*").length);
  callback({ graph: { getSvg() { return svg; } } });
};
"#;

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
fn fake_bundle_finds_namespaced_foreign_object_descendants() {
    let path = temp_runtime_path("krr-drawio-namespaced-descendants");
    assert!(std::fs::write(&path, NAMESPACED_DESCENDANTS_BUNDLE).is_ok());

    let rendered =
        DrawioJsRuntimeOps::render("<mxGraphModel />", &path, DiagramColorPreset::light());

    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            svg.contains(r#"data-node-namespace-count="1""#)
                && svg.contains(r#"data-document-namespace-count="1""#)
                && svg.contains(r#"data-any-namespace-count="1""#)
                && svg.contains(r#"data-any-local-name-count="1""#)
                && svg.contains(r#"data-all-descendant-count="2""#)
        }),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_uses_official_reference_viewport() {
    let path = temp_runtime_path("kdr-drawio-reference-viewport-unit");
    let bundle = fake_bundle()
        .replace(
            "GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {",
            r#"GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const initialContainer = `${_container.clientWidth}x${_container.clientHeight}`;"#,
        )
        .replace(r#"  svg.setAttribute("width", "20");"#, r#"  svg.setAttribute("width", "1600");"#)
        .replace(
            r#"  svg.setAttribute("viewBox", "0 0 20 10");"#,
            OFFICIAL_REFERENCE_VIEWPORT_BUNDLE_HOOK,
        );
    assert!(std::fs::write(&path, bundle).is_ok());

    let rendered =
        DrawioJsRuntimeOps::render("<mxGraphModel />", &path, DiagramColorPreset::light());

    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            svg.contains(r#"data-viewport="1520x845""#)
                && svg.contains(r#"data-initial-container="0x0""#)
                && svg.contains(r#"data-constrained-container="1496x10""#)
                && svg.contains(r#"data-min-width-container="1496x665""#)
                && svg.contains(r#"data-explicit-container="1126x665""#)
        }),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_resolves_cisco_stencil_placeholder_colors() {
    let path = temp_runtime_path("kdr-drawio-cisco-placeholder-unit");
    assert!(std::fs::write(&path, fake_bundle_with_cisco_placeholders()).is_ok());

    let source = r##"<mxGraphModel><root><mxCell id="cisco" style="shape=mxgraph.cisco.misc.access_point;html=1;fillColor=#10739E;strokeColor=#ffffff;" vertex="1" /></root></mxGraphModel>"##;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            svg.contains(r##"fill="#54a9ce""##)
                && svg.contains(r##"stroke="#ffffff""##)
                && svg.contains(r##"stroke="#121212""##)
                && !svg.contains("light-dark(fillcolor")
        }),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_uses_stencil_default_for_unresolved_placeholder() {
    let path = temp_runtime_path("kdr-drawio-stencil-default-color-unit");
    assert!(std::fs::write(&path, fake_bundle_with_unresolved_stencil_color()).is_ok());

    let source = r##"<mxGraphModel><root><mxCell id="salesforce" style="shape=mxgraph.salesforce.web2;html=1;fillColor=#e5e5e5;" vertex="1" /></root></mxGraphModel>"##;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r##"fill="#032d60""##) && !svg.contains("fillcolor2")),
        "{rendered:?}"
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
fn request_fields_come_from_preset_not_global_state() -> Result<(), String> {
    DiagramColorPreset::set_dark_mode(true);
    let request = DrawioRenderRequest::new("<mxGraphModel />", DiagramColorPreset::light())?;

    assert!(!request.dark_mode);
    assert_eq!(request.background, "transparent");
    Ok(())
}

#[test]
fn cached_remote_image_is_embedded_from_packaged_resources() -> Result<(), String> {
    const URL: &str = "https://upload.wikimedia.org/wikipedia/de/8/89/FirefoxLogo.svg";
    let path = temp_runtime_path("krr-drawio-cached-remote-image");
    RuntimeAsset::drawio().materialize_at(path.clone())?;
    let source = format!(
        r#"<mxGraphModel><root><mxCell id="0"/><mxCell id="1" parent="0"/><mxCell id="2" style="shape=image;image={URL}" vertex="1" parent="1"><mxGeometry width="230" height="220" as="geometry"/></mxCell></root></mxGraphModel>"#
    );

    let rendered = DrawioJsRuntimeOps::render(&source, &path, DiagramColorPreset::dark());
    let _ = std::fs::remove_file(path);

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| { svg.contains("data:image/svg+xml;base64,") && !svg.contains(URL) }),
        "{rendered:?}"
    );
    Ok(())
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
