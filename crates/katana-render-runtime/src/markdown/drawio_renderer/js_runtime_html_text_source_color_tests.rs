use super::DrawioJsRuntimeOps;
use super::test_support::{
    assert_render_contains, assert_render_not_contains, fake_html_text_color_bundle,
};
use crate::markdown::color_preset::DiagramColorPreset;
use std::sync::atomic::{AtomicUsize, Ordering};

static TEMP_RUNTIME_COUNTER: AtomicUsize = AtomicUsize::new(0);

const COLOR_HELPER_TEMPLATE: &str = "var mxUtils = {\n{color_helper}\n};\n";

const HTML_TEXT_BODY_TEMPLATE: &str = r##"  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "cell");
  const foreignObject = document.createElementNS("http://www.w3.org/2000/svg", "foreignObject");
{html_text_node}
  group.appendChild(foreignObject);
  svg.appendChild(group);
"##;

const DIRECT_COLOR_HELPER: &str = r##"  getLightDarkColor(value) {
    const key = String(value).toLowerCase();
    const dark = key === "#7bac36" ? "#507a14" : "#121212";
    return {
      light: value,
      dark,
      cssText: `light-dark(${{value}}, ${{dark}})`,
    };
  },"##;

const NESTED_COLOR_HELPER: &str = r##"  getLightDarkColor(value) {
    return {
      light: value,
      dark: "#121212",
      cssText: `light-dark(${value}, #121212)`,
    };
  },"##;

const DIRECT_HTML_TEXT_NODE: &str = r##"  const div = document.createElement("div");
  div.setAttribute("data-katana-drawio-html-text", "content");
  div.setAttribute("style", "font-size: 12px; color: {wrong_color}");
  div.textContent = "Label";
  foreignObject.appendChild(div);"##;

const NESTED_HTML_TEXT_NODE: &str = r##"  const root = document.createElement("div");
  root.setAttribute("data-katana-drawio-html-text", "content");
  root.setAttribute("style", "font-size: 12px; color: #121212");
  const nested = document.createElement("div");
  nested.textContent = "Label";
  root.appendChild(nested);
  foreignObject.appendChild(root);"##;

#[test]
fn fake_bundle_restores_explicit_black_html_text_source_color() {
    assert_restored_html_text_source_color("#000000", "#202020", "#ededed");
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
    let counter = TEMP_RUNTIME_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{prefix}-{}-{counter}.js", std::process::id()))
}

fn fake_bundle(wrong_color: &str) -> String {
    fake_bundle_with_content(
        DIRECT_COLOR_HELPER,
        &DIRECT_HTML_TEXT_NODE.replace("{wrong_color}", wrong_color),
    )
}

fn fake_nested_bundle() -> String {
    fake_bundle_with_content(NESTED_COLOR_HELPER, NESTED_HTML_TEXT_NODE)
}

fn fake_bundle_with_content(color_helper: &str, html_text_node: &str) -> String {
    fake_html_text_color_bundle(COLOR_HELPER_TEMPLATE, HTML_TEXT_BODY_TEMPLATE)
        .replace("{color_helper}", color_helper)
        .replace("{html_text_node}", html_text_node)
}
