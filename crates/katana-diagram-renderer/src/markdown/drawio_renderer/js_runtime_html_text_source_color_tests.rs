use super::DrawioJsRuntimeOps;
use crate::markdown::color_preset::DiagramColorPreset;

const FAKE_BUNDLE_TEMPLATE: &str = r##"
function Graph() {}
var mxUtils = {
  getLightDarkColor(value) {
    const key = String(value).toLowerCase();
    const dark = key === "#7bac36" ? "#507a14" : "#121212";
    return {
      light: value,
      dark,
      cssText: `light-dark(${{value}}, ${{dark}})`,
    };
  },
};
var Editor = {
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
  const foreignObject = document.createElementNS("http://www.w3.org/2000/svg", "foreignObject");
  const div = document.createElement("div");
  div.setAttribute("data-katana-drawio-html-text", "content");
  div.setAttribute("style", "font-size: 12px; color: {wrong_color}");
  div.textContent = "Label";
  foreignObject.appendChild(div);
  group.appendChild(foreignObject);
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

const FAKE_NESTED_BUNDLE: &str = r##"
function Graph() {}
var mxUtils = {
  getLightDarkColor(value) {
    return {
      light: value,
      dark: "#121212",
      cssText: `light-dark(${value}, #121212)`,
    };
  },
};
var Editor = {
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
  const foreignObject = document.createElementNS("http://www.w3.org/2000/svg", "foreignObject");
  const root = document.createElement("div");
  root.setAttribute("data-katana-drawio-html-text", "content");
  root.setAttribute("style", "font-size: 12px; color: #121212");
  const nested = document.createElement("div");
  nested.textContent = "Label";
  root.appendChild(nested);
  foreignObject.appendChild(root);
  group.appendChild(foreignObject);
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
fn fake_bundle_restores_explicit_black_html_text_source_color() {
    assert_restored_html_text_source_color("#000000", "#202020", "#ffffff");
}

#[test]
fn fake_bundle_restores_explicit_white_html_text_source_color() {
    assert_restored_html_text_source_color("#FFFFFF", "#ffffff", "#121212");
}

#[test]
fn fake_bundle_restores_adaptive_html_text_source_color() {
    assert_restored_html_text_source_color("#7BAC36", "#ffffff", "#507a14");
}

#[test]
fn fake_bundle_keeps_explicit_html_text_source_color_in_light_mode() {
    assert_restored_html_text_source_color_with_preset(
        "#7BAC36",
        "#ffffff",
        "#7bac36",
        DiagramColorPreset::light(),
    );
}

#[test]
fn fake_bundle_restores_nested_default_html_text_source_color() {
    assert_restored_nested_html_text_source_color("whiteSpace=wrap;html=1;");
}

#[test]
fn fake_bundle_restores_nested_fill_html_text_source_color() {
    assert_restored_nested_html_text_source_color("whiteSpace=wrap;html=1;fillColor=#f8cecc;");
}

fn assert_restored_nested_html_text_source_color(source_style: &str) {
    let path = temp_runtime_path("kdr-drawio-nested-html-text-source-color-unit");
    assert!(std::fs::write(&path, fake_nested_bundle()).is_ok());

    let source = format!(
        r#"<mxGraphModel><root><mxCell id="cell" value="&lt;div&gt;Label&lt;/div&gt;" style="{source_style}" vertex="1"><mxGeometry x="0" y="0" width="80" height="30" as="geometry"/></mxCell></root></mxGraphModel>"#,
    );
    let rendered = DrawioJsRuntimeOps::render(&source, &path, DiagramColorPreset::dark());

    assert_render_contains(&rendered, "color: #ffffff");
    assert_render_not_contains(&rendered, "color: #121212");
}

fn assert_restored_html_text_source_color(source_color: &str, wrong_color: &str, expected: &str) {
    assert_restored_html_text_source_color_with_preset(
        source_color,
        wrong_color,
        expected,
        DiagramColorPreset::dark(),
    );
}

fn assert_restored_html_text_source_color_with_preset(
    source_color: &str,
    wrong_color: &str,
    expected: &str,
    preset: &DiagramColorPreset,
) {
    let path = temp_runtime_path("kdr-drawio-html-text-source-color-unit");
    assert!(std::fs::write(&path, fake_bundle(wrong_color)).is_ok());

    let source = format!(
        r##"<mxGraphModel><root><mxCell id="cell" value="Label" style="text;html=1;fontColor={source_color};" vertex="1"><mxGeometry x="0" y="0" width="80" height="30" as="geometry"/></mxCell></root></mxGraphModel>"##
    );
    let rendered = DrawioJsRuntimeOps::render(&source, &path, preset);

    let expected_token = format!("color: {expected}");
    let wrong_token = format!("color: {wrong_color}");
    assert_render_contains(&rendered, expected_token.as_str());
    assert_render_not_contains(&rendered, wrong_token.as_str());
}

fn temp_runtime_path(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}.js", std::process::id()))
}

fn fake_bundle(wrong_color: &str) -> String {
    FAKE_BUNDLE_TEMPLATE.replace("{wrong_color}", wrong_color)
}

fn fake_nested_bundle() -> &'static str {
    FAKE_NESTED_BUNDLE
}

fn assert_render_contains(rendered: &Result<String, String>, marker: &str) {
    assert!(
        rendered.as_ref().is_ok_and(|svg| svg.contains(marker)),
        "{rendered:?}"
    );
}

fn assert_render_not_contains(rendered: &Result<String, String>, marker: &str) {
    assert!(
        rendered.as_ref().is_ok_and(|svg| !svg.contains(marker)),
        "{rendered:?}"
    );
}
