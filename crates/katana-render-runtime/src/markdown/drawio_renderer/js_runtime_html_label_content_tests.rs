use super::DrawioJsRuntimeOps;
use crate::markdown::color_preset::DiagramColorPreset;

#[test]
fn fake_bundle_restores_object_label_with_parent_placeholder() {
    let path = temp_runtime_path("kdr-drawio-html-object-label-unit");
    assert!(std::fs::write(&path, fake_bundle()).is_ok());

    let source = r#"<mxGraphModel><root><object label="TO DO" status="New" id="column"><mxCell parent="1" vertex="1" /></object><object label="Task 3&lt;br&gt;&lt;i&gt;%status%&lt;/i&gt;" placeholders="1" id="card"><mxCell style="whiteSpace=wrap;html=1;fontSize=16;fontColor=#FFFFFF;" parent="column" vertex="1" /></object></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered.as_ref().is_ok_and(|svg| svg.contains("Task 3")),
        "{rendered:?}"
    );
    assert!(
        rendered.as_ref().is_ok_and(|svg| svg.contains("New")),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("color: #121212")),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_preserves_plain_multiline_html_label() {
    let path = temp_runtime_path("kdr-drawio-html-plain-multiline-unit");
    assert!(std::fs::write(&path, fake_bundle()).is_ok());

    let source = r#"<mxfile><diagram><mxGraphModel><root><mxCell id="math" value="$$A_{m,n} =&#xa; \begin{pmatrix}&#xa;  a_{1,1} &amp; a_{1,2} &amp; \cdots &amp; a_{1,n} \\&#xa;  a_{2,1} &amp; a_{2,2} &amp; \cdots &amp; a_{2,n} \\&#xa; \end{pmatrix}$$" style="text;html=1;whiteSpace=wrap;overflow=hidden;fontSize=12;" vertex="1" /></root></mxGraphModel></diagram></mxfile>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"a_{1,1} &amp; a_{1,2}"#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"\begin{pmatrix}"#)),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"\end{pmatrix}$$"#)),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_normalizes_html_label_block_boundaries_and_nbsp() {
    let path = temp_runtime_path("kdr-drawio-html-block-nbsp-unit");
    assert!(std::fs::write(&path, fake_bundle()).is_ok());

    let source = r#"<mxfile><diagram><mxGraphModel><root><mxCell id="card" value="&lt;div&gt;Europe&nbsp;Oil Inc.&lt;/div&gt;&lt;div&gt;Auxiliary&nbsp;Oil Refining Plant&lt;/div&gt;" style="text;html=1;whiteSpace=wrap;overflow=hidden;fontSize=12;" vertex="1" /></root></mxGraphModel></diagram></mxfile>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("<tspan x=\"170\" dy=\"0\">Europe Oil Inc.</tspan>")),
        "{rendered:?}"
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg
                .contains("<tspan x=\"170\" dy=\"15\">Auxiliary Oil Refining Plant</tspan>")),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_trims_edge_breaks_before_markup_html_label() {
    let path = temp_runtime_path("krr-drawio-html-edge-breaks-unit");
    assert!(std::fs::write(&path, fake_bundle()).is_ok());

    let source = r#"<mxfile><diagram><mxGraphModel><root><mxCell id="card" value="&lt;br&gt;First line&lt;br&gt;Second line&lt;br&gt;" style="text;html=1;whiteSpace=wrap;overflow=hidden;fontSize=12;" vertex="1" /></root></mxGraphModel></diagram></mxfile>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert_render_contains(&rendered, "First line<br></br>Second line");
    assert_render_not_contains(&rendered, "<br></br>First line");
    assert_render_not_contains(&rendered, "Second line<br></br>");
}

#[test]
fn fake_bundle_trims_styled_edge_breaks_before_markup_html_label() {
    let path = temp_runtime_path("krr-drawio-html-styled-edge-breaks-unit");
    assert!(std::fs::write(&path, fake_bundle()).is_ok());

    let source = r#"<mxfile><diagram><mxGraphModel><root><mxCell id="card" value="&lt;br style=&quot;font-size: 12px;&quot;&gt;First line&lt;br style=&quot;font-size: 12px;&quot;&gt;Second line&lt;br style=&quot;font-size: 12px;&quot;&gt;" style="text;html=1;whiteSpace=wrap;overflow=hidden;fontSize=12;" vertex="1" /></root></mxGraphModel></diagram></mxfile>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert_render_contains(
        &rendered,
        r#"First line<br style="font-size: 12px;"></br>Second line"#,
    );
    assert_render_not_contains(&rendered, r#"<br style="font-size: 12px;"></br>First line"#);
    assert_render_not_contains(
        &rendered,
        r#"Second line<br style="font-size: 12px;"></br>"#,
    );
}

#[test]
fn fake_bundle_repositions_top_html_label_fallback_to_html_label_top() {
    let path = temp_runtime_path("krr-drawio-html-top-fallback-unit");
    assert!(std::fs::write(&path, fake_bundle()).is_ok());

    let source = r#"<mxfile><diagram><mxGraphModel><root><mxCell id="topcard" value="First line&#xa;Second line" style="text;html=1;whiteSpace=wrap;overflow=hidden;fontSize=12;verticalAlign=top;" vertex="1" /></root></mxGraphModel></diagram></mxfile>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert_render_contains(&rendered, r#"<text x="170" y="120""#);
    assert_render_not_contains(&rendered, r#"<text x="170" y="180""#);
}

fn temp_runtime_path(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}.js", std::process::id()))
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
  svg.appendChild(createGroup("topcard", 100, 100, 140, 80));
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
  if (id === "topcard") {
    text.setAttribute("x", "170");
    text.setAttribute("y", "180");
    text.setAttribute("font-size", "12px");
    text.setAttribute("text-anchor", "middle");
  }
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
