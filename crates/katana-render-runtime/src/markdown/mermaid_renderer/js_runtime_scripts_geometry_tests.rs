#[test]
fn runtime_normalizes_requirement_viewbox_to_browser_bounds() -> Result<(), String> {
    let first = original_synthetic_svg(fake_mermaid_with_requirement_viewbox())?;
    let second = vary_synthetic_geometry(&first)?;
    assert_fake_svg_geometry_is_source_independent(
        [first, second],
        [
            "requirementDiagram\nrequirement test_req { id: 1 }",
            "requirementDiagram\nrequirement second_req { id: 2 }",
        ],
    )
}

#[test]
fn runtime_preserves_fake_raw_class_geometry_and_generic_row_class() -> Result<(), String> {
    for (first, second, view_box, max_width) in [
        ("PreviewPane", "RenderedSection", "4 4 311.5 367", "311.5"),
        ("OtherPane", "OtherSection", "-7 -11 413.25 509.5", "413.25"),
    ] {
        let raw = independent_raw_class_geometry(first, second, view_box, max_width);
        let source = format!("classDiagram\nclass {first}\nclass {second}\n{first} --> {second}");
        let request = serde_json::json!({
            "source": source, "svgId": "id", "theme": "dark", "background": "#000",
            "fill": "#111", "text": "#fff", "stroke": "#fff", "arrow": "#fff",
        })
        .to_string();
        let fake = fake_mermaid_with_class_enumeration_layout(&raw);
        let scripts = MermaidRuntimeScripts::build_with_zenuml(&fake, "", &request);
        let rendered = DiagramV8Runtime::render(&scripts)?;
        /* WHY: 一般のrowクラス付与だけを許し、入力定義から独立した形状とpaintの全byteを保持する。 */
        let expected = raw.replace(
            "class=\"text-outer-tspan\"",
            "class=\"text-outer-tspan row\"",
        );
        assert_eq!(rendered, expected);
        assert!(!rendered.contains("235.1796875"));
        assert!(!rendered.contains("M117.59,146L117.59,150.167"));
    }
    Ok(())
}

#[test]
fn runtime_normalizes_mindmap_fixture_layout_to_browser_bounds() -> Result<(), String> {
    let source = "mindmap\n  root((mindmap))";
    let first = original_mindmap_fixture_svg(0);
    assert_eq!(svg_attribute_value(&first, "x"), None);
    let with_x = first.replacen("<path ", "<path x=\"17\" ", 1);
    assert_eq!(svg_attribute_value(&with_x, "x"), Some("17"));
    assert_eq!(
        render_fake_mermaid_for_source(fake_mermaid_with_mindmap_layout(), source)?,
        render_fake_svg_for_source(&first, source)?,
    );
    let normalized = render_fake_svg_for_source(&first, source)?;
    let namespaced = normalized.replacen("<svg ", "<svg xmlns=\"http://www.w3.org/2000/svg\" ", 1);
    assert!(matches!(
        assert_synthetic_svg_contract(&normalized, &namespaced),
        Err(error) if error.contains("root namespace")
    ));
    let second = original_mindmap_fixture_svg(1);
    assert_fake_svg_geometry_is_source_independent(
        [first, second],
        [source, "mindmap\n  root((mindmap revised))"],
    )
}

#[test]
fn runtime_normalizes_simple_mindmap_fixture_layout_to_browser_bounds() -> Result<(), String> {
    let source = "mindmap\n  root((Mermaid))";
    let first = original_simple_mindmap_fixture_svg(0);
    assert_eq!(
        render_fake_mermaid_for_source(fake_mermaid_with_simple_mindmap_layout(), source)?,
        render_fake_svg_for_source(&first, source)?,
    );
    let second = original_simple_mindmap_fixture_svg(1);
    assert_fake_svg_geometry_is_source_independent(
        [first, second],
        [source, "mindmap\n  root((Mermaid revised))"],
    )
}

#[test]
fn runtime_normalizes_localized_requirement_layout_to_browser_bounds() -> Result<(), String> {
    let first = original_synthetic_svg(fake_mermaid_with_localized_requirement_layout())?;
    let second = vary_synthetic_geometry(&first)?;
    assert_fake_svg_geometry_is_source_independent(
        [first, second],
        [
            "requirementDiagram\nrequirement テスト要件 { id: 1 }",
            "requirementDiagram\nrequirement   テスト要件 { id: 2 }",
        ],
    )
}

#[test]
fn runtime_normalizes_japanese_sequence_activation_layout_to_browser_bounds() -> Result<(), String>
{
    let first = original_synthetic_svg(fake_mermaid_with_japanese_sequence_activation_layout())?;
    let second = vary_synthetic_geometry(&first)?;
    assert_fake_svg_geometry_is_source_independent(
        [first, second],
        [
            "sequenceDiagram\n田中->>+鈴木: こんにちは鈴木さん、お元気ですか？",
            "sequenceDiagram\n佐藤->>+高橋: 今日はよろしくお願いします",
        ],
    )
}

#[test]
fn runtime_normalizes_english_sequence_activation_layout_to_browser_bounds() -> Result<(), String> {
    let first = original_synthetic_svg(fake_mermaid_with_english_sequence_activation_layout())?;
    let second = vary_synthetic_geometry(&first)?;
    assert_fake_svg_geometry_is_source_independent(
        [first, second],
        [
            "sequenceDiagram\nAlice->>+John: Hello John, how are you?",
            "sequenceDiagram\nMary->>+Paul: Welcome, how are you?",
        ],
    )
}

#[test]
fn runtime_normalizes_c4_review_keeps_svg_when_source_does_not_match() {
    let rendered = render_mermaid_request(
        fake_mermaid_with_c4_review_svg(),
        "",
        r##"{"source":"graph TD\nA-->B","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
    );

    assert!(
        rendered
            .as_ref()
            .is_ok_and(|it| it.contains(r#"style="max-width: 300px;""#)
                && it.contains(r#"viewBox="0 0 300 200""#)
                && !it.contains(r#"viewBox="0 -70 1148 441""#)),
        "{rendered:?}"
    );
}

#[test]
fn runtime_normalizes_c4_review_changes_svg_when_source_matches() -> Result<(), String> {
    let first = original_synthetic_svg(fake_mermaid_with_c4_review_svg())?;
    let second = vary_synthetic_geometry(&first)?;
    assert_fake_svg_geometry_is_source_independent(
        [first, second],
        [
            "C4Context\nPerson(a,\"A\")",
            "C4Context \n Person(b, \"B\")",
        ],
    )
}

#[test]
fn runtime_reads_html_line_breaks_as_visible_text() {
    let rendered = render_mermaid_request(
        fake_mermaid_with_html_line_break(),
        "",
        r##"{"source":"mindmap\n  root((mindmap))","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
    );

    assert_rendered_contains(&rendered, "On effectiveness\nand features");
}

#[test]
fn runtime_measures_svg_without_viewbox_with_dom_metrics_helper() {
    let rendered = render_mermaid_request(
        fake_mermaid_measuring_svg_without_viewbox(),
        "",
        r##"{"source":"graph TD; A-->B","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
    );

    assert_rendered_contains(&rendered, "800:600");
}

#[test]
fn synthetic_geometry_helpers_reject_malformed_inputs() -> Result<(), String> {
    assert_malformed_state_helpers();
    assert_malformed_svg_attributes()?;
    assert_malformed_svg_geometry()
}

#[test]
fn synthetic_svg_geometry_parsers_reject_malformed_components() {
    assert!(parse_synthetic_translate("translate(nope, 2)").is_err());
    assert_eq!(
        parse_synthetic_translate("translate(1)").err().as_deref(),
        Some("expected two translate components")
    );
    assert!(parse_view_box("0 0 nope 1").is_err());
    assert_eq!(
        parse_view_box("0 0 1").err().as_deref(),
        Some("expected four viewBox components")
    );
}

#[test]
fn c4_metrics_reject_non_numeric_and_f32_overflow_values() {
    assert_eq!(
        c4_native_f32(&serde_json::Value::Null).err().as_deref(),
        Some("finite SVG number missing")
    );
    assert_eq!(
        c4_native_f32(&serde_json::json!(f64::MAX)).err().as_deref(),
        Some("SVG number exceeds f32 range")
    );
}

#[test]
fn synthetic_svg_root_namespace_contract_rejects_changes() {
    assert_eq!(
        synthetic_root_attributes(r#"<svg xmlns="urn:unsupported"/>"#)
            .err()
            .as_deref(),
        Some("root must be SVG")
    );
    assert_eq!(
        assert_synthetic_root_namespace_is_preserved(
            r#"<svg xmlns="urn:source"/>"#,
            r#"<svg xmlns="urn:source"/>"#,
        ),
        Ok(())
    );
    let mismatch = assert_synthetic_root_namespace_is_preserved(
        r#"<svg xmlns="urn:source"/>"#,
        r#"<svg xmlns="urn:rendered"/>"#,
    );
    assert!(
        mismatch.err().is_some_and(|message| {
            message.starts_with("root namespace must remain identical:")
        })
    );
}

#[test]
fn removed_fixture_geometry_corrections_preserve_synthetic_svg_geometry() -> Result<(), String> {
    for (raw, sources) in FIXTURE_SENSITIVE_GEOMETRY_CASES {
        let varied = vary_synthetic_geometry(raw)?.replacen(r#"y="157""#, r#"y="163""#, 1);
        assert_fake_svg_geometry_is_source_independent([raw.to_owned(), varied], sources)?;
    }
    Ok(())
}
