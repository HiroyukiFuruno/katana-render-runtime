use super::super::DrawioJsRuntimeOps;
use super::super::test_support::{fake_bundle, temp_runtime_path};
use crate::markdown::color_preset::DiagramColorPreset;

const HIDDEN_TEXT_REPETITIONS: usize = 4;

fn visible_text_fixture(parent: &str, visible: &str, trailing: &str, hidden: &str) -> String {
    format!(
        r#"  const label = document.createElement("div");
  label.textContent = "{visible}";
  label.appendChild(document.createElement("br"));
  const trailing = document.createElement("span");
  trailing.textContent = "{trailing}";
  label.appendChild(trailing);
  const hidden = document.createElement("span");
  hidden.setAttribute("style", "display: none");
  hidden.textContent = "{hidden}";
  label.appendChild(hidden);
  {parent}.appendChild(label);
"#,
    )
}

fn render_visible_text_contract() -> Result<String, String> {
    let path = temp_runtime_path("kdr-drawio-visible-inner-text-box-unit");
    let hidden = "hidden text remains part of textContent".repeat(HIDDEN_TEXT_REPETITIONS);
    let body = format!(
        r#"{}
  const visibleOnly = (() => {{
{}
    return label;
  }})();
  svg.setAttribute(
    "data-text-content-retains-hidden",
    String(label.textContent.includes("hidden text remains part of textContent")),
  );
  svg.setAttribute("data-visible-inner-text", label.innerText.replace("\n", "|"));
  svg.setAttribute("data-empty-inner-text", document.createElement("div").innerText);
  svg.setAttribute(
    "data-hidden-text-does-not-expand-box",
    String(label.getBBox().width === visibleOnly.getBBox().width),
  );
"#,
        visible_text_fixture("svg", "v", "a", &hidden),
        visible_text_fixture("svg", "v", "a", ""),
    );
    let bundle = fake_bundle().replace("  svg.appendChild(text);", &body);
    std::fs::write(&path, bundle).map_err(|error| error.to_string())?;
    DrawioJsRuntimeOps::render("<mxGraphModel />", &path, DiagramColorPreset::light())
}

#[test]
fn fake_bundle_separates_text_content_inner_text_and_hidden_layout_text() {
    let rendered = render_visible_text_contract();

    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            svg.contains(r#"data-text-content-retains-hidden="true""#)
                && svg.contains(r#"data-visible-inner-text="v|a""#)
                && svg.contains(r#"data-empty-inner-text="""#)
                && svg.contains(r#"data-hidden-text-does-not-expand-box="true""#)
        }),
        "actual SVG or render error: {rendered:?}"
    );
}

fn render_visible_text_error() -> Result<String, String> {
    let path = temp_runtime_path("kdr-drawio-visible-inner-text-error-unit");
    let label = visible_text_fixture("_container", "visible", "after", "hidden");
    let bundle = fake_bundle()
        .replace(
            "GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {",
            &format!(
                "GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {{\n{label}"
            ),
        )
        .replace("  callback({\n", "  return;\n  callback({\n");
    std::fs::write(&path, bundle).map_err(|error| error.to_string())?;
    DrawioJsRuntimeOps::render("<mxGraphModel />", &path, DiagramColorPreset::light())
}

#[test]
fn fake_bundle_uses_visible_inner_text_for_drawio_error() {
    let rendered = render_visible_text_error();

    assert!(
        rendered
            .as_ref()
            .is_err_and(|error| error == "Error: Draw.io viewer failed: visible\nafter"),
        "actual SVG or error: {rendered:?}"
    );
}
