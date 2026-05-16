use super::DrawioJsRuntimeOps;
use crate::markdown::color_preset::DiagramColorPreset;

#[test]
fn fake_bundle_maps_floorplan_white_paint_to_official_dark() {
    let path = temp_runtime_path("kdr-drawio-floorplan-white-unit");
    assert!(std::fs::write(&path, fake_bundle_with_floorplan_white_paint()).is_ok());

    let rendered =
        DrawioJsRuntimeOps::render(floorplan_source(), &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r##"fill="#ebebeb""##)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r##"stroke="#ebebeb""##)),
        "{rendered:?}"
    );
}

fn temp_runtime_path(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}.js", std::process::id()))
}

fn floorplan_source() -> &'static str {
    r#"<mxGraphModel><root>
<mxCell id="1" parent="0"/>
<mxCell id="cell" style="shape=mxgraph.floorplan.office_chair;fillColor=#000000;strokeColor=#000000;" vertex="1" parent="1">
  <mxGeometry x="0" y="0" width="40" height="40" as="geometry"/>
</mxCell>
</root></mxGraphModel>"#
}

fn fake_bundle_with_floorplan_white_paint() -> &'static str {
    FAKE_BUNDLE_WITH_FLOORPLAN_WHITE_PAINT
}

const FAKE_BUNDLE_WITH_FLOORPLAN_WHITE_PAINT: &str = r##"
function Graph() {}
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "40px");
  svg.setAttribute("height", "40px");
  svg.setAttribute("viewBox", "0 0 40 40");
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "cell");
  const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
  path.setAttribute("d", "M 1 1 L 39 1 L 39 39 L 1 39 Z");
  path.setAttribute("fill", "#000000");
  path.setAttribute("stroke", "#000000");
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
"##;
