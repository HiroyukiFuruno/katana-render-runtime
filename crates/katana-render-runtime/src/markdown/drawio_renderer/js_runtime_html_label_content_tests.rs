use super::DrawioJsRuntimeOps;
use super::test_support::{assert_render_contains, assert_render_not_contains, temp_runtime_path};
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

    assert_render_contains(&rendered, "First line<br/>Second line");
    assert_render_not_contains(&rendered, "<br/>First line");
    assert_render_not_contains(&rendered, "Second line<br/>");
}

#[test]
fn fake_bundle_trims_styled_edge_breaks_before_markup_html_label() {
    let path = temp_runtime_path("krr-drawio-html-styled-edge-breaks-unit");
    assert!(std::fs::write(&path, fake_bundle()).is_ok());

    let source = r#"<mxfile><diagram><mxGraphModel><root><mxCell id="card" value="&lt;br style=&quot;font-size: 12px;&quot;&gt;First line&lt;br style=&quot;font-size: 12px;&quot;&gt;Second line&lt;br style=&quot;font-size: 12px;&quot;&gt;" style="text;html=1;whiteSpace=wrap;overflow=hidden;fontSize=12;" vertex="1" /></root></mxGraphModel></diagram></mxfile>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert_render_contains(
        &rendered,
        r#"First line<br style="font-size: 12px;"/>Second line"#,
    );
    assert_render_not_contains(&rendered, r#"<br style="font-size: 12px;"/>First line"#);
    assert_render_not_contains(&rendered, r#"Second line<br style="font-size: 12px;"/>"#);
}

#[test]
fn fake_bundle_repositions_html_label_fallback_for_all_vertical_alignments() {
    let path = temp_runtime_path("krr-drawio-html-fallback-vertical-align-unit");
    assert!(std::fs::write(&path, fake_bundle()).is_ok());

    let source = r#"<mxfile><diagram><mxGraphModel><root><mxCell id="topcard" value="First line&#xa;Second line" style="text;html=1;whiteSpace=wrap;overflow=hidden;fontSize=12;verticalAlign=top;" vertex="1" /><mxCell id="middlecard" value="Line 1&#xa;Line 2&#xa;Line 3&#xa;Line 4" style="text;html=1;whiteSpace=wrap;overflow=hidden;fontSize=12;verticalAlign=middle;" vertex="1" /><mxCell id="bottomcard" value="Line 1&#xa;Line 2&#xa;Line 3" style="text;html=1;whiteSpace=wrap;overflow=hidden;fontSize=12;verticalAlign=bottom;" vertex="1" /><mxCell id="m11_note" value="⑪ LibreChat Backend (openidStrategy.js) が id_token 検証&#xa;・iss / aud / exp / sig (JWKS) / nonce&#xa;・roles claim を抽出 → OPENID_REQUIRED_ROLE と照合&#xa;・未割当者は 403 で拒否 (OPENID_REQUIRED_ROLE_TOKEN_KIND=id 前提)" style="text;html=1;fontSize=10;align=left;fontColor=#0d47a1;fillColor=#e3f2fd;strokeColor=#1976d2;verticalAlign=middle;" vertex="1" /></root></mxGraphModel></diagram></mxfile>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert_render_contains(&rendered, r#"<text x="170" y="120""#);
    assert_render_contains(&rendered, r#"<text x="370" y="124""#);
    assert_render_contains(&rendered, r#"<text x="570" y="148""#);
    assert_render_contains(&rendered, r#"<text x="730" y="631" font-size="10px""#);
    assert_render_not_contains(&rendered, r#"<text x="370" y="180""#);
    assert_render_not_contains(&rendered, r#"<text x="570" y="180""#);
    assert_render_not_contains(&rendered, r#"<text x="730" y="638" font-size="10px""#);
}

#[test]
fn fake_bundle_restores_overflow_fill_table_alignment() {
    let path = temp_runtime_path("krr-drawio-html-overflow-fill-unit");
    assert!(std::fs::write(&path, fake_bundle()).is_ok());

    let source = r#"<mxGraphModel><root><mxCell id="fillcard" value="Table" style="html=1;overflow=fill;whiteSpace=wrap;" vertex="1" /></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert_render_contains(&rendered, "align-items: unsafe center");
    assert_render_contains(&rendered, "padding-top: 11px");
    assert_render_contains(&rendered, "white-space: normal");
    assert_render_contains(&rendered, "word-wrap: normal");
    assert_render_not_contains(&rendered, "align-items: unsafe flex-start");
    assert_render_not_contains(&rendered, "white-space: nowrap");
}

#[test]
fn fake_bundle_keeps_complete_native_multiline_html_label() {
    let path = temp_runtime_path("krr-drawio-html-complete-native-unit");
    assert!(std::fs::write(&path, fake_bundle()).is_ok());

    let source = r#"<mxGraphModel><root><mxCell id="complete" value="First line&#xa;Second line" style="html=1;whiteSpace=wrap;" vertex="1" /></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert_render_contains(&rendered, r#"data-native-complete="1""#);
    assert_render_contains(&rendered, "First line<br/>Second line");
}

fn fake_bundle() -> &'static str {
    static BUNDLE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    BUNDLE
        .get_or_init(|| {
            let marker = "  group.appendChild(shape);\n";
            let group_tail = super::html_label_tests::FAKE_BUNDLE_TAIL;
            assert_eq!(group_tail.matches(marker).count(), 1);
            let group_tail =
                group_tail.replacen(marker, &format!("{FAKE_GROUP_TEXT_STYLE}{marker}"), 1);
            let body = [super::html_label_tests::FAKE_BUNDLE_BODY, FAKE_BUNDLE_BODY].concat();
            let tail = [FAKE_BUNDLE_AFTER_CALLBACK, &group_tail].concat();
            super::test_support::fake_html_label_bundle("960", "720", &body, &tail)
        })
        .as_str()
}

const FAKE_BUNDLE_BODY: &str = r#"  svg.appendChild(createGroup("topcard", 100, 100, 140, 80));
  svg.appendChild(createGroup("middlecard", 300, 100, 140, 80));
  svg.appendChild(createGroup("bottomcard", 500, 100, 140, 80));
  svg.appendChild(createGroup("m11_note", 540, 600, 380, 90));
  svg.appendChild(createOverflowFillGroup());
  svg.appendChild(createCompleteNativeLabelGroup());
"#;

const FAKE_BUNDLE_AFTER_CALLBACK: &str = r#"function createCompleteNativeLabelGroup() {
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "complete");
  const shape = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  shape.setAttribute("x", "10");
  shape.setAttribute("y", "10");
  shape.setAttribute("width", "120");
  shape.setAttribute("height", "60");
  group.appendChild(shape);
  const foreignObject = document.createElementNS("http://www.w3.org/2000/svg", "foreignObject");
  const div = document.createElement("div");
  div.setAttribute("data-native-complete", "1");
  div.innerHTML = "First line<br>Second line";
  foreignObject.appendChild(div);
  group.appendChild(foreignObject);
  const text = document.createElementNS("http://www.w3.org/2000/svg", "text");
  text.textContent = "First line\nSecond line";
  group.appendChild(text);
  return group;
}
function createOverflowFillGroup() {
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "fillcard");
  const foreignObject = document.createElementNS("http://www.w3.org/2000/svg", "foreignObject");
  const outer = document.createElement("div");
  outer.setAttribute("style", "display: flex; align-items: unsafe flex-start; padding-top: 10px");
  const box = document.createElement("div");
  const content = document.createElement("div");
  content.setAttribute("style", "white-space: nowrap");
  content.textContent = "Table";
  box.appendChild(content);
  outer.appendChild(box);
  foreignObject.appendChild(outer);
  group.appendChild(foreignObject);
  return group;
}
"#;

const FAKE_GROUP_TEXT_STYLE: &str = r#"  if (["topcard", "middlecard", "bottomcard", "m11_note"].includes(id)) {
    text.setAttribute("x", String(x + width / 2));
    text.setAttribute("y", "180");
    text.setAttribute("font-size", id === "m11_note" ? "10px" : "12px");
    text.setAttribute("text-anchor", "middle");
  }
"#;
