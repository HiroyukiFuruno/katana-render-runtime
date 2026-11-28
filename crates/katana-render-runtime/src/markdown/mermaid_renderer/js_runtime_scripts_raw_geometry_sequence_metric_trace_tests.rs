use serde_json::Value;

const MAX_SVG_FONT_METRIC_CALLS_PER_FIXTURE: usize = 32;
const SVG_METRIC_VALUE_COUNT: usize = 8;

type SvgFontMetricArguments = (String, String, f64, u16, bool);

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SvgFontMetricObservation {
    ordinal: usize,
    args: SvgFontMetricArguments,
    metrics: SvgFontMetricResult,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SvgFontMetricResult {
    advance: f64,
    ascent: f64,
    descent: f64,
    #[serde(default, deserialize_with = "deserialize_finite_optional")]
    layout_ascent: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_finite_optional")]
    ink_left: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_finite_optional")]
    ink_top: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_finite_optional")]
    ink_right: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_finite_optional")]
    ink_bottom: Option<f64>,
}

pub(super) fn verify_svg_font_metric_observations(
    path: &str,
    expected_count: usize,
    envelope: &Value,
) -> Result<(), String> {
    let call_count = envelope["svgFontMetricCalls"]
        .as_u64()
        .ok_or("SVG font metric callback count missing")?;
    if call_count > MAX_SVG_FONT_METRIC_CALLS_PER_FIXTURE as u64 {
        return Err(format!("SVG metric trace exceeds diagnostic bound: {path}"));
    }
    if call_count != expected_count as u64 {
        return Err(format!("SVG metric callback count changed: {path}"));
    }
    let encoded = envelope
        .get("svgFontMetricObservations")
        .cloned()
        .ok_or("SVG metric observations missing")?;
    let observations: Vec<SvgFontMetricObservation> =
        serde_json::from_value(encoded).map_err(|error| error.to_string())?;
    if observations.len() != call_count as usize {
        return Err(format!("SVG metric callback count/trace mismatch: {path}"));
    }
    for (ordinal, observation) in observations.iter().enumerate() {
        verify_svg_font_metric_observation(path, ordinal, observation)?;
    }
    Ok(())
}

pub(super) fn verify_sequence_text_geometry(
    path: &str,
    expected_metric_count: usize,
    actor_labels: &[&str],
    envelope: &Value,
) -> Result<(), String> {
    verify_font_callbacks(path, envelope)?;
    verify_svg_font_metric_observations(path, expected_metric_count, envelope)?;
    verify_actor_text_box_observations(path, actor_labels, envelope)
}

fn verify_font_callbacks(path: &str, envelope: &Value) -> Result<(), String> {
    let callback_count = envelope["fontMeasureCalls"]
        .as_u64()
        .ok_or("font callback count missing")?
        .checked_add(
            envelope["svgFontMetricCalls"]
                .as_u64()
                .ok_or("SVG font callback count missing")?,
        )
        .ok_or("font callback count overflow")?;
    if callback_count == 0 {
        return Err(format!("real font bridge was not called: {path}"));
    }
    Ok(())
}

fn verify_actor_text_box_observations(
    path: &str,
    actor_labels: &[&str],
    envelope: &Value,
) -> Result<(), String> {
    let observations = envelope["svgTextBoxObservations"]
        .as_array()
        .ok_or("actor SVG text-box observations missing")?;
    if observations.len() != actor_labels.len() * 2 {
        return Err(format!(
            "actor SVG text-box observation count changed: {path}"
        ));
    }
    for (ordinal, row) in observations.iter().enumerate() {
        verify_actor_text_box_observation(path, ordinal, row, actor_labels)?;
    }
    for label in actor_labels {
        let count = observations
            .iter()
            .filter(|row| row["node"]["text"].as_str() == Some(label))
            .count();
        if count != 2 {
            return Err(format!(
                "actor label measurement count changed for {label}: {path}"
            ));
        }
    }
    Ok(())
}

fn verify_actor_text_box_observation(
    path: &str,
    ordinal: usize,
    row: &Value,
    actor_labels: &[&str],
) -> Result<(), String> {
    let text = row["node"]["text"]
        .as_str()
        .ok_or("actor SVG text-box text missing")?;
    if row["ordinal"].as_u64() != Some(ordinal as u64)
        || !actor_labels.contains(&text)
        || row["node"]["localName"] != "text"
        || row["node"]["parent"]["localName"] != "svg"
        || row["node"]["parent"]["parentTagName"].as_str().is_none()
    {
        return Err(format!("actor SVG text-box input binding changed: {path}"));
    }
    let bounds = ["x", "y", "width", "height"].map(|name| row["bbox"][name].as_f64());
    if bounds
        .into_iter()
        .any(|value| value.is_none_or(|number| !number.is_finite()))
    {
        return Err(format!("actor SVG text-box result is invalid: {path}"));
    }
    Ok(())
}

fn verify_svg_font_metric_observation(
    path: &str,
    ordinal: usize,
    observation: &SvgFontMetricObservation,
) -> Result<(), String> {
    if observation.ordinal != ordinal {
        return Err(format!("SVG metric callback ordinal mismatch: {path}"));
    }
    verify_svg_font_metric_arguments(path, &observation.args)?;
    verify_svg_font_metric_values(path, &observation.metrics)
}

fn verify_svg_font_metric_arguments(
    path: &str,
    arguments: &SvgFontMetricArguments,
) -> Result<(), String> {
    let (_, _, size, _, _) = arguments;
    if !size.is_finite() {
        return Err(format!(
            "SVG metric callback contains a non-finite size: {path}"
        ));
    }
    Ok(())
}

fn verify_svg_font_metric_values(path: &str, metrics: &SvgFontMetricResult) -> Result<(), String> {
    if !svg_metric_values(metrics).into_iter().all(f64::is_finite) {
        return Err(format!(
            "SVG metric callback contains non-finite values: {path}"
        ));
    }
    if metrics.advance < 0.0
        || metrics.ascent + metrics.descent < 0.0
        || metrics.layout_ascent.is_some_and(|value| value < 0.0)
    {
        return Err(format!(
            "SVG metric callback contains invalid dimensions: {path}"
        ));
    }
    verify_svg_metric_ink_bounds(path, metrics)
}

fn svg_metric_values(metrics: &SvgFontMetricResult) -> [f64; SVG_METRIC_VALUE_COUNT] {
    [
        metrics.advance,
        metrics.ascent,
        metrics.descent,
        metrics.layout_ascent.unwrap_or(0.0),
        metrics.ink_left.unwrap_or(0.0),
        metrics.ink_top.unwrap_or(0.0),
        metrics.ink_right.unwrap_or(0.0),
        metrics.ink_bottom.unwrap_or(0.0),
    ]
}

fn verify_svg_metric_ink_bounds(path: &str, metrics: &SvgFontMetricResult) -> Result<(), String> {
    let bounds = [
        metrics.ink_left,
        metrics.ink_top,
        metrics.ink_right,
        metrics.ink_bottom,
    ];
    let present = bounds.iter().filter(|value| value.is_some()).count();
    if present != 0 && present != bounds.len() {
        return Err(format!("SVG metric ink bounds are incomplete: {path}"));
    }
    if let [Some(left), Some(top), Some(right), Some(bottom)] = bounds
        && (left > right || top > bottom)
    {
        return Err(format!("SVG metric ink bounds are reversed: {path}"));
    }
    Ok(())
}

fn deserialize_finite_optional<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = <f64 as serde::Deserialize>::deserialize(deserializer)?;
    if value.is_finite() {
        Ok(Some(value))
    } else {
        Err(serde::de::Error::custom("metric value is not finite"))
    }
}

#[cfg(test)]
mod metric_trace_contract_tests {
    use super::{MAX_SVG_FONT_METRIC_CALLS_PER_FIXTURE, verify_svg_font_metric_observations};
    use serde_json::{Value, json};

    fn valid_envelope() -> Value {
        json!({
            "svgFontMetricCalls": 1,
            "svgFontMetricObservations": [{
                "ordinal": 0,
                "args": ["A", "Arial", 16.0, 400, false],
                "metrics": {
                    "advance": 8.0,
                    "ascent": 12.0,
                    "descent": 4.0,
                    "layoutAscent": 12.0,
                    "inkLeft": 0.0,
                    "inkTop": -12.0,
                    "inkRight": 8.0,
                    "inkBottom": 4.0
                }
            }]
        })
    }

    #[test]
    fn rejects_missing_and_mismatched_svg_metric_trace_fields() -> Result<(), String> {
        let mut missing_trace = valid_envelope();
        missing_trace
            .as_object_mut()
            .ok_or("fixture object missing")?
            .remove("svgFontMetricObservations")
            .ok_or("trace field missing")?;
        let mut missing_advance = valid_envelope();
        missing_advance["svgFontMetricObservations"][0]["metrics"]
            .as_object_mut()
            .ok_or("metrics object missing")?
            .remove("advance")
            .ok_or("advance field missing")?;
        let mut count_mismatch = valid_envelope();
        count_mismatch["svgFontMetricCalls"] = json!(2);

        assert!(verify_svg_font_metric_observations("missing", 1, &missing_trace).is_err());
        assert!(verify_svg_font_metric_observations("advance", 1, &missing_advance).is_err());
        assert!(verify_svg_font_metric_observations("count", 2, &count_mismatch).is_err());
        Ok(())
    }

    #[test]
    fn rejects_invalid_values_and_trace_budget_overflow() -> Result<(), String> {
        let mut invalid_optional = valid_envelope();
        invalid_optional["svgFontMetricObservations"][0]["metrics"]["layoutAscent"] = Value::Null;
        let mut invalid_required = valid_envelope();
        invalid_required["svgFontMetricObservations"][0]["metrics"]["advance"] = Value::Null;
        let mut invalid_bounds = valid_envelope();
        invalid_bounds["svgFontMetricObservations"][0]["metrics"]["inkLeft"] = json!(9.0);
        let over_budget = json!({
            "svgFontMetricCalls": MAX_SVG_FONT_METRIC_CALLS_PER_FIXTURE + 1,
            "svgFontMetricObservations": []
        });

        assert!(verify_svg_font_metric_observations("null", 1, &invalid_optional).is_err());
        assert!(verify_svg_font_metric_observations("required", 1, &invalid_required).is_err());
        assert!(verify_svg_font_metric_observations("bounds", 1, &invalid_bounds).is_err());
        assert!(
            verify_svg_font_metric_observations(
                "budget",
                MAX_SVG_FONT_METRIC_CALLS_PER_FIXTURE + 1,
                &over_budget
            )
            .is_err()
        );
        Ok(())
    }
}
