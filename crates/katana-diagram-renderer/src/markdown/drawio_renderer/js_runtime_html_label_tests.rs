use super::DrawioJsRuntimeOps;
use crate::markdown::color_preset::DiagramColorPreset;

#[test]
fn fake_bundle_applies_html_label_spacing() {
    let path = temp_runtime_path("kdr-drawio-html-label-spacing-unit");
    assert!(std::fs::write(&path, fake_bundle()).is_ok());

    let source = r#"<mxGraphModel><root><mxCell id="right" value="Right" style="align=right;verticalAlign=top;html=1;fontSize=12;spacing=10;spacingLeft=0;spacingRight=70;" vertex="1" /><mxCell id="left" value="Left" style="align=left;verticalAlign=top;html=1;fontSize=12;spacing=10;spacingLeft=70;" vertex="1" /></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::light());

    assert_rendered_contains(&rendered, "width: 224px");
    assert_rendered_contains(&rendered, "padding-top: 160px");
    assert_rendered_contains(&rendered, "margin-left: 42px");
    assert_rendered_contains(&rendered, "width: 242px");
    assert_rendered_contains(&rendered, "margin-left: 616px");
}

#[test]
fn fake_bundle_defaults_text_shape_html_label_to_top_left() {
    let path = temp_runtime_path("kdr-drawio-html-text-default-unit");
    assert!(std::fs::write(&path, fake_bundle()).is_ok());

    let source = r#"<mxGraphModel><root><mxCell id="text" value="Text" style="text;html=1;fontSize=12;spacing=5;spacingTop=-20;whiteSpace=wrap;overflow=hidden;" vertex="1" /></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::light());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("align-items: unsafe flex-start"))
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("justify-content: unsafe flex-start"))
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("padding-top: 0px"))
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("max-height: 40px")),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_offsets_dark_cube_page_text_shape_like_official() {
    let path = temp_runtime_path("kdr-drawio-dark-cube-html-text-unit");
    assert!(std::fs::write(&path, fake_bundle()).is_ok());

    let source = r##"<mxGraphModel page="1" background="#1A1A1A"><root><mxCell id="cube" style="shape=cube;" vertex="1" /><mxCell id="text" value="Text" style="text;html=1;fontSize=12;spacing=5;spacingTop=-20;whiteSpace=wrap;overflow=hidden;" vertex="1" /></root></mxGraphModel>"##;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("padding-top: 3px")),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_applies_middle_html_label_vertical_spacing() {
    let path = temp_runtime_path("kdr-drawio-html-middle-spacing-unit");
    assert!(std::fs::write(&path, fake_bundle()).is_ok());

    let source = r#"<mxGraphModel><root><mxCell id="text" value="Text" style="text;html=1;fontSize=12;spacing=5;spacingTop=-10;whiteSpace=wrap;overflow=hidden;verticalAlign=middle;" vertex="1" /></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::light());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("padding-top: 25px")),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_positions_top_html_label_like_drawio() {
    let path = temp_runtime_path("kdr-drawio-html-top-label-unit");
    assert!(std::fs::write(&path, fake_bundle()).is_ok());

    let source = r#"<mxGraphModel><root><mxCell id="card" value="Card" style="whiteSpace=wrap;rounded=1;html=1;align=left;verticalAlign=top;fontSize=12;" vertex="1" /></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::light());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("padding-top: 108px")),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_positions_bottom_uml_actor_html_label_like_drawio() {
    let path = temp_runtime_path("kdr-drawio-html-bottom-actor-label-unit");
    assert!(std::fs::write(&path, fake_bundle()).is_ok());

    let source = r#"<mxGraphModel><root><mxCell id="actor" value="Third-party payment &lt;br&gt;verification service" style="shape=umlActor;verticalLabelPosition=bottom;verticalAlign=top;html=1;fontSize=12;align=center;" vertex="1" /></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert_rendered_contains(&rendered, "width: 1px");
    assert_rendered_contains(&rendered, "padding-top: 67px");
    assert_rendered_contains(&rendered, "margin-left: 15px");
    assert_rendered_contains(&rendered, "white-space: nowrap");
}

#[test]
fn fake_bundle_adapts_nested_html_font_color_for_dark_mode() {
    let path = temp_runtime_path("kdr-drawio-html-font-color-unit");
    assert!(std::fs::write(&path, fake_bundle()).is_ok());

    let source = r##"<mxGraphModel><root><mxCell id="card" value="&lt;font color=&quot;#a0522d&quot;&gt;Development&lt;/font&gt;" style="whiteSpace=wrap;rounded=1;html=1;align=left;verticalAlign=top;fontSize=12;" vertex="1" /></root></mxGraphModel>"##;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("color: #d28f70")),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_positions_explicit_left_html_label_like_drawio() {
    let path = temp_runtime_path("kdr-drawio-html-left-label-unit");
    assert!(std::fs::write(&path, fake_bundle()).is_ok());

    let source = r#"<mxGraphModel><root><mxCell id="package" value="«component»&lt;br&gt;&lt;b&gt;:OnlineStore&lt;/b&gt;" style="html=1;whiteSpace=wrap;fontSize=16;labelPosition=left;verticalLabelPosition=top;align=right;verticalAlign=bottom;spacingLeft=0;spacingRight=-120;spacingTop=0;spacingBottom=-50;" vertex="1" /></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("width: 452px")),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("padding-top: 47px")),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("margin-left: -232px")),
        "{rendered:?}"
    );
}

fn temp_runtime_path(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}.js", std::process::id()))
}

fn assert_rendered_contains(rendered: &Result<String, String>, needle: &str) {
    assert!(
        rendered.as_ref().is_ok_and(|svg| svg.contains(needle)),
        "{rendered:?}"
    );
}

fn fake_bundle() -> &'static str {
    FAKE_BUNDLE
}

const FAKE_BUNDLE: &str = r#"
function Graph() {}
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "900");
  svg.setAttribute("height", "400");
  svg.setAttribute("viewBox", "0 0 900 400");
  svg.appendChild(createGroup("right", 34, 145, 312, 95));
  svg.appendChild(createGroup("left", 536, 265, 330, 95));
  svg.appendChild(createGroup("text", 10, 10, 100, 40));
  svg.appendChild(createGroup("card", 100, 100, 140, 80));
  svg.appendChild(createGroup("actor", 0, 0, 30, 60));
  svg.appendChild(createGroup("math", 250, 100, 220, 120));
  svg.appendChild(createGroup("package", 100, 0, 450, 280));
  callback({
    graph: {
      getSvg() {
        return svg;
      },
    },
  });
};
function createGroup(id, x, y, width, height) {
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", id);
  const shape = document.createElementNS("http://www.w3.org/2000/svg", "g");
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("x", String(x));
  rect.setAttribute("y", String(y));
  rect.setAttribute("width", String(width));
  rect.setAttribute("height", String(height));
  shape.appendChild(rect);
  const text = document.createElementNS("http://www.w3.org/2000/svg", "text");
  text.textContent = id;
  group.appendChild(shape);
  if (id === "math") {
    const foreignObject = document.createElementNS("http://www.w3.org/2000/svg", "foreignObject");
    const div = document.createElement("div");
    div.textContent = "$$A_{m,n} =";
    foreignObject.appendChild(div);
    group.appendChild(foreignObject);
  }
  group.appendChild(text);
  return group;
}
"#;
