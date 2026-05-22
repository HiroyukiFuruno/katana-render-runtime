use super::DrawioJsRuntimeOps;
use crate::markdown::color_preset::DiagramColorPreset;

const FAKE_BUNDLE_WITH_EDGE_LABELS: &str = r#"
function Graph() {}
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "240px");
  svg.setAttribute("height", "120px");
  svg.setAttribute("viewBox", "0 0 240 120");
  svg.appendChild(createEdgeGroup("negative", 80));
  svg.appendChild(createEdgeGroup("positive", 180));
  callback({
    graph: {
      getSvg() {
        return svg;
      },
    },
  });
};
function createEdgeGroup(id, x) {
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", id);
  const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
  path.setAttribute("d", "M 10 10 L 20 20");
  const text = document.createElementNS("http://www.w3.org/2000/svg", "text");
  text.setAttribute("x", String(x));
  text.setAttribute("y", "60");
  text.textContent = "Label";
  group.appendChild(path);
  group.appendChild(text);
  return group;
}
"#;

#[test]
fn fake_bundle_shifts_negative_edge_markup_text_label() {
    let path = temp_runtime_path("kdr-drawio-negative-edge-label-unit");
    assert!(std::fs::write(&path, fake_bundle_with_edge_labels()).is_ok());

    let rendered =
        DrawioJsRuntimeOps::render(edge_label_source(), &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"transform="translate(-40,0)""#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"<text x="180""#)),
        "{rendered:?}"
    );
}

fn temp_runtime_path(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}.js", std::process::id()))
}

fn edge_label_source() -> &'static str {
    r#"<mxGraphModel page="0"><root>
<mxCell id="1" parent="0"/>
<mxCell id="negative" value="Left&lt;div&gt;Label&lt;/div&gt;" style="shape=flexArrow;html=1;fillColor=#10739E;fontColor=#10739E;" edge="1" parent="1">
  <mxGeometry x="-0.2" width="50" height="50" relative="1" as="geometry"/>
</mxCell>
<mxCell id="positive" value="Right&lt;div&gt;Label&lt;/div&gt;" style="shape=flexArrow;html=1;fillColor=#AE4132;fontColor=#AE4132;" edge="1" parent="1">
  <mxGeometry x="0.2" width="50" height="50" relative="1" as="geometry"/>
</mxCell>
</root></mxGraphModel>"#
}

fn fake_bundle_with_edge_labels() -> &'static str {
    FAKE_BUNDLE_WITH_EDGE_LABELS
}
