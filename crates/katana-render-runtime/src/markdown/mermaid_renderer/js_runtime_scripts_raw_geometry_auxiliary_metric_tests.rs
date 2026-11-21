use crate::markdown::svg_rasterize::SvgRasterizeOps;
use serde_json::{Value, json};

const METRIC_TEXT_INDEX: usize = 0;
const METRIC_FAMILY_INDEX: usize = 1;
const METRIC_SIZE_INDEX: usize = 2;
const METRIC_WEIGHT_INDEX: usize = 3;
const METRIC_ITALIC_INDEX: usize = 4;
const METRIC_CEIL_SCALE: f64 = 64.0;

pub(super) fn auxiliary_native_span_metrics(envelope: &Value) -> Result<Value, String> {
    let font = metric_font_tuple(full_member_metric_args(envelope)?)?;
    let (parts, whole) = measure_auxiliary_spans(&font)?;
    let parts_sum = sum_advances(&parts, "auxiliary part advance is missing")?;
    let parts_ceil64_sum = sum_ceil64_advances(&parts)?;
    Ok(json!({
        "font": font,
        "parts": parts,
        "whole": whole,
        "partsAdvanceSum": parts_sum,
        "partsCeil64": ceil64(parts_sum),
        "partsCeil64Sum": parts_ceil64_sum,
        "wholeCeil64": ceil64(
            whole["advance"].as_f64().ok_or("whole advance is missing")?
        ),
    }))
}

fn full_member_metric_args(envelope: &Value) -> Result<&[Value], String> {
    envelope["svgFontMetricObservations"]
        .as_array()
        .and_then(|rows| {
            rows.iter().rev().find_map(|row| {
                (row["args"].get(METRIC_TEXT_INDEX).and_then(Value::as_str)
                    == Some("-int sizeInFeet"))
                .then(|| row["args"].as_array())
                .flatten()
            })
        })
        .map(Vec::as_slice)
        .ok_or_else(|| "full member metric observation is missing".to_string())
}

fn measure_auxiliary_spans(font: &(&str, f32, u16, bool)) -> Result<(Vec<Value>, Value), String> {
    let parts = ["-int", " sizeInFeet"]
        .into_iter()
        .map(|text| measure_auxiliary_span(text, font))
        .collect::<Result<Vec<_>, _>>()?;
    let whole = measure_auxiliary_span("-int sizeInFeet", font)?;
    Ok((parts, whole))
}

fn metric_font_tuple(args: &[Value]) -> Result<(&str, f32, u16, bool), String> {
    Ok((
        args.get(METRIC_FAMILY_INDEX)
            .and_then(Value::as_str)
            .ok_or("font family is missing")?,
        args.get(METRIC_SIZE_INDEX)
            .and_then(Value::as_f64)
            .ok_or("font size is missing")? as f32,
        args.get(METRIC_WEIGHT_INDEX)
            .and_then(Value::as_u64)
            .ok_or("font weight is missing")? as u16,
        args.get(METRIC_ITALIC_INDEX)
            .and_then(Value::as_bool)
            .ok_or("font style is missing")?,
    ))
}

fn measure_auxiliary_span(text: &str, font: &(&str, f32, u16, bool)) -> Result<Value, String> {
    let metrics = SvgRasterizeOps::measure_svg_text_metrics(text, font.0, font.1, font.2, font.3)
        .map_err(|error| format!("auxiliary SVG metric probe failed: {error:?}"))?;
    Ok(json!({
        "text": text,
        "advance": metrics.advance,
        "ascent": metrics.ascent,
        "descent": metrics.descent,
    }))
}

fn sum_advances(rows: &[Value], error: &str) -> Result<f64, String> {
    rows.iter().try_fold(0.0, |sum, row| {
        row["advance"]
            .as_f64()
            .map(|advance| sum + advance)
            .ok_or_else(|| error.to_string())
    })
}

fn sum_ceil64_advances(rows: &[Value]) -> Result<f64, String> {
    rows.iter().try_fold(0.0, |sum, row| {
        row["advance"]
            .as_f64()
            .map(|advance| sum + ceil64(advance))
            .ok_or_else(|| "auxiliary part advance is missing".to_string())
    })
}

fn ceil64(value: f64) -> f64 {
    (value * METRIC_CEIL_SCALE).ceil() / METRIC_CEIL_SCALE
}
