use super::super::test_support::{PROBE_TEST, TempDirectory, read_response, run_probe_child};
use super::super::{
    CaptureRequest, CaptureResult, ProbeRequest, ProbeResponse, REQUEST_SCHEMA, RESPONSE_SCHEMA,
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
