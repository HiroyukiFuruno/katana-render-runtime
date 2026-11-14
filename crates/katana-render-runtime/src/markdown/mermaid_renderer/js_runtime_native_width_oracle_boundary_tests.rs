use std::panic::AssertUnwindSafe;

use serde_json::{Value, json};

use super::oracle::assert_bridge_widths_match_rust;
use crate::markdown::diagram_js_runtime::{DiagramRuntimeScript, DiagramV8Runtime};
use crate::markdown::svg_rasterize::SvgRasterizeOps;

const CANVAS_SEGMENTS_ARGUMENT_INDEX: usize = 5;
const CANVAS_WHOLE_ARGUMENT_COUNT: usize = 5;
const CANVAS_SEGMENTED_ARGUMENT_COUNT: usize = 6;
const CANVAS_SEGMENT_COUNT: usize = 2;
const RAW_BRIDGE_SCRIPT: &str = r#"
const CANVAS_SEGMENTS_ARGUMENT_INDEX = 5;
const cssArgs = ['AV', 'Noto Sans', 16, 400, false];
const svgArgs = ['AV', 'Noto Sans', 16, 400, false];
const canvasArgs = ['AV', 'Noto Sans', 16, 400, false];
const canvasSegmentsArgs = ['AV', 'Noto Sans', 16, 400, false, ['A', 'V']];
const cssWidth = globalThis.__katanaMeasureText(...cssArgs);
const svgMetrics = globalThis.__katanaMeasureSvgTextMetrics(...svgArgs);
const canvasWidth = globalThis.__katanaMeasureCanvasText(...canvasArgs);
const canvasSegmentsWidth = globalThis.__katanaMeasureCanvasText(...canvasSegmentsArgs);
JSON.stringify([
  {kind: 'css', args: cssArgs, width: cssWidth},
  {kind: 'svg', args: svgArgs, width: svgMetrics.advance, metrics: svgMetrics},
  {kind: 'canvas', args: canvasArgs, width: canvasWidth},
  {
    kind: 'canvas',
    args: canvasSegmentsArgs,
    width: canvasSegmentsWidth,
    segments: canvasSegmentsArgs[CANVAS_SEGMENTS_ARGUMENT_INDEX],
  },
]);
"#;

pub(super) fn actual_v8_calls(script_label: &'static str) -> Result<Vec<Value>, String> {
    let output = SvgRasterizeOps::with_html_font_render_scope(|| {
        DiagramV8Runtime::render(&[DiagramRuntimeScript::borrowed(
            script_label,
            RAW_BRIDGE_SCRIPT,
        )])
    })?;
    let calls: Vec<Value> = serde_json::from_str(&output).map_err(|error| error.to_string())?;
    assert_actual_call_shapes(&calls);
    assert_bridge_widths_match_rust(&calls)?;
    Ok(calls)
}

fn assert_actual_call_shapes(calls: &[Value]) {
    assert!(calls.iter().any(|call| call["kind"] == "css"));
    assert!(calls.iter().any(|call| call["kind"] == "svg"));
    assert!(calls.iter().any(|call| {
        call["kind"] == "canvas"
            && call["args"]
                .as_array()
                .is_some_and(|args| args.len() == CANVAS_WHOLE_ARGUMENT_COUNT)
    }));
    assert!(calls.iter().any(|call| {
        call["kind"] == "canvas"
            && call["args"]
                .as_array()
                .is_some_and(|args| args.len() == CANVAS_SEGMENTED_ARGUMENT_COUNT)
            && call["segments"]
                .as_array()
                .is_some_and(|segments| segments.len() == CANVAS_SEGMENT_COUNT)
    }));
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
fn raw_v8_svg_call_rejects_missing_text_and_family() -> Result<(), String> {
    let calls = actual_v8_calls("native-width-oracle-missing-text")?;
    let index = calls
        .iter()
        .position(|call| call["kind"] == "svg")
        .ok_or("raw V8 SVG call missing")?;
    for (argument, expected) in [
        (0, "text argument 0 missing"),
        (1, "text argument 1 missing"),
    ] {
        let mut changed = calls.clone();
        changed[index]["args"][argument] = Value::Null;
        assert_eq!(
            assert_bridge_widths_match_rust(&changed).err().as_deref(),
            Some(expected)
        );
    }
    Ok(())
}

#[test]
fn raw_v8_svg_call_rejects_unmeasurable_text_and_size() -> Result<(), String> {
    let calls = actual_v8_calls("native-width-oracle-invalid-measurement")?;
    let index = calls
        .iter()
        .position(|call| call["kind"] == "svg")
        .ok_or("raw V8 SVG call missing")?;
    for (argument, value) in [(0, json!("A\u{0}V")), (2, json!(-1))] {
        let mut changed = calls.clone();
        changed[index]["args"][argument] = value;
        let error = assert_bridge_widths_match_rust(&changed)
            .err()
            .ok_or("invalid SVG text measurement was accepted")?;
        assert!(
            error.starts_with("SVG boundary reference failed:"),
            "{error}"
        );
    }
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
    let mut no_metrics = calls.clone();
    no_metrics[svg_index]
        .as_object_mut()
        .ok_or("raw V8 SVG call is not an object")?
        .remove("metrics")
        .ok_or("raw V8 SVG metrics missing before mutation")?;
    assert_eq!(
        assert_bridge_widths_match_rust(&no_metrics)
            .err()
            .as_deref(),
        Some("SVG metrics missing")
    );
    let mut overflow = calls;
    overflow[svg_index]["width"] = json!(f64::MAX);
    assert_eq!(
        assert_bridge_widths_match_rust(&overflow).err().as_deref(),
        Some("SVG number exceeds f32 range")
    );
    Ok(())
}

#[test]
fn raw_v8_canvas_call_rejects_malformed_segments() -> Result<(), String> {
    let calls = actual_v8_calls("native-width-oracle-boundary.js")?;
    let canvas_index = calls
        .iter()
        .position(|call| {
            call["kind"] == "canvas"
                && call["args"]
                    .as_array()
                    .is_some_and(|args| args.len() == CANVAS_SEGMENTED_ARGUMENT_COUNT)
        })
        .ok_or("segmented Canvas call missing")?;
    for malformed in [
        json!(["A", 1]),
        json!(["A", "", "V"]),
        json!(["A"]),
        Value::Null,
        json!("AV"),
    ] {
        assert_canvas_rejection(&calls, canvas_index, malformed);
    }
    Ok(())
}

fn assert_canvas_rejection(calls: &[Value], canvas_index: usize, segments: Value) {
    let mut corrupted = calls.to_vec();
    corrupted[canvas_index]["args"][CANVAS_SEGMENTS_ARGUMENT_INDEX] = segments;
    let rejected = std::panic::catch_unwind(AssertUnwindSafe(|| {
        assert_bridge_widths_match_rust(&corrupted)
    }))
    .map_or(true, |result| result.is_err());
    assert!(rejected, "malformed Canvas segments were accepted");
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
