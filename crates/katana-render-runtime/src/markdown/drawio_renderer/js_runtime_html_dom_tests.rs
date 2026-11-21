use super::DrawioJsRuntimeOps;
use super::test_support::{
    FAKE_BUNDLE_WITH_FOREIGN_OBJECT, HTML_WRAP_BUNDLE_HOOK, fake_bundle,
    fake_bundle_with_foreign_object, fake_bundle_with_html_comment_label,
    fake_bundle_with_light_dark_svg_paint, temp_runtime_path,
};
use crate::markdown::color_preset::DiagramColorPreset;

#[path = "js_runtime_html_dom_visible_text_tests.rs"]
mod visible_text;

#[test]
fn fake_bundle_wraps_drawio_html_text_within_its_explicit_frame() {
    let path = temp_runtime_path("kdr-drawio-html-wrap-unit");
    let bundle = fake_bundle().replace("  svg.appendChild(text);", HTML_WRAP_BUNDLE_HOOK);
    assert!(std::fs::write(&path, bundle).is_ok());

    let rendered =
        DrawioJsRuntimeOps::render("<mxGraphModel />", &path, DiagramColorPreset::light());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"data-wrapped-client="198x144:198x144""#)),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_honors_numeric_html_line_height() {
    let path = temp_runtime_path("kdr-drawio-html-line-height-unit");
    let bundle = fake_bundle().replace(
        "  svg.appendChild(text);",
        r#"  svg.appendChild(text);
  const label = document.createElement("div");
  label.setAttribute("style", "font-size: 30px; line-height: 1.2;");
  label.textContent = "Periodic Table of Elements";
  svg.appendChild(label);
  svg.setAttribute("data-html-line-height", String(label.clientHeight));"#,
    );
    assert!(std::fs::write(&path, bundle).is_ok());

    let rendered =
        DrawioJsRuntimeOps::render("<mxGraphModel />", &path, DiagramColorPreset::light());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"data-html-line-height="36""#)),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_measures_comment_only_drawio_html_label_as_zero_sized() {
    let path = temp_runtime_path("kdr-drawio-empty-html-label-unit");
    let bundle = fake_bundle().replace(
        "  svg.appendChild(text);",
        r#"  svg.appendChild(text);
  const foreignObject = document.createElementNS("http://www.w3.org/2000/svg", "foreignObject");
  const emptyLabel = document.createElement("div");
  emptyLabel.innerHTML = "<!-->";
  foreignObject.appendChild(emptyLabel);
  svg.appendChild(foreignObject);
  svg.setAttribute(
    "data-empty-label-client",
    `${emptyLabel.textContent.length}:${emptyLabel.clientWidth}x${emptyLabel.clientHeight}`,
  );"#,
    );
    assert!(std::fs::write(&path, bundle).is_ok());

    let rendered =
        DrawioJsRuntimeOps::render("<mxGraphModel />", &path, DiagramColorPreset::light());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"data-empty-label-client="0:0x0""#)),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_preserves_html_text_foreign_object() {
    let path = temp_runtime_path("kdr-drawio-html-label-unit");
    assert!(std::fs::write(&path, fake_bundle_with_foreign_object()).is_ok());

    let rendered =
        DrawioJsRuntimeOps::render("<mxGraphModel />", &path, DiagramColorPreset::light());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains("<foreignObject"))
    );
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| svg.contains(r#"xmlns="http://www.w3.org/1999/xhtml""#)),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_does_not_double_convert_light_dark_html_text_color() {
    let path = temp_runtime_path("kdr-drawio-light-dark-html-text-unit");
    assert!(std::fs::write(&path, fake_bundle_with_foreign_object()).is_ok());

    let rendered =
        DrawioJsRuntimeOps::render("<mxGraphModel />", &path, DiagramColorPreset::dark());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| { svg.contains("color: #ffffff") && !svg.contains("color: #121212") }),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_does_not_double_convert_light_dark_svg_paint() {
    let path = temp_runtime_path("kdr-drawio-light-dark-svg-paint-unit");
    assert!(std::fs::write(&path, fake_bundle_with_light_dark_svg_paint()).is_ok());

    let rendered =
        DrawioJsRuntimeOps::render("<mxGraphModel />", &path, DiagramColorPreset::dark());

    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            svg.contains(r##"fill="rgb(18, 18, 18)""##)
                && svg.contains(r##"fill="#000000""##)
                && svg.contains(r##"stroke="#ffffff""##)
                && svg.contains(r##"stroke="#ededed""##)
                && !svg.contains(r##"fill="#dedede""##)
        }),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_preserves_https_source_font_in_svg() {
    let path = temp_runtime_path("kdr-drawio-source-font-unit");
    assert!(std::fs::write(&path, fake_bundle()).is_ok());

    let source = r#"<mxGraphModel><root><mxCell style="fontSource=https%3A%2F%2Ffonts.googleapis.com%2Fcss%3Ffamily%3DArchitects%2BDaughter;" /></root></mxGraphModel>"#;
    let rendered = DrawioJsRuntimeOps::render(source, &path, DiagramColorPreset::dark());

    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            svg.contains(
                "@import url(\"https://fonts.googleapis.com/css?family=Architects+Daughter\");",
            )
        }),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_ignores_html_comments_in_labels() {
    let path = temp_runtime_path("kdr-drawio-html-comment-unit");
    assert!(std::fs::write(&path, fake_bundle_with_html_comment_label()).is_ok());

    let rendered =
        DrawioJsRuntimeOps::render("<mxGraphModel />", &path, DiagramColorPreset::light());

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| !svg.contains("hidden label") && !svg.contains("!--&gt;")),
        "{rendered:?}"
    );
}

const HTML_ENTITY_SINGLE_DECODE_CASES: [(&str, &str, &str); 3] = [
    (
        r#"div.innerHTML = "zw&amp;ouml;lf &amp;mdash; &amp;Zeta;&amp;alpha;";"#,
        "zw&ouml;lf &mdash; &Zeta;&alpha;",
        "zw&amp;ouml;lf &amp;mdash; &amp;Zeta;&amp;alpha;",
    ),
    (
        r#"div.innerHTML = "zw&ouml;lf &mdash; &Zeta;&alpha;";"#,
        "zwölf — Ζα",
        "zwölf — Ζα",
    ),
    (
        r#"div.textContent = "zw&ouml;lf &mdash; &Zeta;&alpha;";"#,
        "zw&ouml;lf &mdash; &Zeta;&alpha;",
        "zw&amp;ouml;lf &amp;mdash; &amp;Zeta;&amp;alpha;",
    ),
];

#[test]
fn fake_bundle_decodes_double_encoded_named_html_entities() -> Result<(), String> {
    for (case, (assignment, text, serialized)) in
        HTML_ENTITY_SINGLE_DECODE_CASES.into_iter().enumerate()
    {
        let path = temp_runtime_path(&format!("kdr-drawio-html-entity-unit-{case}"));
        let expected_text = serde_json::to_string(text).map_err(|error| error.to_string())?;
        let setup = format!(
            r#"{assignment}
if (div.textContent !== {expected_text}) throw new Error("HTML entity text mismatch");
div.setAttribute("data-entity-text", div.textContent);"#,
        );
        let bundle =
            FAKE_BUNDLE_WITH_FOREIGN_OBJECT.replace(r#"div.textContent = "html label";"#, &setup);
        assert!(std::fs::write(&path, bundle).is_ok());

        let rendered =
            DrawioJsRuntimeOps::render("<mxGraphModel />", &path, DiagramColorPreset::light());
        let expected_attribute = format!(r#"data-entity-text="{serialized}""#);
        let expected_body = format!(">{serialized}<br/>");
        assert!(
            rendered.as_ref().is_ok_and(|svg| {
                svg.contains(&expected_attribute) && svg.contains(&expected_body)
            }),
            "assignment={assignment}: {rendered:?}"
        );
    }
    Ok(())
}
