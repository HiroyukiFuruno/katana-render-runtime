use super::DrawioJsRuntimeOps;
use crate::markdown::color_preset::DiagramColorPreset;

#[test]
fn fake_bundle_preserves_unmatched_disabled_page_svg_bounds() {
    let path = temp_runtime_path("kdr-drawio-disabled-page-bounds-unit");
    assert!(std::fs::write(&path, fake_bundle_with_negative_disabled_page_bounds()).is_ok());

    let source = r#"<mxGraphModel page="0"><root><mxCell id="shape" parent="1" vertex="1"><mxGeometry x="10" y="880" width="246" height="60" as="geometry"/></mxCell></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"width="1571px""#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"height="512px""#))
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| !svg.contains(r#"translate(-121,62)"#))
    );
}

#[test]
fn fake_bundle_preserves_enabled_page_svg_bounds() {
    let path = temp_runtime_path("kdr-drawio-enabled-page-bounds-unit");
    assert!(std::fs::write(&path, fake_bundle_with_negative_disabled_page_bounds()).is_ok());

    let source = r#"<mxGraphModel page="1"><root><mxCell id="shape" parent="1" vertex="1"><mxGeometry x="10" y="880" width="246" height="60" as="geometry"/></mxCell></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"width="1571px""#))
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"height="512px""#))
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| !svg.contains(r#"translate(-121,62)"#))
    );
}

#[test]
fn fake_bundle_prefers_negative_source_crop_box() {
    let path = temp_runtime_path("kdr-drawio-negative-source-crop-unit");
    assert!(std::fs::write(&path, fake_bundle_with_rendered_overflow()).is_ok());

    let source = r#"<mxGraphModel page="0"><root><mxCell id="1" parent="0"/><mxCell id="phone" parent="1" vertex="1"><mxGeometry x="-560" y="-490" width="390" height="780" as="geometry"/></mxCell></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"width="391px""#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"height="781px""#)),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_preserves_positive_top_source_crop_padding() {
    let path = temp_runtime_path("kdr-drawio-source-top-padding-unit");
    assert!(std::fs::write(&path, fake_bundle_with_positive_top_padding()).is_ok());

    let source = r#"<mxGraphModel page="0"><root><mxCell id="1" parent="0"/><mxCell id="shape" parent="1" vertex="1"><mxGeometry x="0" y="72" width="100" height="100" as="geometry"/></mxCell></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"height="111px""#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| !svg.contains(r#"translate(0,-10)"#)),
        "{rendered:?}"
    );
}

fn temp_runtime_path(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}.js", std::process::id()))
}

fn fake_bundle_with_negative_disabled_page_bounds() -> &'static str {
    FAKE_BUNDLE_WITH_NEGATIVE_DISABLED_PAGE_BOUNDS
}

const FAKE_BUNDLE_WITH_NEGATIVE_DISABLED_PAGE_BOUNDS: &str = r#"
function Graph() {}
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "1571px");
  svg.setAttribute("height", "512px");
  svg.setAttribute("viewBox", "0 0 1571 512");
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "shape");
  const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
  path.setAttribute("d", "M 121 -62 L 123 184");
  group.appendChild(path);
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

fn fake_bundle_with_positive_top_padding() -> &'static str {
    FAKE_BUNDLE_WITH_POSITIVE_TOP_PADDING
}

const FAKE_BUNDLE_WITH_POSITIVE_TOP_PADDING: &str = r#"
function Graph() {}
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "101px");
  svg.setAttribute("height", "300px");
  svg.setAttribute("viewBox", "0 0 101 300");
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "shape");
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("x", "0");
  rect.setAttribute("y", "10");
  rect.setAttribute("width", "100");
  rect.setAttribute("height", "100");
  group.appendChild(rect);
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

fn fake_bundle_with_rendered_overflow() -> &'static str {
    FAKE_BUNDLE_WITH_RENDERED_OVERFLOW
}

const FAKE_BUNDLE_WITH_RENDERED_OVERFLOW: &str = r#"
function Graph() {}
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "2560px");
  svg.setAttribute("height", "780px");
  svg.setAttribute("viewBox", "-560 -490 2560 780");
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "phone");
  const phone = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  phone.setAttribute("x", "-560");
  phone.setAttribute("y", "-490");
  phone.setAttribute("width", "390");
  phone.setAttribute("height", "780");
  group.appendChild(phone);
  const overflow = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  overflow.setAttribute("x", "1000");
  overflow.setAttribute("y", "-490");
  overflow.setAttribute("width", "1000");
  overflow.setAttribute("height", "10");
  svg.appendChild(group);
  svg.appendChild(overflow);
  callback({
    graph: {
      getSvg() {
        return svg;
      },
    },
  });
};
"#;
