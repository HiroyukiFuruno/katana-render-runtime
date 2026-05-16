use super::DrawioJsRuntimeOps;
use crate::markdown::color_preset::DiagramColorPreset;

const PLAIN_TEXT_LABEL_SOURCE: &str = r#"<mxGraphModel><root>
<mxCell id="1" parent="0"/>
<mxCell id="text" value="First line&#10;Second line" style="text;spacingTop=-5;whiteSpace=wrap" parent="1" vertex="1">
  <mxGeometry x="20" y="30" width="120" height="60" as="geometry"/>
</mxCell>
</root></mxGraphModel>"#;

#[test]
fn fake_bundle_replaces_multiline_plain_text_foreign_object_with_full_label() {
    let path = temp_runtime_path("kdr-drawio-plain-text-label-unit");
    assert!(std::fs::write(&path, fake_bundle_with_truncated_plain_text()).is_ok());

    let rendered =
        DrawioJsRuntimeOps::render(PLAIN_TEXT_LABEL_SOURCE, &path, DiagramColorPreset::dark());
    assert_plain_text_svg_contains(&rendered);
}

fn temp_runtime_path(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}.js", std::process::id()))
}

fn fake_bundle_with_truncated_plain_text() -> &'static str {
    FAKE_BUNDLE_WITH_TRUNCATED_PLAIN_TEXT
}

fn assert_plain_text_svg_contains(rendered: &Result<String, String>) {
    assert_render_contains(rendered, "First line<br></br>Second line");
    assert_render_contains(rendered, "width: 118px");
    assert_render_contains(rendered, "padding-top: 32px");
    assert_render_contains(rendered, "margin-left: 22px");
}

fn assert_render_contains(rendered: &Result<String, String>, marker: &str) {
    assert!(
        rendered.as_ref().is_ok_and(|svg| svg.contains(marker)),
        "{rendered:?}"
    );
}

const FAKE_BUNDLE_WITH_TRUNCATED_PLAIN_TEXT: &str = r#"
function Graph() {}
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "160px");
  svg.setAttribute("height", "120px");
  svg.setAttribute("viewBox", "0 0 160 120");
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "text");
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("x", "20");
  rect.setAttribute("y", "30");
  rect.setAttribute("width", "120");
  rect.setAttribute("height", "60");
  const foreignObject = document.createElementNS("http://www.w3.org/2000/svg", "foreignObject");
  const div = document.createElement("div");
  div.textContent = "First line";
  foreignObject.appendChild(div);
  const text = document.createElementNS("http://www.w3.org/2000/svg", "text");
  text.textContent = "First line...";
  group.appendChild(rect);
  group.appendChild(foreignObject);
  group.appendChild(text);
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
