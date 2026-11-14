use super::{
    CliFixture, append_changed_byte, assert_custom_provenance, copy_font_response,
    custom_font_request, verify_changed_body,
};
use crate::markdown::chrome_font_identity_probe_tests::identity::physical_file;
use std::fs;

#[test]
fn custom_font_rejects_missing_source_and_resource_files() -> Result<(), String> {
    let mut fixture = CliFixture::new()?;
    let missing = fixture.temp.path().join("missing-font.ttf");
    fixture.source_path = missing.clone();
    let source_error = copy_font_response(&fixture)
        .err()
        .ok_or("missing source must be rejected")?;
    assert!(source_error.starts_with("canonicalize font:"));
    let resource_error = custom_font_request(&fixture, &missing)
        .err()
        .ok_or("missing resource must be rejected")?;
    assert!(resource_error.starts_with("canonicalize font:"));
    assert!(append_changed_byte(&missing).is_err());
    Ok(())
}

#[test]
fn custom_font_rejects_a_directory_as_the_response_body() -> Result<(), String> {
    let fixture = CliFixture::new()?;
    let (_, digest, _) = physical_file(&fixture.source_path)?;
    let response_path = fixture.temp.path().join(format!("0-{digest}.body"));
    fs::create_dir(&response_path).map_err(|error| error.to_string())?;
    let error = copy_font_response(&fixture)
        .err()
        .ok_or("directory response body must be rejected")?;
    assert!(error.starts_with("write font response body:"));
    assert!(response_path.is_dir());
    Ok(())
}

#[test]
fn custom_font_probe_propagates_rejection_of_empty_condition_id() -> Result<(), String> {
    let fixture = CliFixture::new()?;
    let path = copy_font_response(&fixture)?;
    let mut request = custom_font_request(&fixture, &path)?;
    request.captures[0].condition_id.clear();
    let error = verify_changed_body(&fixture, &request)
        .err()
        .ok_or("invalid custom request was accepted")?;
    assert!(error.starts_with("ignored font probe failed with status Some("));
    assert!(!fixture.temp.path().join("changed-response.json").exists());
    Ok(())
}

#[test]
fn custom_provenance_rejects_a_removed_captured_font() -> Result<(), String> {
    let fixture = CliFixture::new()?;
    let path = copy_font_response(&fixture)?;
    let request = custom_font_request(&fixture, &path)?;
    let response = fixture.run_request("before-removal.json", &request)?;
    let capture = response.captures.first().ok_or("capture missing")?;
    assert_eq!(capture.status, "resolved");
    assert_custom_provenance(capture, &path)?;
    fs::remove_file(&path).map_err(|error| error.to_string())?;
    let error = assert_custom_provenance(capture, &path)
        .err()
        .ok_or("removed captured font was accepted")?;
    assert!(error.starts_with("canonicalize custom response:"));
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn custom_font_body_write_reports_real_file_size_limit() -> Result<(), Box<dyn std::error::Error>> {
    use crate::system::file_size_limit_test_support::FileSizeLimitTest;

    FileSizeLimitTest::run_in_child(
        module_path!(),
        "custom_font_body_write_reports_real_file_size_limit",
        || {
            let fixture = CliFixture::new()?;
            let path = copy_font_response(&fixture)?;
            assert!(fs::metadata(&path)?.len() > 0);
            let result = FileSizeLimitTest::with_zero_limit(|| append_changed_byte(&path))?;
            let error = result
                .err()
                .ok_or("custom font write unexpectedly succeeded")?;
            assert!(
                error.contains(&FileSizeLimitTest::limit_error().to_string()),
                "{error}"
            );
            Ok(())
        },
    )
}
