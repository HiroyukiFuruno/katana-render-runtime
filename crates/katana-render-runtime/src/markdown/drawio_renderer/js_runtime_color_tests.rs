use super::DrawioJsRuntimeOps;
use crate::markdown::color_preset::DiagramColorPreset;

#[test]
fn fake_bundle_maps_explicit_white_text_to_official_dark_text() {
    let path = temp_runtime_path("kdr-drawio-color-unit");
    assert!(std::fs::write(&path, fake_bundle()).is_ok());

    let source = r#"<mxGraphModel><root><mxCell id="cell" value="Label" style="text;html=1;fontColor=#FFFFFF;" vertex="1"><mxGeometry x="0" y="0" width="80" height="30" as="geometry"/></mxCell></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("color: #121212"))
    );
}

#[test]
fn fake_bundle_maps_dark_gray_text_to_official_dark_theme_text() {
    let path = temp_runtime_path("kdr-drawio-gray-text-unit");
    assert!(std::fs::write(&path, fake_bundle()).is_ok());

    let source = r#"<mxGraphModel><root><mxCell id="cell" value="Label" style="text;html=1;fontColor=#333333;" vertex="1"><mxGeometry x="0" y="0" width="80" height="30" as="geometry"/></mxCell></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("color: #c1c1c1"))
    );
}

#[test]
fn fake_bundle_uses_drawio_adaptive_dark_color_for_source_text() {
    let path = temp_runtime_path("kdr-drawio-adaptive-source-text-unit");
    assert!(std::fs::write(&path, fake_source_text_bundle()).is_ok());

    let source = r##"<mxGraphModel><root><mxCell id="cell" value="Label" style="rounded=1;html=1;fontColor=#7BAC36;" vertex="1"><mxGeometry x="0" y="0" width="80" height="30" as="geometry"/></mxCell></root></mxGraphModel>"##;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("color: #507a14")),
        "{rendered:?}"
    );
    assert!(
        !rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("color: #ffffff"))
    );
}

#[test]
fn fake_bundle_uses_drawio_adaptive_dark_color_for_html_font_color() {
    let path = temp_runtime_path("kdr-drawio-adaptive-html-font-unit");
    assert!(std::fs::write(&path, fake_source_text_bundle()).is_ok());

    let source = r##"<mxGraphModel><root><mxCell id="cell" value="&lt;font color=&quot;#ffffff&quot;&gt;Label&lt;/font&gt;" style="rounded=1;html=1;fontColor=#7BAC36;" vertex="1"><mxGeometry x="0" y="0" width="80" height="30" as="geometry"/></mxCell></root></mxGraphModel>"##;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r##"font color="#121212" style="color: #ffffff""##)),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_keeps_mapped_html_font_color_after_normalization() {
    let path = temp_runtime_path("kdr-drawio-html-font-map-unit");
    assert!(std::fs::write(&path, fake_source_text_bundle()).is_ok());

    let source = r##"<mxGraphModel><root><mxCell id="cell" value="&lt;font color=&quot;#78A65F&quot;&gt;Label&lt;/font&gt;" style="text;html=1;" vertex="1"><mxGeometry x="0" y="0" width="80" height="30" as="geometry"/></mxCell></root></mxGraphModel>"##;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r##"font color="#78A65F" style="color: #50783b""##)),
        "{rendered:?}"
    );
    assert!(
        !rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r##"font color="#78A65F" style="color: #ffffff""##)),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_preserves_default_black_html_text_as_white() {
    let path = temp_runtime_path("kdr-drawio-default-html-font-unit");
    assert!(std::fs::write(&path, fake_source_text_bundle()).is_ok());

    let source = r##"<mxGraphModel><root><mxCell id="cell" value="Label" style="rounded=1;html=1;" vertex="1"><mxGeometry x="0" y="0" width="80" height="30" as="geometry"/></mxCell></root></mxGraphModel>"##;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("color: #ffffff")),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_preserves_default_font_color_html_text_as_white() {
    let path = temp_runtime_path("kdr-drawio-default-font-color-unit");
    assert!(std::fs::write(&path, fake_source_text_bundle()).is_ok());

    let source = r##"<mxGraphModel><root><mxCell id="cell" value="Label" style="rounded=1;html=1;fontColor=default;" vertex="1"><mxGeometry x="0" y="0" width="80" height="30" as="geometry"/></mxCell></root></mxGraphModel>"##;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("color: #ffffff")),
        "{rendered:?}"
    );
}

fn temp_runtime_path(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}.js", std::process::id()))
}

fn fake_bundle() -> &'static str {
    FAKE_BUNDLE
}

const FAKE_BUNDLE: &str = r#"
function Graph() {}
var mxUtils = {
  lightDarkColorSupported: false,
  preferDarkColor: true,
};
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "80");
  svg.setAttribute("height", "30");
  svg.setAttribute("viewBox", "0 0 80 30");
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "cell");
  const shape = document.createElementNS("http://www.w3.org/2000/svg", "g");
  shape.setAttribute("transform", "translate(0.5,0.5)");
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("x", "0");
  rect.setAttribute("y", "0");
  rect.setAttribute("width", "80");
  rect.setAttribute("height", "30");
  shape.appendChild(rect);
  const textGroup = document.createElementNS("http://www.w3.org/2000/svg", "g");
  const text = document.createElementNS("http://www.w3.org/2000/svg", "text");
  text.textContent = "Label";
  textGroup.appendChild(text);
  group.appendChild(shape);
  group.appendChild(textGroup);
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

fn fake_source_text_bundle() -> &'static str {
    FAKE_SOURCE_TEXT_BUNDLE
}

const FAKE_SOURCE_TEXT_BUNDLE: &str = r##"
function Graph() {}
var mxUtils = {
  getLightDarkColor(value) {
    const key = String(value).toLowerCase();
    const dark = ["#000000", "#ffffff"].includes(key) ? "#121212" : "#507a14";
    return {
      light: value,
      dark,
      cssText: `light-dark(${value}, ${dark})`,
    };
  },
};
const Editor = {
  convertHtmlToText(value) {
    return String(value);
  },
};
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "80");
  svg.setAttribute("height", "30");
  svg.setAttribute("viewBox", "0 0 80 30");
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "cell");
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("x", "0");
  rect.setAttribute("y", "0");
  rect.setAttribute("width", "80");
  rect.setAttribute("height", "30");
  const text = document.createElementNS("http://www.w3.org/2000/svg", "text");
  text.setAttribute("x", "40");
  text.setAttribute("y", "18");
  text.textContent = "Label";
  group.appendChild(rect);
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
"##;
