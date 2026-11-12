use super::{decode_fixture, install_trace};
use crate::markdown::diagram_js_runtime::DiagramRuntimeScript;
use serde_json::{Value, json};

const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg"/>"#;
const OBSERVED_WIDTH: f64 = 12.5;
const OBSERVED_HEIGHT: u32 = 10;
const REQUIRED_ARRAYS: &[(&str, &str)] = &[
    ("calls", "actual native width calls are missing"),
    ("boxes", "actual getBBox observations are missing"),
    (
        "c4DescriptionRows",
        "C4 description observations are missing",
    ),
];

#[test]
fn native_width_targets_preserve_escaped_strings_and_order() -> Result<(), String> {
    let mut scripts = vec![DiagramRuntimeScript::borrowed(
        "render-mermaid.js",
        "render();",
    )];
    let targets = [("a\"b", "line\n\\雪"), ("second", "")];
    install_trace(&mut scripts, &targets)?;
    let target_script = scripts
        .iter()
        .find(|script| script.name == "native-width-targets.js")
        .ok_or("target script missing")?;
    let json = target_script
        .code
        .strip_prefix("globalThis.__krrTraceTargets = ")
        .and_then(|code| code.strip_suffix(';'))
        .ok_or("unexpected target script")?;
    let decoded: Vec<(String, String)> =
        serde_json::from_str(json).map_err(|error| error.to_string())?;
    assert_eq!(
        decoded,
        targets.map(|(selector, text)| (selector.to_owned(), text.to_owned()))
    );
    Ok(())
}

#[test]
fn native_width_trace_rejects_missing_render_entrypoint() {
    let mut scripts = vec![DiagramRuntimeScript::borrowed("other.js", "render();")];
    assert_eq!(
        install_trace(&mut scripts, &[]).err().as_deref(),
        Some("Mermaid render entrypoint missing")
    );
    assert_eq!(scripts.len(), 1);
}

fn recorded_fixture() -> Value {
    json!({
        "calls": [{"text": "A", "width": OBSERVED_WIDTH}],
        "boxes": [{"x": 0, "y": 0, "width": OBSERVED_WIDTH, "height": OBSERVED_HEIGHT}],
        "c4DescriptionRows": [],
        "svg": SVG,
        "probes": {"status": "complete"},
    })
}

#[test]
fn native_width_decoder_preserves_each_valid_observation() -> Result<(), String> {
    let source = recorded_fixture();
    let fixture = decode_fixture(&source.to_string())?;
    assert_eq!(fixture.calls, [source["calls"][0].clone()]);
    assert_eq!(fixture.boxes, [source["boxes"][0].clone()]);
    assert!(fixture.c4_description_rows.is_empty());
    assert_eq!(fixture.svg, SVG);
    assert_eq!(fixture.probes, source["probes"]);
    Ok(())
}

#[test]
fn native_width_decoder_rejects_missing_or_non_array_observations() -> Result<(), String> {
    for &(name, expected) in REQUIRED_ARRAYS {
        let mut value = recorded_fixture();
        let fields = value.as_object_mut().ok_or("fixture object missing")?;
        fields.remove(name).ok_or("fixture observation missing")?;
        assert_rejected(&value, expected);
        value[name] = json!({"unexpected": []});
        assert_rejected(&value, expected);
    }
    Ok(())
}

#[test]
fn native_width_decoder_rejects_invalid_json_and_missing_result_metadata() -> Result<(), String> {
    let error = decode_fixture("{").err().ok_or("truncated JSON accepted")?;
    assert!(error.starts_with("EOF while parsing"), "{error}");
    for (name, expected) in [
        ("probes", "probe metadata missing"),
        ("svg", "rendered SVG missing"),
    ] {
        let mut value = recorded_fixture();
        let fields = value.as_object_mut().ok_or("fixture object missing")?;
        fields.remove(name).ok_or("fixture metadata missing")?;
        assert_rejected(&value, expected);
    }
    let mut value = recorded_fixture();
    value["svg"] = json!(["not a serialized SVG"]);
    assert_rejected(&value, "rendered SVG missing");
    Ok(())
}

fn assert_rejected(value: &Value, expected: &str) {
    assert_eq!(
        decode_fixture(&value.to_string()).err().as_deref(),
        Some(expected)
    );
}
