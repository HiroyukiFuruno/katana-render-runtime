use super::fixture::{CliFixture, selected_font};
use crate::markdown::chrome_font_identity_probe_tests::{
    CaptureRequest, ProbeRequest, REQUEST_SCHEMA,
};

pub(super) fn verify(fixture: &CliFixture) -> Result<(), String> {
    let response = fixture.run_request("mixed-empty-response.json", &mixed_request(fixture))?;
    assert_eq!(response.captures.len(), 2);
    let resolved = &response.captures[0];
    assert_eq!(resolved.condition_id, "probe-mixed-valid-face");
    assert_eq!(resolved.status, "resolved");
    assert_eq!(resolved.font_provenance.len(), 1);
    let unknown = &response.captures[1];
    assert_eq!(unknown.condition_id, "probe-mixed-empty-boundary");
    assert_eq!(unknown.status, "unknown");
    assert!(unknown.font_provenance.is_empty());
    assert_eq!(
        unknown.unknown_reasons,
        ["Chrome boundary inspection returned no selected fonts"]
    );
    Ok(())
}

fn mixed_request(fixture: &CliFixture) -> ProbeRequest {
    ProbeRequest {
        schema: REQUEST_SCHEMA.to_string(),
        captures: vec![
            CaptureRequest {
                condition_id: "probe-mixed-valid-face".to_string(),
                selected_fonts: vec![selected_font(
                    fixture.family_name.clone(),
                    fixture.face.post_script_name.clone(),
                    3,
                    false,
                )],
                resource_files: vec![fixture.nonfont_resource.clone()],
            },
            CaptureRequest {
                condition_id: "probe-mixed-empty-boundary".to_string(),
                selected_fonts: Vec::new(),
                resource_files: Vec::new(),
            },
        ],
    }
}
