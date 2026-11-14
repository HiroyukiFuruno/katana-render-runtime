use super::{
    PROBE_TEST,
    fixture::{CliFixture, selected_font},
};
use crate::markdown::chrome_font_identity_probe_tests::{
    CaptureRequest, ProbeRequest, REQUEST_SCHEMA, ResourceFile, identity::physical_file,
    test_support::run_probe_child,
};
use std::{fs, path::Path};

pub(super) fn verify(fixture: &CliFixture) -> Result<(), String> {
    let custom_path = copy_font_response(fixture)?;
    let request = custom_font_request(fixture, &custom_path)?;
    verify_custom_response(fixture, &request, &custom_path)?;
    append_changed_byte(&custom_path)?;
    verify_changed_body(fixture, &request)
}

fn copy_font_response(fixture: &CliFixture) -> Result<std::path::PathBuf, String> {
    let (_, source_sha, source_bytes) = physical_file(&fixture.source_path)?;
    let path = fixture.temp.path().join(format!("0-{source_sha}.body"));
    fs::write(&path, source_bytes).map_err(|error| format!("write font response body: {error}"))?;
    Ok(path)
}

fn custom_font_request(fixture: &CliFixture, custom_path: &Path) -> Result<ProbeRequest, String> {
    let (_, resource_sha, _) = physical_file(custom_path)?;
    let selected = selected_font(
        fixture.family_name.clone(),
        fixture.face.post_script_name.clone(),
        2,
        true,
    );
    let font_resource = ResourceFile {
        url: "https://font-probe.invalid/captured-font".to_string(),
        body_path: custom_path.to_path_buf(),
        bytes_sha256: resource_sha,
    };
    Ok(ProbeRequest {
        schema: REQUEST_SCHEMA.to_string(),
        captures: vec![CaptureRequest {
            condition_id: "probe-custom-face".to_string(),
            selected_fonts: vec![selected],
            resource_files: vec![font_resource, fixture.nonfont_resource.clone()],
        }],
    })
}

fn verify_custom_response(
    fixture: &CliFixture,
    request: &ProbeRequest,
    custom_path: &Path,
) -> Result<(), String> {
    let response_path = fixture.temp.path().join("custom-response.json");
    let response = run_probe_child(request, &response_path, PROBE_TEST)?;
    let capture = response
        .captures
        .first()
        .ok_or("custom capture response missing")?;
    assert_eq!(capture.status, "resolved");
    assert_custom_provenance(capture, custom_path)
}

fn assert_custom_provenance(
    capture: &super::super::CaptureResult,
    custom_path: &Path,
) -> Result<(), String> {
    let provenance = capture
        .font_provenance
        .first()
        .ok_or("custom font provenance missing")?;
    assert!(
        provenance
            .source
            .contains("captured Chrome resource response body"),
        "custom font source must identify the captured response"
    );
    let expected_path = fs::canonicalize(custom_path)
        .map_err(|error| format!("canonicalize custom response: {error}"))?;
    assert_eq!(provenance.file_path, expected_path.display().to_string());
    Ok(())
}

fn append_changed_byte(path: &Path) -> Result<(), String> {
    let mut bytes = fs::read(path).map_err(|error| error.to_string())?;
    bytes.push(0);
    fs::write(path, bytes).map_err(|error| error.to_string())
}

fn verify_changed_body(fixture: &CliFixture, request: &ProbeRequest) -> Result<(), String> {
    let response = run_probe_child(
        request,
        &fixture.temp.path().join("changed-response.json"),
        PROBE_TEST,
    )?;
    let capture = response
        .captures
        .first()
        .ok_or("changed capture response missing")?;
    assert_eq!(capture.status, "unknown");
    assert!(capture.font_provenance.is_empty());
    assert!(!capture.unknown_reasons.is_empty());
    Ok(())
}

#[cfg(test)]
#[path = "custom_tests.rs"]
mod tests;
