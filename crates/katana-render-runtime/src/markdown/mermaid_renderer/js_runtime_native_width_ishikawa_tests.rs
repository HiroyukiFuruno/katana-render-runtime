use super::{helpers, oracle::call_text};
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use helpers::render_fixture_with_font_audit;
use serde_json::Value;

const ISHIKAWA_13_01_3_FIXTURE_MARKDOWN: &str =
    include_str!("../../../../../tests/fixtures/mermaid/ja/13-01-3.md");
const ISHIKAWA_13_02_4_FIXTURE_MARKDOWN: &str =
    include_str!("../../../../../tests/fixtures/mermaid/ja/13-02-4.md");
const SVG_SIZE_ARG_INDEX: usize = 2;
const SVG_WEIGHT_ARG_INDEX: usize = 3;
const SVG_ITALIC_ARG_INDEX: usize = 4;
#[cfg(target_os = "macos")]
const ISHIKAWA_13_01_3_CHROME_ADVANCE: f32 = 56.0;
#[cfg(target_os = "macos")]
const ISHIKAWA_13_02_4_CHROME_ADVANCE: f32 = 84.0;

struct SvgMetricArgs<'a> {
    text: &'a str,
    family: &'a str,
    size: f32,
    weight: u16,
    italic: bool,
}

#[test]
fn svg_metrics_audit_observes_ishikawa_13_01_3() -> Result<(), String> {
    assert_ishikawa_svg_metrics_audit("13-01-3", ISHIKAWA_13_01_3_FIXTURE_MARKDOWN, "図表品質")
}

#[test]
fn svg_metrics_audit_observes_ishikawa_13_02_4() -> Result<(), String> {
    assert_ishikawa_svg_metrics_audit("13-02-4", ISHIKAWA_13_02_4_FIXTURE_MARKDOWN, "ぼやけた写真")
}

fn assert_ishikawa_svg_metrics_audit(
    fixture: &str,
    fixture_markdown: &str,
    label: &str,
) -> Result<(), String> {
    let source = fenced_mermaid_source(fixture_markdown, fixture)?;
    let (result, _) = render_fixture_with_font_audit(source)?;
    let svg_call = find_ishikawa_svg_call(&result, fixture, label)?;
    let args = svg_metric_args(svg_call, fixture, label)?;
    let (target_metrics, target_audit) =
        audit_ishikawa_svg_metrics(args.text, args.family, args.size, args.weight, args.italic)?;
    assert_bridge_metrics_match(svg_call, target_metrics, fixture, label)?;
    assert_chrome_observed_advance(target_metrics.advance, fixture, label)?;
    print_ishikawa_svg_audit(fixture, label, svg_call, &target_audit);
    assert_target_font_audit(fixture, &target_audit)?;
    Ok(())
}

fn fenced_mermaid_source<'a>(fixture_markdown: &'a str, fixture: &str) -> Result<&'a str, String> {
    fixture_markdown
        .split("~~~mermaid\n")
        .nth(1)
        .and_then(|value| value.split("\n~~~").next())
        .ok_or_else(|| format!("{fixture} fixture Mermaid source missing"))
}

fn find_ishikawa_svg_call<'a>(
    result: &'a helpers::MeasurementFixture,
    fixture: &str,
    label: &str,
) -> Result<&'a Value, String> {
    result
        .calls
        .iter()
        .find(|call| {
            call_text(call) == label && call.get("kind").and_then(Value::as_str) == Some("svg")
        })
        .ok_or_else(|| format!("{fixture} did not record SVG metrics for {label:?}"))
}

fn svg_metric_args<'a>(
    call: &'a Value,
    fixture: &str,
    label: &str,
) -> Result<SvgMetricArgs<'a>, String> {
    let args = svg_args_array(call, fixture, label)?;
    Ok(SvgMetricArgs {
        text: svg_string_arg(args, 0, fixture, label, "text")?,
        family: svg_string_arg(args, 1, fixture, label, "family")?,
        size: svg_size_arg(args, fixture, label)?,
        weight: svg_weight_arg(args, fixture, label)?,
        italic: svg_italic_arg(args, fixture, label)?,
    })
}

fn svg_args_array<'a>(call: &'a Value, fixture: &str, label: &str) -> Result<&'a [Value], String> {
    call.get("args")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("{fixture} SVG args are missing for {label:?}"))
}

fn svg_string_arg<'a>(
    args: &'a [Value],
    index: usize,
    fixture: &str,
    label: &str,
    name: &str,
) -> Result<&'a str, String> {
    args.get(index)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{fixture} SVG {name} arg is missing for {label:?}"))
}

fn svg_size_arg(args: &[Value], fixture: &str, label: &str) -> Result<f32, String> {
    args.get(SVG_SIZE_ARG_INDEX)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(|value| value as f32)
        .ok_or_else(|| format!("{fixture} SVG size arg is invalid for {label:?}"))
}

fn svg_weight_arg(args: &[Value], fixture: &str, label: &str) -> Result<u16, String> {
    args.get(SVG_WEIGHT_ARG_INDEX)
        .and_then(Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .ok_or_else(|| format!("{fixture} SVG weight arg is invalid for {label:?}"))
}

fn svg_italic_arg(args: &[Value], fixture: &str, label: &str) -> Result<bool, String> {
    args.get(SVG_ITALIC_ARG_INDEX)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("{fixture} SVG italic arg is missing for {label:?}"))
}

fn audit_ishikawa_svg_metrics(
    text: &str,
    family: &str,
    size: f32,
    weight: u16,
    italic: bool,
) -> Result<(crate::markdown::svg_rasterize::SvgTextRunMetrics, Value), String> {
    let (metrics, audit) = SvgRasterizeOps::with_actual_font_source_audit(|| {
        SvgRasterizeOps::measure_svg_text_metrics(text, family, size, weight, italic)
    })
    .map_err(|error| error.to_string())?;
    Ok((metrics.map_err(|error| error.to_string())?, audit))
}

fn assert_bridge_metrics_match(
    call: &Value,
    target: crate::markdown::svg_rasterize::SvgTextRunMetrics,
    fixture: &str,
    label: &str,
) -> Result<(), String> {
    let metrics = call
        .get("metrics")
        .ok_or_else(|| format!("{fixture} SVG metrics are missing for {label:?}"))?;
    for (name, expected) in [
        ("advance", target.advance),
        ("ascent", target.ascent),
        ("descent", target.descent),
    ] {
        let actual = metrics
            .get(name)
            .and_then(Value::as_f64)
            .map(|value| value as f32)
            .ok_or_else(|| format!("{fixture} SVG {name} is missing for {label:?}"))?;
        assert_eq!(
            actual, expected,
            "{fixture} SVG {name} differs for {label:?}"
        );
    }
    Ok(())
}

fn print_ishikawa_svg_audit(fixture: &str, label: &str, svg_call: &Value, target_audit: &Value) {
    println!(
        "ishikawa_svg_metrics_audit={}",
        serde_json::json!({
            "fixture": fixture,
            "label": label,
            "args": svg_call.get("args"),
            "metrics": svg_call.get("metrics"),
            "targetFontAudit": target_audit,
        })
    );
}

fn assert_target_font_audit(fixture: &str, target_audit: &Value) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let faces = target_audit["faces"].as_array().ok_or_else(|| {
            format!("{fixture} SVG metrics did not record audited faces: {target_audit}")
        })?;
        if !faces.iter().any(|face| {
            face["post_script_name"] == "HiraginoSans-W5"
                && face["index"].as_u64() == Some(0)
                && face["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("W5.ttc"))
        }) {
            return Err(format!(
                "{fixture} SVG metrics did not select physical HiraginoSans-W5: {target_audit}"
            ));
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (fixture, target_audit);
    Ok(())
}

#[cfg(target_os = "macos")]
fn assert_chrome_observed_advance(advance: f32, fixture: &str, label: &str) -> Result<(), String> {
    /* WHY: 56/84は独立取得したChrome W5 raw観測値であり、KRRの補正値ではない。 */
    let expected = match label {
        "図表品質" => ISHIKAWA_13_01_3_CHROME_ADVANCE,
        "ぼやけた写真" => ISHIKAWA_13_02_4_CHROME_ADVANCE,
        _ => {
            return Err(format!(
                "{fixture} has no Chrome advance observation for {label:?}"
            ));
        }
    };
    assert_eq!(
        advance, expected,
        "{fixture} Chrome-observed advance differs for {label:?}"
    );
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn assert_chrome_observed_advance(
    _advance: f32,
    _fixture: &str,
    _label: &str,
) -> Result<(), String> {
    Ok(())
}
