use super::{CliFixture, append_changed_byte, copy_font_response, custom_font_request};
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
