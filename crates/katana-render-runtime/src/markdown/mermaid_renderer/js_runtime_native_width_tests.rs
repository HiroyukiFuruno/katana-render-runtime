#[path = "js_runtime_native_width_c4_description_tests.rs"]
mod c4_description_tests;
#[path = "js_runtime_native_width_c4_tests.rs"]
mod c4_tests;
#[path = "js_runtime_native_width_helpers.rs"]
mod helpers;
#[path = "js_runtime_native_width_ishikawa_tests.rs"]
mod ishikawa_tests;
#[path = "js_runtime_native_width_oracle.rs"]
mod oracle;
#[path = "js_runtime_native_width_oracle_boundary_tests.rs"]
mod oracle_boundary_tests;
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use helpers::{render_fixture, render_fixture_with_font_audit};
use oracle::{
    assert_bridge_widths_match_rust, assert_inherited_cjk, call_text, call_weight, rounded_width,
};
use serde_json::Value;

const FLOWCHART_SOURCE: &str = r#"%%{init: {"themeVariables": {"fontFamily": "Noto Sans"}}}%%
flowchart LR
  parent["日本語 AV"]
  bold["太字 日本語"]
  parent --> bold
  style parent font-weight:400
  style bold font-weight:700"#;
const STRONG_FLOWCHART_SOURCE: &str = r#"%%{init: {"themeVariables": {"fontFamily": "Noto Sans"}}}%%
flowchart LR
  parent["日本語 AV"]
  bold["`**太字 日本語**`"]
  parent --> bold
  style parent font-weight:400
  style bold font-weight:700"#;
const ARCHITECTURE_SOURCE: &str = r#"architecture-beta
group app(cloud)[KatanA]
service markdown(server)[Markdown] in app
service renderer(server)[Renderer] in app
service svg(database)[SVG cache] in app
markdown:R -- L:renderer
renderer:R -- L:svg"#;
const ARCHITECTURE_BETA_FIXTURE_MARKDOWN: &str =
    include_str!("../../../../../tests/fixtures/mermaid/ja/09-02-architecture-beta.md");

#[test]
fn nonclass_flowchart_uses_native_advance_for_noto_and_weighted_cjk_runs() -> Result<(), String> {
    let plain = render_fixture(FLOWCHART_SOURCE)?;
    assert_label_run_weights(&plain.calls, 400);
    assert_bridge_widths_match_rust(&plain.calls)?;
    /* WHY: Mermaidは単語ごとにtspanを生成し、通常ラベルには子のnormal指定を付ける。 */
    /* WHY: 明示的なMarkdown太字で、分割後の各runが実Rust境界へ700で渡ることを確認する。 */
    let result = render_fixture(STRONG_FLOWCHART_SOURCE)?;
    assert_label_run_weights(&result.calls, 700);
    assert!(
        result
            .calls
            .iter()
            .any(|call| call_text(call) == "日本語" && call_weight(call) == Some(400))
    );
    assert_bridge_widths_match_rust(&result.calls)
}

fn assert_label_run_weights(actual: &[Value], weight: u64) {
    for text in ["太字", " 日本語"] {
        let calls: Vec<_> = actual
            .iter()
            .filter(|call| call_text(call) == text)
            .collect();
        assert!(
            !calls.is_empty() && calls.iter().all(|call| call_weight(call) == Some(weight)),
            "Mermaid run {text:?} did not consistently enter native bridge at {weight}: {actual:?}"
        );
    }
}

#[test]
fn nonclass_architecture_uses_native_advance() -> Result<(), String> {
    let result = render_fixture(ARCHITECTURE_SOURCE)?;
    assert!(
        result
            .calls
            .iter()
            .any(|call| call_text(call).contains("Markdown")),
        "actual architecture did not invoke native bridge"
    );
    assert_bridge_widths_match_rust(&result.calls)
}

#[test]
fn localized_architecture_service_bbox_includes_wide_label() -> Result<(), String> {
    let original = render_fixture(ARCHITECTURE_SOURCE)?;
    let source = ARCHITECTURE_SOURCE
        .replace("[Renderer]", "[レンダラー]")
        .replace("[SVG cache]", "[SVGキャッシュ]");
    let result = render_fixture(&source)?;
    println!(
        "architecture_bbox_observation={}",
        serde_json::json!({"boxes": &result.boxes, "calls": &result.calls})
    );
    for expected in ["レンダラー", "SVGキャッシュ"] {
        assert!(
            result.calls.iter().any(|call| call_text(call) == expected),
            "native bridge did not measure original architecture label {expected:?}: {:?}",
            result.calls
        );
    }
    let (original_id, original_width) = service_bbox(&original, "SVG cache")?;
    let (localized_id, localized_width) = service_bbox(&result, "SVGキャッシュ")?;
    assert_eq!(original_id, localized_id, "service identity changed");
    assert!(
        localized_width > original_width,
        "localized service bbox did not grow for the wider label: english={original_width}, localized={localized_width}"
    );
    Ok(())
}

#[test]
fn architecture_beta_fixture_records_native_service_bbox_inputs() -> Result<(), String> {
    let source = architecture_beta_source()?;
    let (result, font_audit) = render_fixture_with_font_audit(source)?;
    assert_architecture_beta_labels(&result)?;
    println!(
        "architecture_beta_bbox_observation={}",
        serde_json::json!({"boxes": &result.boxes, "calls": &result.calls, "fontAudit": &font_audit})
    );
    #[cfg(target_os = "macos")]
    assert!(
        font_audit["faces"]
            .as_array()
            .is_some_and(|faces| !faces.is_empty()),
        "actual CSS shaping did not record a selected physical face: {font_audit}"
    );
    Ok(())
}

fn architecture_beta_source() -> Result<&'static str, String> {
    mermaid_fixture_source(ARCHITECTURE_BETA_FIXTURE_MARKDOWN)
}

fn mermaid_fixture_source(markdown: &str) -> Result<&str, String> {
    markdown
        .split("~~~mermaid\n")
        .nth(1)
        .and_then(|value| value.split("\n~~~").next())
        .ok_or_else(|| "fixture Mermaid source missing".to_string())
}

#[test]
fn mermaid_fixture_source_rejects_missing_source_fence() {
    for markdown in ["", "# No diagram\n\n~~~text\nexample\n~~~"] {
        assert!(mermaid_fixture_source(markdown).is_err());
    }
}

fn assert_architecture_beta_labels(result: &helpers::MeasurementFixture) -> Result<(), String> {
    let labels = ["データベース", "ストレージ", "サーバー"];
    for label in labels {
        assert!(
            result.calls.iter().any(|call| call_text(call) == label),
            "native bridge did not measure architecture label {label:?}: {:?}",
            result.calls
        );
        assert!(
            result.calls.iter().any(|call| {
                call_text(call) == label
                    && matches!(
                        call.get("kind").and_then(Value::as_str),
                        Some("css") | Some("canvas")
                    )
            }),
            "Canvas native bridge did not measure architecture label {label:?}: {:?}",
            result.calls
        );
        service_bbox(result, label)?;
    }
    Ok(())
}

fn service_bbox(
    result: &helpers::MeasurementFixture,
    label: &str,
) -> Result<(String, f64), String> {
    let service = result
        .boxes
        .iter()
        .find(|entry| {
            entry["node"]["className"] == "architecture-service"
                && entry["node"]["text"]
                    .as_str()
                    .is_some_and(|text| text.contains(label))
        })
        .ok_or("architecture service getBBox observation missing")?;
    let id = service["node"]["id"]
        .as_str()
        .ok_or("architecture service id missing")?
        .to_owned();
    let service_width = service["bbox"]["width"]
        .as_f64()
        .filter(|width| width.is_finite() && *width > 0.0)
        .ok_or("architecture service bbox width missing or invalid")?;
    Ok((id, service_width))
}

#[test]
fn native_noto_and_inherited_cjk_probes_match_rust_boundary() -> Result<(), String> {
    let result = render_fixture(FLOWCHART_SOURCE)?;
    let width = result
        .probes
        .get("noto")
        .and_then(Value::as_f64)
        .ok_or("Noto sample missing")?;
    let expected =
        SvgRasterizeOps::measure_html_text("AV", "Noto Sans", 16.0, 400, false, 0.0, None)
            .map_err(|error| format!("Noto boundary failed: {error:?}"))?;
    assert_eq!(width, rounded_width(expected));
    assert_inherited_cjk(result.probes.get("inherited").ok_or("CJK probe missing")?)
}

#[test]
fn nondefault_spacing_features_and_axes_do_not_enter_nonclass_bridge() -> Result<(), String> {
    let result = render_fixture(FLOWCHART_SOURCE)?;
    assert_eq!(
        result.probes.get("skipped").and_then(Value::as_bool),
        Some(true)
    );
    Ok(())
}

#[test]
fn nondefault_font_properties_inherit_and_initial_restores_eligibility() -> Result<(), String> {
    let result = render_fixture(FLOWCHART_SOURCE)?;
    assert_eq!(
        result.probes.get("eligibility"),
        Some(&serde_json::json!([false, true]))
    );
    Ok(())
}

#[test]
fn class_nondefault_properties_preserve_css_width_and_distinct_runs() -> Result<(), String> {
    let result = render_fixture(FLOWCHART_SOURCE)?;
    assert_eq!(
        result
            .probes
            .get("classCompatible")
            .and_then(Value::as_bool),
        Some(true)
    );
    Ok(())
}

#[test]
fn native_width_oracle_accepts_raw_css_and_svg_calls_and_rejects_corruption() -> Result<(), String>
{
    let calls = oracle_boundary_tests::actual_v8_calls("native-width-raw-bridge-contract.js")?;

    let mut corrupted = calls;
    corrupted[0]["args"][2] = Value::Null;
    assert!(assert_bridge_widths_match_rust(&corrupted).is_err());
    Ok(())
}
