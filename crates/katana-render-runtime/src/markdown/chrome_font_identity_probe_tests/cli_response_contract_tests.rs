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

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn response_writer_reports_real_file_size_limit() -> Result<(), Box<dyn std::error::Error>> {
    use crate::system::file_size_limit_test_support::FileSizeLimitTest;

    FileSizeLimitTest::run_in_child(
        module_path!(),
        "response_writer_reports_real_file_size_limit",
        || {
            let temp = TempDirectory::new()?;
            let output = temp.path().join("response.json");
            let response = response_fixture()?;
            let result = FileSizeLimitTest::with_zero_limit(|| {
                write_fresh_response(output.clone(), &response)
            })?;
            let error = result
                .err()
                .ok_or("response write unexpectedly succeeded")?;
            assert!(error.starts_with("write response:"), "{error}");
            assert!(output.is_file());
            Ok(())
        },
    )
}
