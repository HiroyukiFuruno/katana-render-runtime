use super::oracle::assert_bridge_widths_match_rust;
use crate::markdown::diagram_js_runtime::{DiagramRuntimeScript, DiagramV8Runtime};
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use serde_json::{Value, json};

pub(super) fn actual_v8_calls(script_label: &'static str) -> Result<Vec<Value>, String> {
    let script = r#"
const cssArgs = ['AV', 'Noto Sans', 16, 400, false];
const svgArgs = ['AV', 'Noto Sans', 16, 400, false];
const cssWidth = globalThis.__katanaMeasureText(...cssArgs);
const svgMetrics = globalThis.__katanaMeasureSvgTextMetrics(...svgArgs);
JSON.stringify([
  {kind: 'css', args: cssArgs, width: cssWidth},
  {kind: 'svg', args: svgArgs, width: svgMetrics.advance, metrics: svgMetrics},
]);
"#;
    let output = SvgRasterizeOps::with_html_font_render_scope(|| {
        DiagramV8Runtime::render(&[DiagramRuntimeScript::borrowed(script_label, script)])
    })?;
    let calls: Vec<Value> = serde_json::from_str(&output).map_err(|error| error.to_string())?;
    assert!(calls.iter().any(|call| call["kind"] == "css"));
    assert!(calls.iter().any(|call| call["kind"] == "svg"));
    assert_bridge_widths_match_rust(&calls)?;
    Ok(calls)
}

#[test]
fn raw_v8_css_call_rejects_kind_arguments_and_width_corruption() -> Result<(), String> {
    let calls = actual_v8_calls("native-width-oracle-boundary.js")?;
    let css_index = calls
        .iter()
        .position(|call| call["kind"] == "css")
        .ok_or("raw V8 CSS call missing")?;

    let mut bad_kind = calls.clone();
    bad_kind[css_index]["kind"] = json!("unknown");
    assert!(assert_bridge_widths_match_rust(&bad_kind).is_err());

    let mut bad_arguments = calls.clone();
    bad_arguments[css_index]["args"] = json!(["AV", "Noto Sans", 16, 400]);
    assert!(assert_bridge_widths_match_rust(&bad_arguments).is_err());

    let mut bad_width = calls;
    bad_width[css_index]["width"] = Value::Null;
    assert!(assert_bridge_widths_match_rust(&bad_width).is_err());
    Ok(())
}

#[test]
fn raw_v8_svg_call_rejects_kind_arguments_width_and_metric_corruption() -> Result<(), String> {
    let calls = actual_v8_calls("native-width-oracle-boundary.js")?;
    let svg_index = calls
        .iter()
        .position(|call| call["kind"] == "svg")
        .ok_or("raw V8 SVG call missing")?;
    assert_svg_corruptions_rejected(&calls, svg_index);
    Ok(())
}

fn assert_svg_corruptions_rejected(calls: &[Value], svg_index: usize) {
    let mut bad_kind = calls.to_vec();
    bad_kind.push(calls[svg_index].clone());
    bad_kind[svg_index]["kind"] = json!("unknown");
    assert!(assert_bridge_widths_match_rust(&bad_kind).is_err());

    let mut bad_arguments = calls.to_vec();
    bad_arguments[svg_index]["args"] = json!(["AV", "Noto Sans", 16, 400]);
    assert!(assert_bridge_widths_match_rust(&bad_arguments).is_err());

    let mut bad_width = calls.to_vec();
    bad_width[svg_index]["width"] = Value::Null;
    assert!(assert_bridge_widths_match_rust(&bad_width).is_err());

    for metric in ["advance", "ascent", "descent"] {
        let mut bad_metric = calls.to_vec();
        bad_metric[svg_index]["metrics"][metric] = Value::Null;
        assert!(
            assert_bridge_widths_match_rust(&bad_metric).is_err(),
            "{metric}"
        );
    }
}
