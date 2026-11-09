use super::super::test_support::{
    PROBE_TEST, TempDirectory, physical_font_fixture, read_response, run_probe_child,
};
use super::super::{
    CaptureRequest, CaptureResult, ProbeRequest, ProbeResponse, REQUEST_SCHEMA, RESPONSE_SCHEMA,
    ResourceFile, identity::physical_file, probe::probe,
};
use super::{request_sha256, verified_single_capture};
use std::fs;

const CONDITION_ID: &str = "identity-contract-case";
const INVALID_CAPTURE_RESPONSE_CASE_COUNT: usize = 5;
const RESPONSE_BINDING_ERROR: &str = "font probe response schema or request binding differs";
const CAPTURE_COUNT_ERROR: &str = "font probe response must contain one capture";
const CONDITION_ERROR: &str = "font probe response condition differs";

fn request() -> ProbeRequest {
    ProbeRequest {
        schema: REQUEST_SCHEMA.to_string(),
        captures: vec![CaptureRequest {
            condition_id: CONDITION_ID.to_string(),
            selected_fonts: Vec::new(),
            resource_files: Vec::new(),
        }],
    }
}

#[test]
fn font_probe_rejects_invalid_request_shape_and_duplicate_selected_font() -> Result<(), String> {
    let mut wrong_schema = request();
    wrong_schema.schema = "unknown-schema".into();
    let mut empty = request();
    empty.captures.clear();
    for invalid in [wrong_schema, empty] {
        assert_eq!(
            probe_serialized(&invalid).err().as_deref(),
            Some("unsupported request schema or empty capture list")
        );
    }
    let (_temp, _, selected) = physical_font_fixture()?;
    let mut duplicate = request();
    duplicate.captures[0].selected_fonts = vec![selected.clone(), selected];
    assert_eq!(
        probe_serialized(&duplicate).err().as_deref(),
        Some("duplicate CDP selected-font entry")
    );
    Ok(())
}

#[test]
fn font_probe_keeps_duplicate_resources_and_incomplete_identity_unknown() -> Result<(), String> {
    let (temp, _, mut selected) = physical_font_fixture()?;
    let resource = captured_font_resource(&temp)?;
    let mut request = request();
    selected.is_custom_font = true;
    request.captures[0].selected_fonts = vec![selected.clone()];
    request.captures[0].resource_files = vec![
        resource.clone(),
        ResourceFile {
            url: "https://example.test/same-font".into(),
            ..resource
        },
    ];
    selected.glyph_count = 0;
    request.captures.push(CaptureRequest {
        condition_id: "incomplete-font".into(),
        selected_fonts: vec![selected],
        resource_files: Vec::new(),
    });
    let observed = probe_serialized(&request)?;
    assert_unknown_captures(observed)
}

#[test]
fn captured_font_bytes_do_not_resolve_a_different_postscript_name() -> Result<(), String> {
    let (temp, _, mut selected) = physical_font_fixture()?;
    let resource = captured_font_resource(&temp)?;
    selected.is_custom_font = true;
    let mut request = request();
    request.captures[0].selected_fonts = vec![selected.clone()];
    request.captures[0].resource_files = vec![resource.clone()];
    selected.post_script_name = "KRR-Uncaptured-PostScript-Face".into();
    request.captures.push(CaptureRequest {
        condition_id: "uncaptured-font".into(),
        selected_fonts: vec![selected],
        resource_files: vec![resource],
    });
    let observed = probe_serialized(&request)?;
    let [verified, unknown] = observed.captures.as_slice() else {
        return Err("expected two font captures".into());
    };
    assert_eq!(verified.status, "resolved");
    assert_eq!(verified.font_provenance.len(), 1);
    assert_eq!(unknown.status, "unknown");
    assert!(unknown.font_provenance.is_empty());
    assert!(unknown.unknown_reasons.iter().any(|reason| {
        reason.contains("no exact physical face matched the CDP PostScript name")
    }));
    Ok(())
}

fn captured_font_resource(temp: &TempDirectory) -> Result<ResourceFile, String> {
    let body_path = temp.path().join("physical-font.ttf");
    let (_, bytes_sha256, _) = physical_file(&body_path)?;
    Ok(ResourceFile {
        url: "https://example.test/font".into(),
        body_path,
        bytes_sha256,
    })
}

fn probe_serialized(request: &ProbeRequest) -> Result<ProbeResponse, String> {
    let bytes = serde_json::to_vec(request).map_err(|error| error.to_string())?;
    probe(&bytes)
}

fn assert_unknown_captures(observed: ProbeResponse) -> Result<(), String> {
    let [duplicate, incomplete] = observed.captures.as_slice() else {
        return Err("expected two font captures".into());
    };
    for (capture, expected) in [
        (duplicate, "duplicate physical font resource path"),
        (incomplete, "CDP selected-font identity is incomplete"),
    ] {
        assert_eq!(capture.status, "unknown");
        assert!(capture.font_provenance.is_empty());
        assert!(
            capture
                .unknown_reasons
                .iter()
                .any(|reason| reason.contains(expected))
        );
    }
    Ok(())
}

fn response(schema: &str, binding: &str, conditions: &[&str]) -> ProbeResponse {
    ProbeResponse {
        schema: schema.to_string(),
        request_sha256: binding.to_string(),
        inventory_method: "typed contract fixture".to_string(),
        inventory_limit: "no browser or host inventory".to_string(),
        captures: conditions
            .iter()
            .map(|condition_id| CaptureResult {
                condition_id: (*condition_id).to_string(),
                status: "ok".to_string(),
                font_provenance: Vec::new(),
                unknown_reasons: Vec::new(),
            })
            .collect(),
    }
}

fn invalid_responses(
    binding: &str,
) -> [(ProbeResponse, &'static str); INVALID_CAPTURE_RESPONSE_CASE_COUNT] {
    [
        (
            response("wrong-schema", binding, &[CONDITION_ID]),
            RESPONSE_BINDING_ERROR,
        ),
        (
            response(RESPONSE_SCHEMA, "wrong-binding", &[CONDITION_ID]),
            RESPONSE_BINDING_ERROR,
        ),
        (response(RESPONSE_SCHEMA, binding, &[]), CAPTURE_COUNT_ERROR),
        (
            response(RESPONSE_SCHEMA, binding, &[CONDITION_ID, CONDITION_ID]),
            CAPTURE_COUNT_ERROR,
        ),
        (
            response(RESPONSE_SCHEMA, binding, &["different-condition"]),
            CONDITION_ERROR,
        ),
    ]
}

#[test]
fn verified_single_capture_enforces_schema_binding_count_and_condition() -> Result<(), String> {
    let request = request();
    let binding = request_sha256(&request)?;
    for (response, expected_error) in invalid_responses(&binding) {
        assert_eq!(
            verified_single_capture(&request, response, CONDITION_ID)
                .err()
                .as_deref(),
            Some(expected_error)
        );
    }

    let valid = response(RESPONSE_SCHEMA, &binding, &[CONDITION_ID]);
    let capture = verified_single_capture(&request, valid, CONDITION_ID)
        .map_err(|error| format!("valid single capture was rejected: {error}"))?;
    assert_eq!(capture.condition_id, CONDITION_ID);
    assert_eq!(capture.status, "ok");
    Ok(())
}

#[test]
fn response_reader_reports_missing_and_malformed_files() -> Result<(), String> {
    let temp = TempDirectory::new()?;
    let missing = read_response(&temp.path().join("missing.json"));
    assert!(
        missing
            .err()
            .is_some_and(|error| error.starts_with("read response:"))
    );

    let malformed_path = temp.path().join("malformed.json");
    fs::write(&malformed_path, b"{").map_err(|error| error.to_string())?;
    let malformed = read_response(&malformed_path);
    assert!(
        malformed
            .err()
            .is_some_and(|error| error.starts_with("parse response:"))
    );
    Ok(())
}

#[test]
fn probe_child_rejects_reused_response_path() -> Result<(), String> {
    let temp = TempDirectory::new()?;
    let response_path = temp.path().join("response.json");
    fs::write(&response_path, b"existing").map_err(|error| error.to_string())?;
    let error = run_probe_child(&request(), &response_path, PROBE_TEST)
        .err()
        .ok_or("reused response path must be rejected")?;
    assert_eq!(error, "probe request and response paths must be fresh");
    Ok(())
}

#[test]
fn probe_child_propagates_real_request_validation_failure() -> Result<(), String> {
    let temp = TempDirectory::new()?;
    let response_path = temp.path().join("invalid-response.json");
    let mut invalid = request();
    invalid.captures[0].condition_id.clear();
    let error = run_probe_child(&invalid, &response_path, PROBE_TEST)
        .err()
        .ok_or("invalid request must fail in the ignored probe child")?;
    assert!(error.starts_with("ignored font probe failed with status Some("));
    Ok(())
}

#[test]
fn probe_child_rejects_reused_request_path() -> Result<(), String> {
    let temp = TempDirectory::new()?;
    let response_path = temp.path().join("response.json");
    let request_path = response_path.with_extension("request.json");
    fs::write(&request_path, b"existing").map_err(|error| error.to_string())?;
    let error = run_probe_child(&request(), &response_path, PROBE_TEST)
        .err()
        .ok_or("reused request path must be rejected")?;
    assert_eq!(error, "probe request and response paths must be fresh");
    assert_eq!(
        fs::read(&request_path).map_err(|error| error.to_string())?,
        b"existing"
    );
    Ok(())
}

#[test]
fn probe_child_reports_request_write_failure() -> Result<(), String> {
    let temp = TempDirectory::new()?;
    let response_path = temp.path().join("missing-parent").join("response.json");
    let error = run_probe_child(&request(), &response_path, PROBE_TEST)
        .err()
        .ok_or("request write must fail when the parent is missing")?;
    assert!(error.starts_with("write request:"));
    Ok(())
}
