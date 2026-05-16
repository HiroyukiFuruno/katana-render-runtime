use super::DrawioJsRuntimeOps;
use crate::markdown::color_preset::DiagramColorPreset;

const FAKE_BUNDLE_WITH_LIGHT_CANVAS_BORDER: &str = r##"
function Graph() {}
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "80px");
  svg.setAttribute("height", "40px");
  svg.setAttribute("viewBox", "0 0 80 40");
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "cell");
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("x", "0");
  rect.setAttribute("y", "0");
  rect.setAttribute("width", "80");
  rect.setAttribute("height", "40");
  rect.setAttribute("fill", "#ffffff");
  rect.setAttribute("stroke", "#e8edf0");
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
"##;

#[test]
fn fake_bundle_maps_business_canvas_border_to_official_dark() {
    let path = temp_runtime_path("kdr-drawio-business-canvas-border-unit");
    assert!(std::fs::write(&path, fake_bundle_with_light_canvas_border()).is_ok());

    let rendered =
        DrawioJsRuntimeOps::render(business_canvas_source(), &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r##"stroke="#1e2325""##)),
        "{rendered:?}"
    );
    assert!(
        !rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r##"stroke="#e8edf0""##)),
        "{rendered:?}"
    );
}

fn temp_runtime_path(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}.js", std::process::id()))
}

fn business_canvas_source() -> &'static str {
    r##"<mxGraphModel><root>
<mxCell id="1" parent="0"/>
<mxCell id="cell" style="rounded=0;whiteSpace=wrap;html=1;strokeColor=#e8edf0;strokeWidth=5;fillColor=#ffffff;" vertex="1" parent="1">
  <mxGeometry x="0" y="0" width="80" height="40" as="geometry"/>
</mxCell>
</root></mxGraphModel>"##
}

fn fake_bundle_with_light_canvas_border() -> &'static str {
    FAKE_BUNDLE_WITH_LIGHT_CANVAS_BORDER
}
