use super::DrawioJsRuntimeOps;
use crate::markdown::color_preset::DiagramColorPreset;

#[test]
fn fake_bundle_pads_positive_infographic_export_top() {
    let path = temp_runtime_path("kdr-drawio-infographic-export-top-unit");
    assert!(std::fs::write(&path, fake_bundle_with_top_aligned_content()).is_ok());

    let source = r#"<mxGraphModel page="0"><root><mxCell id="1" parent="0"/><mxCell id="shape" style="shape=mxgraph.infographic.cylinder;" parent="1" vertex="1"><mxGeometry x="0" y="82" width="100" height="100" as="geometry"/></mxCell></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"viewBox="0 -10 100 110""#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"height="110px""#)),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_pads_dark_cube_export_top() {
    let path = temp_runtime_path("kdr-drawio-dark-cube-export-top-unit");
    assert!(std::fs::write(&path, fake_bundle_with_top_aligned_content()).is_ok());

    let source = r##"<mxGraphModel page="1" background="#1A1A1A"><root><mxCell id="1" parent="0"/><mxCell id="shape" style="shape=cube;fillColor=#A680B8;" parent="1" vertex="1"><mxGeometry width="100" height="100" as="geometry"/></mxCell></root></mxGraphModel>"##;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"viewBox="0 -10 100 110""#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"height="110px""#)),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_does_not_pad_regular_page_export_top() {
    let path = temp_runtime_path("kdr-drawio-regular-export-top-unit");
    assert!(std::fs::write(&path, fake_bundle_with_top_aligned_content()).is_ok());

    let source = r#"<mxGraphModel page="1"><root><mxCell id="1" parent="0"/><mxCell id="shape" style="rounded=1;" parent="1" vertex="1"><mxGeometry width="100" height="100" as="geometry"/></mxCell></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"viewBox="0 0 100 100""#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"height="100px""#)),
        "{rendered:?}"
    );
}

fn temp_runtime_path(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}.js", std::process::id()))
}

fn fake_bundle_with_top_aligned_content() -> &'static str {
    FAKE_BUNDLE_WITH_TOP_ALIGNED_CONTENT
}

const FAKE_BUNDLE_WITH_TOP_ALIGNED_CONTENT: &str = r#"
function Graph() {}
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "100px");
  svg.setAttribute("height", "100px");
  svg.setAttribute("viewBox", "0 0 100 100");
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "shape");
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("x", "0");
  rect.setAttribute("y", "0");
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
