use super::{
    DiagnosticOutput, GIT_METRIC_LABELS, GIT_PATH, GIT_SOURCE, assert_render_binding,
    diagnostic_record, fixture_mermaid_source, raw_geometry_request, raw_geometry_vendor_config,
    render_vendor_raw_and_processed_mode, render_vendor_raw_and_processed_with_metric_labels,
    write_diagnostic_record,
};
use serde_json::json;

#[path = "js_runtime_scripts_raw_geometry_c4_observation_tests.rs"]
mod c4_observation_tests;
use c4_observation_tests::assert_c4_observations;

fn assert_layout_render_identity(
    path: &str,
    request: &serde_json::Value,
    config: &serde_json::Value,
    envelope: &serde_json::Value,
) -> Result<(), String> {
    if envelope["renderCalls"].as_u64() != Some(1)
        || envelope.get("initializeInput") != Some(config)
        || envelope["svgId"].as_str() != request["svgId"].as_str()
    {
        return Err(format!("layout render identity changed for {path}"));
    }
    Ok(())
}

fn add_layout_binding_fields(
    record: &mut serde_json::Value,
    role: &str,
    request: &serde_json::Value,
    render_input: &serde_json::Value,
) {
    if let Some(object) = record.as_object_mut() {
        object.insert("role".to_string(), json!(role));
        object.insert("requestSource".to_string(), request["source"].clone());
        object.insert("renderInputObserved".to_string(), render_input.clone());
        object.insert(
            "sourceBindingMatches".to_string(),
            json!(request.get("source") == Some(render_input)),
        );
    }
}

const LAYOUT_OUTPUT_ENV: &str = "KRR_LAYOUT_NORMALIZER_DIAGNOSTIC_OUTPUT";
const C4_OUTPUT_ENV: &str = "KRR_C4_GEOMETRY_DIAGNOSTIC_OUTPUT";
const KANBAN_PATH: &str = "tests/fixtures/mermaid/en/14-01-kanban-simple.md";
const KANBAN_SOURCE: &str =
    include_str!("../../../../../tests/fixtures/mermaid/en/14-01-kanban-simple.md");
const BLOCK_PATH: &str = "tests/fixtures/mermaid/en/10-02-block-diagram-vertical.md";
const BLOCK_SOURCE: &str =
    include_str!("../../../../../tests/fixtures/mermaid/en/10-02-block-diagram-vertical.md");
const ISHIKAWA_PATH: &str = "tests/fixtures/mermaid/ja/13-02-4.md";
const ISHIKAWA_SOURCE: &str = include_str!("../../../../../tests/fixtures/mermaid/ja/13-02-4.md");
const C4_PATH: &str = "tests/fixtures/mermaid/en/08-01-c4-context-simple.md";
const C4_SOURCE: &str =
    include_str!("../../../../../tests/fixtures/mermaid/en/08-01-c4-context-simple.md");
const C4_METRIC_LABELS: &[&str] = &[
    "KatanA renderer context",
    "User",
    "KatanA",
    "Markdown files",
    "Edits",
    "Reads and writes",
];

#[test]
fn captures_layout_fixtures_before_and_after_normalization() -> Result<(), String> {
    let output = DiagnosticOutput::new(LAYOUT_OUTPUT_ENV)?;
    let block_words = ["A", " wide", " one", " in", " the", " middle"];
    let block_text = block_words.concat();
    let block_labels = [&[block_text.as_str()][..], &block_words[..]].concat();
    let records = layout_fixture_specs(block_labels.as_slice())
        .into_iter()
        .map(|fixture| {
            let path = fixture.0;
            capture_layout_fixture(fixture).map_err(|error| format!("{error} for {path}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    write_diagnostic_record(&output, json!(records))
}

#[test]
fn captures_c4_layout_measurements_before_serialization() -> Result<(), String> {
    let output = DiagnosticOutput::new(C4_OUTPUT_ENV)?;
    let source = fixture_mermaid_source(C4_SOURCE)?;
    let request = raw_geometry_request(&source);
    let config = raw_geometry_vendor_config();
    let (envelope, font_source_audit) = render_c4_with_font_source_audit(&request, &config)?;
    assert_layout_render_identity(C4_PATH, &request, &config, &envelope)?;
    write_c4_diagnostic_record(
        &source,
        request,
        config,
        &envelope,
        font_source_audit,
        &output,
    )?;
    assert_c4_observations(&envelope)
}

#[cfg(target_os = "macos")]
fn render_c4_with_font_source_audit(
    request: &serde_json::Value,
    config: &serde_json::Value,
) -> Result<(serde_json::Value, serde_json::Value), String> {
    let (envelope, mut audit) =
        crate::markdown::svg_rasterize::SvgRasterizeOps::with_actual_font_source_audit(|| {
            render_vendor_raw_and_processed_with_metric_labels(request, config, C4_METRIC_LABELS)
        })
        .map_err(|error| error.to_string())?;
    audit
        .as_object_mut()
        .ok_or("C4 font source audit is not an object")?
        .insert("available".to_string(), json!(true));
    Ok((envelope?, audit))
}

#[cfg(not(target_os = "macos"))]
fn render_c4_with_font_source_audit(
    request: &serde_json::Value,
    config: &serde_json::Value,
) -> Result<(serde_json::Value, serde_json::Value), String> {
    let envelope =
        render_vendor_raw_and_processed_with_metric_labels(request, config, C4_METRIC_LABELS)?;
    Ok((
        envelope,
        json!({
            "available": false,
            "faces": [],
            "route": "non-macos-no-audit"
        }),
    ))
}

fn write_c4_diagnostic_record(
    source: &str,
    request: serde_json::Value,
    config: serde_json::Value,
    envelope: &serde_json::Value,
    font_source_audit: serde_json::Value,
    output: &DiagnosticOutput,
) -> Result<(), String> {
    let mut record = diagnostic_record(C4_PATH, source, request, config, envelope.clone())?;
    let fields = record
        .as_object_mut()
        .ok_or("C4 diagnostic record is not an object")?;
    fields.insert(
        "c4GeometryObservations".to_string(),
        envelope["c4GeometryObservations"].clone(),
    );
    fields.insert("c4Observer".to_string(), c4_observer_fields(envelope));
    fields.insert("actualFontSourceAudit".to_string(), font_source_audit);
    write_diagnostic_record(output, record)
}

fn c4_observer_fields(envelope: &serde_json::Value) -> serde_json::Value {
    let value = |name: &str| {
        envelope
            .get(name)
            .cloned()
            .unwrap_or(serde_json::Value::Null)
    };
    json!({
        "enabled": value("c4GeometryObserverEnabled"),
        "hookCalls": value("c4GeometryHookCalls"),
        "hookNodes": value("c4GeometryHookNodes"),
        "prototypeAfterRender": value("c4GeometryPrototypeAfterRender"),
    })
}

#[test]
fn c4_observer_fields_preserve_false_zero_empty_and_missing_values() {
    let envelope = json!({
        "c4GeometryObserverEnabled": false,
        "c4GeometryHookCalls": {"getBBox": 0, "getBoundingClientRect": 2},
        "c4GeometryHookNodes": [],
        "c4GeometryPrototypeAfterRender": {"getBBox": false},
    });
    let fields = c4_observer_fields(&envelope);
    assert_eq!(fields["enabled"], json!(false));
    assert_eq!(fields["hookCalls"]["getBBox"], json!(0));
    assert_eq!(fields["hookNodes"], json!([]));
    assert_eq!(fields["prototypeAfterRender"]["getBBox"], json!(false));
    assert_eq!(c4_observer_fields(&json!({}))["enabled"], json!(null));
    assert!(
        fields["prototypeAfterRender"]
            .get("getBoundingClientRect")
            .is_none()
    );
}

fn layout_fixture_specs<'a>(block_labels: &'a [&'a str]) -> Vec<(&'a str, &'a str, &'a [&'a str])> {
    let mut fixtures = vec![
        (KANBAN_PATH, KANBAN_SOURCE, &["Todo"][..]),
        (BLOCK_PATH, BLOCK_SOURCE, block_labels),
        (ISHIKAWA_PATH, ISHIKAWA_SOURCE, &["ぼやけた写真"][..]),
        (GIT_PATH, GIT_SOURCE, &GIT_METRIC_LABELS[..]),
    ];
    fixtures.extend_from_slice(LAYOUT_EXTRA_FIXTURES);
    fixtures
}

const LAYOUT_EXTRA_FIXTURES: &[(&str, &str, &[&str])] = &[
    (
        "tests/fixtures/mermaid/en/05-01-er-diagram-simple.md",
        include_str!("../../../../../tests/fixtures/mermaid/en/05-01-er-diagram-simple.md"),
        &["ER", "DOCUMENT"],
    ),
    (
        "tests/fixtures/mermaid/en/21-01-timeline-phases.md",
        include_str!("../../../../../tests/fixtures/mermaid/en/21-01-timeline-phases.md"),
        &["Spike"],
    ),
    (
        "tests/fixtures/mermaid/en/23-01-treemap-flat.md",
        include_str!("../../../../../tests/fixtures/mermaid/en/23-01-treemap-flat.md"),
        &["Mermaid"],
    ),
    (
        "tests/fixtures/mermaid/en/24-01-user-journey-diagram-preview.md",
        include_str!(
            "../../../../../tests/fixtures/mermaid/en/24-01-user-journey-diagram-preview.md"
        ),
        &["Edit"],
    ),
];

fn capture_layout_fixture(
    (path, markdown, metric_labels): (&str, &str, &[&str]),
) -> Result<serde_json::Value, String> {
    if metric_labels.is_empty() {
        return Err(format!("layout metric labels must be non-empty for {path}"));
    }
    let source = fixture_mermaid_source(markdown)?;
    let request = raw_geometry_request(&source);
    let config = raw_geometry_vendor_config();
    let production = capture_production(path, &source, &request, &config, metric_labels)?;
    let direct = capture_direct(path, &source, &request, &config, metric_labels)?;
    Ok(json!({"production": production, "direct": direct}))
}

fn capture_production(
    path: &str,
    source: &str,
    request: &serde_json::Value,
    config: &serde_json::Value,
    metric_labels: &[&str],
) -> Result<serde_json::Value, String> {
    let envelope =
        render_vendor_raw_and_processed_with_metric_labels(request, config, metric_labels)?;
    assert_layout_render_identity(path, request, config, &envelope)?;
    let render_input = envelope["renderInput"].clone();
    let mut record = diagnostic_record(path, source, request.clone(), config.clone(), envelope)?;
    add_layout_binding_fields(
        &mut record,
        "production-postprocess",
        request,
        &render_input,
    );
    Ok(record)
}

fn capture_direct(
    path: &str,
    source: &str,
    request: &serde_json::Value,
    config: &serde_json::Value,
    metric_labels: &[&str],
) -> Result<serde_json::Value, String> {
    let envelope =
        render_vendor_raw_and_processed_mode(request, config, false, &[], metric_labels)?;
    assert_render_binding(path, source, request, config, &envelope)?;
    let render_input = envelope["renderInput"].clone();
    let mut record = diagnostic_record(path, source, request.clone(), config.clone(), envelope)?;
    add_layout_binding_fields(&mut record, "vendor-direct", request, &render_input);
    Ok(record)
}
