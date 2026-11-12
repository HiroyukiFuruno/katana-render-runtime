use crate::markdown::svg_rasterize::SvgRasterizeOps;
use serde_json::Value;
#[path = "js_runtime_native_width_oracle_call.rs"]
mod call;
#[path = "js_runtime_native_width_oracle_canvas.rs"]
mod canvas;
use call::ParsedCall;

const TEXT_METRIC_WEIGHT_ARGUMENT_INDEX: usize = 3;
const TEXT_METRIC_ITALIC_ARGUMENT_INDEX: usize = 4;
const INHERITED_SVG_FONT_SIZE_PX: f32 = 16.0;
const WIDTH_ROUNDING_SCALE: f64 = 64.0;

pub(super) fn assert_bridge_widths_match_rust(calls: &[Value]) -> Result<(), String> {
    assert!(!calls.is_empty(), "actual native calls are missing");
    assert!(
        calls
            .iter()
            .any(|call| call.get("kind").and_then(Value::as_str) == Some("svg")),
        "eligible SVG runs did not invoke the raw metric bridge: {calls:?}"
    );
    for call in calls {
        assert_call(call)?;
    }
    Ok(())
}

fn assert_call(call: &Value) -> Result<(), String> {
    let parsed = ParsedCall::from_json(call)?;
    match call.get("kind").and_then(Value::as_str) {
        Some("css") => assert_css_call(parsed),
        Some("canvas") => canvas::assert_call(call, parsed),
        Some("svg") => assert_svg_call(call, parsed),
        _ => Err("native endpoint kind missing or invalid".into()),
    }
}

fn assert_css_call(call: ParsedCall<'_>) -> Result<(), String> {
    let expected = SvgRasterizeOps::measure_html_text(
        call.text,
        call.family,
        call.size,
        call.weight,
        call.italic,
        0.0,
        None,
    )
    .map_err(|error| format!("CSS boundary reference failed: {error:?}"))?;
    assert_eq!(
        call.actual,
        rounded_width(expected),
        "CSS {:?}, {}, {}",
        call.text,
        call.family,
        call.weight
    );
    Ok(())
}

fn assert_svg_call(call: &Value, parsed: ParsedCall<'_>) -> Result<(), String> {
    let expected = SvgRasterizeOps::measure_svg_text_metrics(
        parsed.text,
        parsed.family,
        parsed.size,
        parsed.weight,
        parsed.italic,
    )
    .map_err(|error| format!("SVG boundary reference failed: {error:?}"))?;
    let metrics = call.get("metrics").ok_or("SVG metrics missing")?;
    assert_svg_metrics(metrics, &expected, &parsed)?;
    assert_eq!(
        raw_svg_number(call.get("width"))?,
        expected.advance,
        "SVG {:?}, {}, {}",
        parsed.text,
        parsed.family,
        parsed.weight
    );
    Ok(())
}

fn assert_svg_metrics(
    metrics: &Value,
    expected: &crate::markdown::svg_rasterize::SvgTextRunMetrics,
    parsed: &ParsedCall<'_>,
) -> Result<(), String> {
    for (name, expected_value) in [
        ("advance", expected.advance),
        ("ascent", expected.ascent),
        ("descent", expected.descent),
    ] {
        assert_eq!(
            raw_svg_number(metrics.get(name))?,
            expected_value,
            "SVG {name}, {:?}, {}, {}",
            parsed.text,
            parsed.family,
            parsed.weight
        );
    }
    Ok(())
}

fn raw_svg_number(value: Option<&Value>) -> Result<f32, String> {
    /* WHY: SVG APIのf32値で厳密比較し、JSONからのf64復元差を許容誤差に置き換えない。 */
    let decoded = value
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or("finite SVG number missing")?;
    let native = decoded as f32;
    native
        .is_finite()
        .then_some(native)
        .ok_or_else(|| "SVG number exceeds f32 range".into())
}

pub(super) fn assert_inherited_cjk(probe: &Value) -> Result<(), String> {
    let family = probe
        .get("family")
        .and_then(Value::as_str)
        .ok_or("family missing")?;
    let text = probe
        .get("text")
        .and_then(Value::as_str)
        .ok_or("text missing")?;
    let weight = probe
        .get("weight")
        .and_then(Value::as_u64)
        .ok_or("weight missing")? as u16;
    let width = raw_svg_number(probe.get("width"))?;
    assert_eq!((family, weight, text), ("Noto Sans", 700, "日本語"));
    assert_eq!(probe.get("kind").and_then(Value::as_str), Some("svg"));
    let expected = SvgRasterizeOps::measure_svg_text_metrics(
        text,
        family,
        INHERITED_SVG_FONT_SIZE_PX,
        weight,
        false,
    )
    .map_err(|error| format!("inherited SVG boundary failed: {error:?}"))?;
    assert_eq!(width, expected.advance);
    Ok(())
}

pub(super) fn call_text(call: &Value) -> &str {
    call.get("args")
        .and_then(Value::as_array)
        .and_then(|args| args.first())
        .and_then(Value::as_str)
        .map_or("", std::convert::identity)
}

pub(super) fn call_weight(call: &Value) -> Option<u64> {
    call.get("args")
        .and_then(Value::as_array)
        .and_then(|args| args.get(TEXT_METRIC_WEIGHT_ARGUMENT_INDEX))
        .and_then(Value::as_u64)
}

pub(super) fn rounded_width(width: f32) -> f64 {
    (f64::from(width) * WIDTH_ROUNDING_SCALE).ceil() / WIDTH_ROUNDING_SCALE
}

#[cfg(test)]
mod tests {
    use super::super::oracle_boundary_tests::actual_v8_calls;
    use super::{assert_bridge_widths_match_rust, assert_inherited_cjk};
    use serde_json::{Value, json};

    #[test]
    fn raw_v8_html_width_calls_reject_unmeasurable_text() -> Result<(), String> {
        let calls = actual_v8_calls("native-width-oracle-invalid-html-text.js")?;
        for (kind, expected) in [
            ("css", "CSS boundary reference failed:"),
            ("canvas", "Canvas boundary reference failed:"),
        ] {
            let mut corrupted = calls.clone();
            let row = corrupted
                .iter_mut()
                .find(|call| call["kind"] == kind)
                .ok_or_else(|| format!("raw V8 {kind} call missing"))?;
            row["args"][0] = json!("\0");
            row["args"][1] = json!("");
            let error = assert_bridge_widths_match_rust(&corrupted)
                .err()
                .ok_or_else(|| format!("unmeasurable {kind} text was accepted"))?;
            assert!(error.starts_with(expected), "{error}");
        }
        Ok(())
    }

    #[test]
    fn inherited_cjk_probe_rejects_missing_metadata_and_invalid_width() {
        let valid = json!({
            "family": "Noto Sans", "text": "日本語", "weight": 700,
            "kind": "svg", "width": 1.0
        });
        for (key, expected) in [
            ("family", "family missing"),
            ("text", "text missing"),
            ("weight", "weight missing"),
            ("width", "finite SVG number missing"),
        ] {
            let mut probe = valid.clone();
            probe[key] = Value::Null;
            assert_eq!(
                assert_inherited_cjk(&probe).err().as_deref(),
                Some(expected)
            );
        }
        let mut probe = valid;
        probe["width"] = json!(1e100);
        assert_eq!(
            assert_inherited_cjk(&probe).err().as_deref(),
            Some("SVG number exceeds f32 range")
        );
    }
}
