use super::DrawioJsRuntimeOps;
use super::test_support::{assert_render_contains, fake_html_label_bundle, temp_runtime_path};
use crate::markdown::color_preset::DiagramColorPreset;

#[test]
fn fake_bundle_preserves_nested_gray_html_text_official_dark_color() {
    let path = temp_runtime_path("kdr-drawio-explicit-html-color-unit");
    assert!(std::fs::write(&path, fake_bundle()).is_ok());

    let source = r##"<mxGraphModel><root><mxCell id="cell" value="&lt;font color=&quot;#999999&quot;&gt;Tips&lt;/font&gt;" style="whiteSpace=wrap;html=1;fontColor=#2F5B7C;" vertex="1"><mxGeometry x="0" y="0" width="160" height="60" as="geometry"/></mxCell></root></mxGraphModel>"##;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert_render_contains(&rendered, "color: #6a6a6a");
    assert!(
        !rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("color: #ffffff")),
        "{rendered:?}"
    );
}

fn fake_bundle() -> String {
    fake_html_label_bundle("160", "60", FAKE_BUNDLE_BODY, "")
}

const FAKE_BUNDLE_BODY: &str = r#"  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "cell");
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("x", "0");
  rect.setAttribute("y", "0");
  rect.setAttribute("width", "160");
  rect.setAttribute("height", "60");
  group.appendChild(rect);
  const text = document.createElementNS("http://www.w3.org/2000/svg", "text");
  text.textContent = "Tips";
  group.appendChild(text);
  svg.appendChild(group);
"#;
