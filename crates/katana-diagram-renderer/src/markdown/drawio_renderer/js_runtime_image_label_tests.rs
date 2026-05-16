use super::DrawioJsRuntimeOps;
use crate::markdown::color_preset::DiagramColorPreset;

#[test]
fn fake_bundle_positions_default_image_html_label_below_image() {
    let path = temp_runtime_path("kdr-drawio-default-image-label-unit");
    assert!(std::fs::write(&path, fake_bundle()).is_ok());

    let source = r#"<mxGraphModel><root><mxCell id="image" value="Network Switch&lt;br&gt;" style="image;html=1;image=img/lib/clip_art/networking/Switch_128x128.png;fontSize=12;fontColor=#0A3DA3;" vertex="1" /></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert_rendered_contains(&rendered, "width: 1px");
    assert_rendered_contains(&rendered, "align-items: unsafe flex-start");
    assert_rendered_contains(&rendered, "padding-top: 89px");
    assert_rendered_contains(&rendered, "margin-left: 140px");
    assert_rendered_contains(&rendered, "white-space: nowrap");
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
  svg.setAttribute("width", "400");
  svg.setAttribute("height", "220");
  svg.setAttribute("viewBox", "0 0 400 220");
  svg.appendChild(createImageGroup("image", 100, 40, 80, 42));
  callback({
    graph: {
      getSvg() {
        return svg;
      },
    },
  });
};
function createImageGroup(id, x, y, width, height) {
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", id);
  const image = document.createElementNS("http://www.w3.org/2000/svg", "image");
  image.setAttribute("x", String(x));
  image.setAttribute("y", String(y));
  image.setAttribute("width", String(width));
  image.setAttribute("height", String(height));
  group.appendChild(image);
  const text = document.createElementNS("http://www.w3.org/2000/svg", "text");
  text.textContent = id;
  group.appendChild(text);
  return group;
}
"#;
