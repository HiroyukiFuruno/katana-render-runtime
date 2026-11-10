use super::super::test_support::TempDirectory;
use super::super::{CaptureRequest, ProbeRequest, ProbeResponse, REQUEST_SCHEMA, RESPONSE_SCHEMA};
use super::{probe, write_fresh_response};
use std::fs;

#[test]
fn probe_response_writer_rejects_an_inconsistent_schema_without_writing() -> Result<(), String> {
    let temp = TempDirectory::new()?;
    let output = temp.path().join("response.json");
    let mut response = response_fixture()?;
    response.schema = "krr-unsupported-response-v0".to_owned();

    assert_eq!(
        write_fresh_response(output.clone(), &response)
            .err()
            .as_deref(),
        Some("response schema is inconsistent")
    );
    assert!(!output.exists());

    response.schema = RESPONSE_SCHEMA.to_owned();
    write_fresh_response(output.clone(), &response)?;
    let bytes = fs::read(output).map_err(|error| error.to_string())?;
    let saved: ProbeResponse = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    assert_eq!(saved.schema, RESPONSE_SCHEMA);
    assert_eq!(saved.request_sha256, response.request_sha256);
    Ok(())
}

fn response_fixture() -> Result<ProbeResponse, String> {
    let request = ProbeRequest {
        schema: REQUEST_SCHEMA.to_owned(),
        captures: vec![CaptureRequest {
            condition_id: "response-schema".to_owned(),
            selected_fonts: Vec::new(),
            resource_files: Vec::new(),
        }],
    };
    let request_bytes = serde_json::to_vec(&request).map_err(|error| error.to_string())?;
    probe(&request_bytes)
}
