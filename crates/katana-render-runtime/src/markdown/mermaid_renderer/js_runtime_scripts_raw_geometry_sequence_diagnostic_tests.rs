use super::{
    envelope_svg, fixture_mermaid_source, render_vendor_raw_and_processed_through_runtime,
};
use crate::markdown::{
    color_preset::DiagramColorPreset,
    runtime_assets::{MERMAID_JS_CHECKSUM, MERMAID_JS_VERSION},
};
use resvg::usvg::roxmltree::Document;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::hash_map::DefaultHasher,
    fs::OpenOptions,
    hash::{Hash, Hasher},
    io::Write,
    path::{Path, PathBuf},
};

#[path = "js_runtime_scripts_raw_geometry_sequence_metric_trace_tests.rs"]
mod metric_trace;
use metric_trace::verify_sequence_text_geometry;

const OUTPUT_ENV: &str = "KRR_SEQUENCE_NORMALIZER_DIAGNOSTIC_OUTPUT";
const SIMPLE_FIXTURE_PATH: &str =
    "tests/fixtures/mermaid/representative/04-01-sequence-diagram-simple.md";
const SIMPLE_FIXTURE_MARKDOWN: &str = include_str!(
    "../../../../../tests/fixtures/mermaid/representative/04-01-sequence-diagram-simple.md"
);
const ACTIVATION_FIXTURE_PATH: &str =
    "tests/fixtures/mermaid/en/04-02-sequence-diagram-activate-deactivate.md";
const ACTIVATION_FIXTURE_MARKDOWN: &str = include_str!(
    "../../../../../tests/fixtures/mermaid/en/04-02-sequence-diagram-activate-deactivate.md"
);
const SIMPLE_MESSAGE_COUNT: usize = 2;
const ACTIVATION_MESSAGE_COUNT: usize = 4;
const SIMPLE_SVG_FONT_METRIC_CALLS: usize = 10;
const ACTIVATION_SVG_FONT_METRIC_CALLS: usize = 16;
const SIMPLE_ACTOR_LABELS: [&str; 2] = ["User", "KatanA"];
const ACTIVATION_ACTOR_LABELS: [&str; 2] = ["Alice", "John"];

struct SequenceFixtureExpectation {
    path: &'static str,
    markdown: &'static str,
    message_count: usize,
    metric_calls: usize,
    actor_labels: &'static [&'static str],
}

#[test]
#[ignore = "diagnostic only; requires a fresh absolute TMP output path"]
fn captures_real_sequence_vendor_svg_before_and_after_normalization() -> Result<(), String> {
    let record = diagnostic_record(capture_fixture_rows()?)?;
    write_fresh_output(
        &diagnostic_output_path()?,
        &serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?,
    )
}

fn capture_fixture_rows() -> Result<Vec<Value>, String> {
    let fixtures = [
        SequenceFixtureExpectation {
            path: SIMPLE_FIXTURE_PATH,
            markdown: SIMPLE_FIXTURE_MARKDOWN,
            message_count: SIMPLE_MESSAGE_COUNT,
            metric_calls: SIMPLE_SVG_FONT_METRIC_CALLS,
            actor_labels: &SIMPLE_ACTOR_LABELS,
        },
        SequenceFixtureExpectation {
            path: ACTIVATION_FIXTURE_PATH,
            markdown: ACTIVATION_FIXTURE_MARKDOWN,
            message_count: ACTIVATION_MESSAGE_COUNT,
            metric_calls: ACTIVATION_SVG_FONT_METRIC_CALLS,
            actor_labels: &ACTIVATION_ACTOR_LABELS,
        },
    ];
    fixtures.into_iter().map(capture_fixture).collect()
}

fn diagnostic_record(rows: Vec<Value>) -> Result<Value, String> {
    Ok(json!({
        "schema": "krr-mermaid-sequence-vendor-normalization-diagnostic-v1",
        "formalAcceptance": false,
        "vendor": {"version": MERMAID_JS_VERSION, "sha256": MERMAID_JS_CHECKSUM},
        "captureMethod": "real vendor return captured before the unchanged production normalizer in one V8 execution",
        "rows": rows,
    }))
}

fn capture_fixture(fixture: SequenceFixtureExpectation) -> Result<Value, String> {
    let source = fixture_mermaid_source(fixture.markdown)?;
    let preset = DiagramColorPreset::dark();
    let request = sequence_request(&source, preset);
    let expected_config = sequence_vendor_config(&request);
    let envelope = render_vendor_raw_and_processed_through_runtime(
        &request,
        &expected_config,
        fixture.actor_labels,
    )?;
    verify_capture(&fixture, &source, &request, &expected_config, &envelope)?;
    capture_record(fixture.path, source, request, envelope)
}

fn verify_capture(
    fixture: &SequenceFixtureExpectation,
    source: &str,
    request: &Value,
    expected_config: &Value,
    envelope: &Value,
) -> Result<(), String> {
    verify_render_context(fixture.path, source, request, expected_config, envelope)?;
    verify_sequence_svgs(fixture.path, fixture.message_count, envelope)?;
    verify_sequence_text_geometry(
        fixture.path,
        fixture.metric_calls,
        fixture.actor_labels,
        envelope,
    )
}

fn verify_render_context(
    path: &str,
    source: &str,
    request: &Value,
    expected_config: &Value,
    envelope: &Value,
) -> Result<(), String> {
    assert_eq!(envelope["renderCalls"], 1, "{path}");
    assert_eq!(
        envelope["renderInput"], source,
        "English fixture source preparation must be identity"
    );
    assert_eq!(&envelope["request"], request, "{path}");
    assert_eq!(envelope["svgId"], envelope["request"]["svgId"], "{path}");
    assert_eq!(envelope["diagramType"], "sequencediagram", "{path}");
    assert_eq!(&envelope["expectedConfig"], expected_config, "{path}");
    assert_eq!(&envelope["initializeInput"], expected_config, "{path}");
    Ok(())
}

fn verify_sequence_svgs(path: &str, message_count: usize, envelope: &Value) -> Result<(), String> {
    let raw = envelope_svg(envelope, "raw")?;
    let processed = envelope_svg(envelope, "processed")?;
    verify_svg_root(path, "raw", raw)?;
    verify_svg_root(path, "processed", processed)?;
    assert!(
        raw.contains(r#"aria-roledescription="sequence""#),
        "raw vendor sequence missing: {path}"
    );
    assert_eq!(
        raw.matches("class=\"messageText\"").count(),
        message_count,
        "raw message inventory: {path}"
    );
    Ok(())
}

fn verify_svg_root(path: &str, label: &str, svg: &str) -> Result<(), String> {
    let document =
        Document::parse(svg).map_err(|error| format!("{label} SVG parse {path}: {error}"))?;
    if document.root_element().tag_name().name() != "svg" {
        return Err(format!("{label} document root is not SVG: {path}"));
    }
    Ok(())
}

fn capture_record(
    path: &str,
    source: String,
    request: Value,
    envelope: Value,
) -> Result<Value, String> {
    let raw = envelope_svg(&envelope, "raw")?;
    let processed = envelope_svg(&envelope, "processed")?;
    let source_sha256 = sha256(source.as_bytes());
    Ok(json!({
        "fixturePath": path,
        "source": source,
        "sourceSha256": source_sha256,
        "request": request,
        "initializeInput": envelope["initializeInput"],
        "renderInput": envelope["renderInput"],
        "svgId": envelope["svgId"],
        "renderCalls": envelope["renderCalls"],
        "fontMeasureCalls": envelope["fontMeasureCalls"],
        "svgFontMetricCalls": envelope["svgFontMetricCalls"],
        "svgFontMetricObservations": envelope["svgFontMetricObservations"],
        "svgTextBoxObservations": envelope["svgTextBoxObservations"],
        "rawVendorSvg": raw,
        "normalizedSvg": processed,
        "formalAcceptance": false,
    }))
}

fn sequence_request(source: &str, preset: &DiagramColorPreset) -> Value {
    let mut hasher = DefaultHasher::new();
    "mermaid-svg-id-v1".hash(&mut hasher);
    super::super::super::hash_mermaid_source_and_preset(&mut hasher, source, preset);
    json!({
        "source": source,
        "svgId": format!("katana-mermaid-svg-{:016x}", hasher.finish()),
        "theme": preset.mermaid_theme.as_ref(),
        "background": preset.background.as_ref(),
        "fill": preset.fill.as_ref(),
        "text": preset.text.as_ref(),
        "stroke": preset.stroke.as_ref(),
        "arrow": preset.arrow.as_ref(),
        "diagramType": "",
    })
}

fn sequence_vendor_config(request: &Value) -> Value {
    let background = &request["background"];
    let fill = &request["fill"];
    let text = &request["text"];
    let stroke = &request["stroke"];
    let arrow = &request["arrow"];
    json!({
        "startOnLoad": false,
        "securityLevel": "loose",
        "htmlLabels": false,
        "theme": request["theme"],
        "flowchart": {"htmlLabels": false, "useMaxWidth": false},
        "sequence": {"useMaxWidth": false},
        "themeVariables": {
            "background": background, "mainBkg": fill, "primaryColor": fill,
            "primaryTextColor": text, "primaryBorderColor": stroke, "secondaryColor": fill,
            "secondaryTextColor": text, "secondaryBorderColor": stroke, "tertiaryColor": fill,
            "tertiaryTextColor": text, "tertiaryBorderColor": stroke, "nodeTextColor": text,
            "lineColor": arrow, "textColor": text, "edgeLabelBackground": fill,
            "actorBkg": fill, "actorTextColor": text, "actorBorder": stroke,
            "signalColor": arrow, "signalTextColor": text, "labelTextColor": text,
            "noteBkgColor": fill, "noteTextColor": text, "noteBorderColor": stroke,
            "clusterBkg": background, "clusterBorder": stroke, "titleColor": text,
        },
    })
}

fn diagnostic_output_path() -> Result<PathBuf, String> {
    let output = std::env::var_os(OUTPUT_ENV).ok_or("diagnostic output path is required")?;
    let path = PathBuf::from(output);
    if !path.is_absolute() {
        return Err("diagnostic output path must be absolute".into());
    }
    Ok(path)
}

pub(super) fn write_fresh_output(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or("diagnostic output parent is missing")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest
        .parent()
        .and_then(Path::parent)
        .ok_or("workspace root missing")?;
    let tmp = workspace
        .join("tmp")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !parent.starts_with(&tmp) {
        return Err("diagnostic output must be under workspace tmp".into());
    }
    let file_path = parent.join(
        path.file_name()
            .ok_or("diagnostic output filename is missing")?,
    );
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(file_path)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes).map_err(|e| e.to_string())
}

pub(super) fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
