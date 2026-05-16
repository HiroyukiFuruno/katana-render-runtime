use super::DrawioJsRuntimeOps;
use crate::markdown::color_preset::DiagramColorPreset;

const FAKE_BUNDLE_WITH_DEVICE_PAGE_CONTENT: &str = r#"
function Graph() {}
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "300px");
  svg.setAttribute("height", "200px");
  svg.setAttribute("viewBox", "0 0 300 200");
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "shape");
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("x", "0");
  rect.setAttribute("y", "0");
  rect.setAttribute("width", "100");
  rect.setAttribute("height", "60");
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

#[test]
fn fake_bundle_crops_device_page_from_rendered_content_bounds() {
    let path = temp_runtime_path("kdr-drawio-device-page-crop-unit");
    assert!(std::fs::write(&path, fake_bundle_with_device_page_content()).is_ok());

    let rendered =
        DrawioJsRuntimeOps::render(device_page_source(), &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"width="101px""#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| !svg.contains(r#"transform="translate(-10,0)""#)),
        "{rendered:?}"
    );
}

fn temp_runtime_path(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}.js", std::process::id()))
}

fn device_page_source() -> &'static str {
    r#"<mxfile type="device"><diagram><mxGraphModel page="1" background="none"><root>
<mxCell id="1" parent="0"/>
<mxCell id="shape" style="shape=rect;" vertex="1" parent="1">
  <mxGeometry x="10" y="0" width="100" height="60" as="geometry"/>
</mxCell>
</root></mxGraphModel></diagram></mxfile>"#
}

fn fake_bundle_with_device_page_content() -> &'static str {
    FAKE_BUNDLE_WITH_DEVICE_PAGE_CONTENT
}
