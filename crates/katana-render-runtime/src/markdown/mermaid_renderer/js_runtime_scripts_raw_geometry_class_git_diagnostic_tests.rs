use super::diagnostic_output::DiagnosticOutput;
use super::{
    envelope_svg, fixture_mermaid_source, raw_geometry_request, raw_geometry_vendor_config,
    render_vendor_raw_and_processed_mode, render_vendor_raw_and_processed_with_metric_labels,
};
use crate::markdown::runtime_assets::{MERMAID_JS_CHECKSUM, RuntimeAsset};
use serde_json::json;

#[path = "js_runtime_scripts_raw_geometry_auxiliary_metric_tests.rs"]
mod auxiliary_metric;
use auxiliary_metric::auxiliary_native_span_metrics;

const CLASS_PATH: &str = "tests/fixtures/mermaid/en/03-02-class-diagram-inheritance.md";
const CLASS_SOURCE: &str =
    include_str!("../../../../../tests/fixtures/mermaid/en/03-02-class-diagram-inheritance.md");
const GIT_PATH: &str = "tests/fixtures/mermaid/en/12-01-git-graph-simple.md";
const GIT_SOURCE: &str =
    include_str!("../../../../../tests/fixtures/mermaid/en/12-01-git-graph-simple.md");
const OUTPUT_ENV: &str = "KRR_CLASS_GIT_NORMALIZER_DIAGNOSTIC_OUTPUT";
const CLASS_METRIC_LABELS: [&str; 3] = ["Fish", "-int sizeInFeet", "-canEat()"];
const GIT_METRIC_LABELS: [&str; 3] = ["main", "base", "rust-js"];
#[path = "js_runtime_scripts_raw_geometry_layout_diagnostic_tests.rs"]
mod layout_diagnostic;
#[test]
fn captures_class_fixture_before_and_after_normalization() -> Result<(), String> {
    let output = DiagnosticOutput::new(OUTPUT_ENV)?;
    capture_fixture(CLASS_PATH, CLASS_SOURCE, &CLASS_METRIC_LABELS, &output)
}

#[test]
fn captures_git_fixture_before_and_after_normalization() -> Result<(), String> {
    let output = DiagnosticOutput::new(OUTPUT_ENV)?;
    capture_fixture(GIT_PATH, GIT_SOURCE, &GIT_METRIC_LABELS, &output)
}

fn capture_fixture(
    path: &str,
    markdown: &str,
    metric_labels: &[&str],
    output: &DiagnosticOutput,
) -> Result<(), String> {
    let source = fixture_mermaid_source(markdown)?;
    let request = raw_geometry_request(&source);
    let config = raw_geometry_vendor_config();
    let envelope =
        render_vendor_raw_and_processed_with_metric_labels(&request, &config, metric_labels)?;
    assert_render_binding(path, &source, &request, &config, &envelope)?;
    assert_selected_metrics(path, metric_labels, &envelope)?;
    let record = diagnostic_record(path, &source, request, config, envelope)?;
    write_diagnostic_record(output, record)
}

fn assert_selected_metrics(
    path: &str,
    metric_labels: &[&str],
    envelope: &serde_json::Value,
) -> Result<(), String> {
    if envelope["svgMetricObservationMode"] != "selected"
        || envelope["svgMetricLabels"] != json!(metric_labels)
    {
        return Err(format!("selected metric mode changed for {path}"));
    }
    let observations = envelope["svgFontMetricObservations"]
        .as_array()
        .ok_or("selected metric observations are not an array")?;
    selected_metric_counts_match(envelope, observations.len(), path)?;
    assert_selected_labels(metric_labels, observations)
}

fn selected_metric_counts_match(
    envelope: &serde_json::Value,
    unique_count: usize,
    path: &str,
) -> Result<(), String> {
    let selected_calls = envelope["selectedMetricCalls"]
        .as_u64()
        .ok_or("selected metric call count is missing")?;
    let duplicate_calls = envelope["selectedMetricDuplicateCalls"]
        .as_u64()
        .ok_or("selected metric duplicate count is missing")?;
    if selected_calls != unique_count as u64 + duplicate_calls {
        return Err(format!("selected metric counts changed for {path}"));
    }
    Ok(())
}

fn assert_selected_labels(
    metric_labels: &[&str],
    observations: &[serde_json::Value],
) -> Result<(), String> {
    for label in metric_labels {
        if !observations.iter().any(|observation| {
            observation["args"]
                .get(0)
                .and_then(serde_json::Value::as_str)
                == Some(*label)
        }) {
            return Err(format!("selected metric label was not observed: {label}"));
        }
    }
    Ok(())
}

fn assert_render_binding(
    path: &str,
    source: &str,
    request: &serde_json::Value,
    config: &serde_json::Value,
    envelope: &serde_json::Value,
) -> Result<(), String> {
    if envelope["renderCalls"].as_u64() != Some(1)
        || envelope["renderInput"].as_str() != Some(source)
        || envelope.get("initializeInput") != Some(config)
        || envelope["svgId"].as_str() != request["svgId"].as_str()
    {
        return Err(format!("render binding changed for {path}"));
    }
    Ok(())
}

fn diagnostic_record(
    path: &str,
    source: &str,
    request: serde_json::Value,
    config: serde_json::Value,
    envelope: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let mut record = diagnostic_base_record(path, source, &request, &config, &envelope)?;
    let vendor = RuntimeAsset::mermaid()
        .bytes()
        .map_err(|error| error.to_string())?;
    record
        .as_object_mut()
        .ok_or("diagnostic record is not an object")?
        .extend(
            diagnostic_metric_fields(&envelope, vendor)
                .as_object()
                .ok_or("diagnostic metric fields are not an object")?
                .clone(),
        );
    add_auxiliary_native_span_metrics(path, &envelope, &mut record)?;
    Ok(record)
}

fn add_auxiliary_native_span_metrics(
    path: &str,
    envelope: &serde_json::Value,
    record: &mut serde_json::Value,
) -> Result<(), String> {
    if path == CLASS_PATH {
        record
            .as_object_mut()
            .ok_or("diagnostic record is not an object")?
            .insert(
                "auxiliaryNativeSpanMetrics".to_string(),
                auxiliary_native_span_metrics(envelope)?,
            );
    }
    Ok(())
}

fn diagnostic_base_record(
    path: &str,
    source: &str,
    request: &serde_json::Value,
    config: &serde_json::Value,
    envelope: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let raw = envelope_svg(envelope, "raw")?;
    let processed = envelope_svg(envelope, "processed")?;
    Ok(json!({
        "schema": "krr-class-git-vendor-normalizer-diagnostic-v1",
        "fixturePath": path,
        "sourceSha256": super::sequence_diagnostic::sha256(source.as_bytes()),
        "vendorSha256": MERMAID_JS_CHECKSUM,
        "request": request,
        "config": config,
        "renderCalls": envelope["renderCalls"],
        "renderInput": envelope["renderInput"],
        "svgId": envelope["svgId"],
        "candidateNativeVendorBeforePostprocess": raw,
        "processedSvg": processed,
        "formalAcceptance": false,
    }))
}

fn diagnostic_metric_fields(envelope: &serde_json::Value, vendor: &[u8]) -> serde_json::Value {
    json!({
        "fontMeasureCalls": envelope["fontMeasureCalls"],
        "svgFontMetricCalls": envelope["svgFontMetricCalls"],
        "svgMetricLabels": envelope["svgMetricLabels"],
        "svgMetricObservationMode": envelope["svgMetricObservationMode"],
        "selectedMetricCalls": envelope["selectedMetricCalls"],
        "selectedMetricDuplicateCalls": envelope["selectedMetricDuplicateCalls"],
        "svgFontMetricObservations": envelope["svgFontMetricObservations"],
        "svgTextBoxObservations": envelope["svgTextBoxObservations"],
        "vendorBytesSha256": super::sequence_diagnostic::sha256(vendor),
    })
}

fn write_diagnostic_record(
    output: &DiagnosticOutput,
    record: serde_json::Value,
) -> Result<(), String> {
    output.write(&serde_json::to_vec_pretty(&record).map_err(|error| error.to_string())?)
}
