use super::fixture::{CliFixture, selected_font};
use crate::markdown::chrome_font_identity_probe_tests::{
    CaptureRequest, ProbeRequest, REQUEST_SCHEMA,
};

pub(super) fn verify(fixture: &CliFixture) -> Result<(), String> {
    let response = fixture.run_request("unknown-response.json", &unknown_font_request(fixture))?;
    let capture = response
        .captures
        .first()
        .ok_or("unknown capture response missing")?;
    assert_eq!(capture.status, "unknown");
    assert!(capture.font_provenance.is_empty());
    assert_eq!(capture.unknown_reasons.len(), 1);
    Ok(())
}

fn unknown_font_request(fixture: &CliFixture) -> ProbeRequest {
    ProbeRequest {
        schema: REQUEST_SCHEMA.to_string(),
        captures: vec![CaptureRequest {
            condition_id: "probe-unknown-face".to_string(),
            selected_fonts: vec![selected_font(
                fixture.family_name.clone(),
                "KRR-Unknown-PostScript-Face".to_string(),
                1,
                false,
            )],
            resource_files: Vec::new(),
        }],
    }
}
