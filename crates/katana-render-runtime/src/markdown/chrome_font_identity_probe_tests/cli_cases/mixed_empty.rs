use super::fixture::{CliFixture, selected_font};
use crate::markdown::chrome_font_identity_probe_tests::{
    CaptureRequest, ProbeRequest, REQUEST_SCHEMA,
};

pub(super) fn verify(fixture: &CliFixture) -> Result<(), String> {
    let response = fixture.run_request("mixed-empty-response.json", &mixed_request(fixture))?;
    if response.captures.len() != 2 {
        return Err("mixed capture response count differs".to_string());
    }
    let resolved = &response.captures[0];
    assert_eq!(resolved.condition_id, "probe-mixed-valid-face");
    assert_eq!(resolved.status, "resolved");
    if resolved.font_provenance.len() != 1 {
        return Err("valid capture provenance was lost beside an unknown capture".to_string());
    }
    let unknown = &response.captures[1];
    assert_eq!(unknown.condition_id, "probe-mixed-empty-boundary");
    assert_eq!(unknown.status, "unknown");
    if !unknown.font_provenance.is_empty()
        || unknown.unknown_reasons != ["Chrome boundary inspection returned no selected fonts"]
    {
        return Err("empty-font capture was not retained as an explicit unknown".to_string());
    }
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
