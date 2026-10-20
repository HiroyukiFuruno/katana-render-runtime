#[test]
fn build_includes_bundle_and_render_script() {
    let scripts = MermaidRuntimeScripts::build_with_zenuml("bundle", "plugin", "{}");
    assert!(scripts.iter().any(|it| it.name == "mermaid-runtime.min.js"));
    assert!(scripts.iter().any(|it| it.name == "mermaid.min.js"));
    assert!(scripts.iter().any(|it| it.name == "mermaid-zenuml.min.js"));
    assert!(scripts.iter().any(|it| it.name == "render-mermaid.js"));
}

#[test]
fn invalid_request_json_does_not_load_zenuml_runtime() {
    let scripts = MermaidRuntimeScripts::build_with_zenuml("bundle", "plugin", "not-json");

    assert!(!scripts.iter().any(|it| it.name == "zenuml-runtime.min.js"));
}

#[test]
fn zenuml_registration_runs_before_render() {
    let rendered = render_mermaid_request(
        fake_mermaid(),
        fake_zenuml(),
        r##"{"source":"zenuml\nA.method()","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff","diagramType":"zenuml"}"##,
    );

    assert_rendered_contains(&rendered, "registered");
}

#[test]
fn zenuml_directive_source_registers_external_diagram_without_request_hint() {
    let rendered = render_mermaid_request(
        fake_mermaid(),
        fake_zenuml(),
        r##"{"source":"%%{init: { \"theme\": \"dark\" }}%%\n%% comment\nzenuml\nA.method()","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
    );

    assert_rendered_contains(&rendered, "registered");
}

#[test]
fn runtime_provides_constructable_style_sheet() {
    let rendered = render_mermaid_request(
        fake_mermaid_with_style_sheet(),
        "",
        r##"{"source":"graph TD; A-->B","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
    );

    assert_rendered_contains(&rendered, "font-family: sans-serif");
}

#[test]
fn runtime_preserves_c4_description_with_explicit_line_break() {
    let scripts = MermaidRuntimeScripts::build(
        mermaid_test_bundle(),
        r##"{"source":"C4Context\nPerson(customer, \"顧客\", \"1行目<br/> 2行目\")","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff","diagramType":"c4"}"##,
        MermaidDiagramType::Other,
    );

    let rendered = DiagramV8Runtime::render(&scripts);

    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            svg.contains(">1行目</tspan>") && svg.contains(">2行目</tspan>")
        }),
        "{rendered:?}"
    );
}

#[test]
#[should_panic(expected = "unexpected test error: boom")]
fn must_asset_bytes_reports_unexpected_test_errors() {
    let _ = must_asset_bytes(Err("boom".to_string()));
}

#[test]
#[should_panic(expected = "invalid utf-8")]
fn must_utf8_reports_unexpected_test_errors() {
    let _ = must_utf8(std::str::from_utf8(&[u8::MAX]));
}

#[test]
fn runtime_measures_horizontal_svg_arcs_like_a_browser() {
    let rendered = render_mermaid_request(
        fake_mermaid_measuring_horizontal_svg_arcs(),
        "",
        r##"{"source":"C4Container\nContainerDb(db, \"DB\", \"SQL\")","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff","diagramType":"c4"}"##,
    );

    assert_rendered_contains(&rendered, "0:0:216:121.673");
}

#[test]
fn runtime_measures_vertical_svg_arcs_like_a_browser() {
    let rendered = render_mermaid_request(
        fake_mermaid_measuring_vertical_svg_arcs(),
        "",
        r##"{"source":"C4Context\nSystemQueue(queue, \"Queue\")","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff","diagramType":"c4"}"##,
    );

    assert_rendered_contains(
        &rendered,
        "-9.26887261930837:-73.650734:243.80661785792515:73.650734",
    );
}

#[test]
fn runtime_measures_repeated_relative_curves_from_each_segment_origin() {
    let rendered = render_mermaid_request(
        fake_mermaid_measuring_repeated_relative_curves(),
        "",
        r##"{"source":"graph TD; A-->B","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
    );

    assert_rendered_contains(&rendered, "10:10:60:0");
}

#[test]
fn runtime_matches_browser_c4_canvas_text_width() {
    let rendered = render_mermaid_request(
        fake_mermaid_measuring_c4_canvas_text(),
        "",
        r##"{"source":"C4Container\nContainerDb(db, \"DB\", \"SQL\")","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff","diagramType":"c4"}"##,
    );

    assert_rendered_contains(&rendered, "54.279");
}

#[test]
fn c4_wide_svg_width_preserves_native_raw_advance() -> Result<(), String> {
    assert_c4_native_width_contract(&[(
        "リレーショナルデータベーススキ",
        "Open Sans, sans-serif",
        10.5,
    )])
}

#[test]
fn c4_relation_svg_widths_preserve_native_raw_advance() -> Result<(), String> {
    assert_c4_native_width_contract(&[
        ("Uses", "Open Sans, sans-serif", 12.0),
        ("[HTTPS]", "Open Sans, sans-serif", 12.0),
        ("Reads from and writes to", "Open Sans, sans-serif", 12.0),
        ("利用する", "Open Sans, sans-serif", 12.0),
        ("AVATAR", "Arial", 14.0),
        ("日本語の幅", "Hiragino Sans", 16.0),
    ])
}

#[test]
fn runtime_respects_nearest_c4_font_weight() {
    let rendered = render_mermaid_request(
        fake_mermaid_measuring_nearest_c4_font_weight(),
        "",
        r##"{"source":"C4Context\nSystemQueue(queue, \"Queue\")","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff","diagramType":"c4"}"##,
    );

    assert_rendered_contains(&rendered, ">normal<");
}

#[test]
fn runtime_applies_descendant_css_text_anchor_to_text_measurement() {
    let rendered = render_mermaid_request(
        fake_mermaid_with_descendant_text_anchor(),
        "",
        r##"{"source":"block-beta\nA[\"Markdown\"]","svgId":"id","theme":"dark","background":"#000","fill":"#111","text":"#fff","stroke":"#fff","arrow":"#fff"}"##,
    );

    assert_rendered_contains(&rendered, "<text>-");
}

#[test]
fn runtime_preserves_er_vendor_viewbox_and_geometry() -> Result<(), String> {
    let first = original_synthetic_svg(fake_mermaid_with_er_viewbox())?;
    let second = vary_synthetic_geometry(&first)?;
    assert_fake_vendor_svg_geometry_is_source_independent(
        [first, second],
        [
            "erDiagram\nDOCUMENT ||--o{ SECTION : contains",
            "erDiagram\nREPORT ||--o{ PAGE : contains",
        ],
    )
}

#[test]
fn runtime_preserves_state_viewbox_and_label_geometry() -> Result<(), String> {
    let cases = [
        ("-8 -8 184.711 295", "184.711px", "translate(1, -9.5)", None),
        (
            "4 4 173 329",
            "173px",
            "translate(-60, -9.5)",
            Some("start"),
        ),
        ("4 0 173 333", "173px", "translate(13.5, 2)", Some("end")),
        ("0 0 173 333", "173px", "translate(0, -9.5)", Some("middle")),
    ];
    for (view_box, max_width, transform, text_anchor) in cases {
        assert_state_case(view_box, max_width, transform, text_anchor)?;
    }
    Ok(())
}
