use super::{
    PROBE_TEST,
    fixture::{CliFixture, selected_font},
};
use crate::markdown::chrome_font_identity_probe_tests::{
    CaptureRequest, ProbeRequest, REQUEST_SCHEMA, test_support::run_probe_child,
};
use std::path::Path;

pub(super) fn verify(fixture: &CliFixture) -> Result<(), String> {
    let response_path = fixture.temp.path().join("local-response.json");
    let response = run_probe_child(&local_font_request(fixture), &response_path, PROBE_TEST)?;
    let capture = response
        .captures
        .first()
        .ok_or("local capture response missing")?;
    assert_eq!(capture.status, "resolved");
    assert_provenance(capture, &fixture.family_name, fixture.face.index)
}

fn local_font_request(fixture: &CliFixture) -> ProbeRequest {
    ProbeRequest {
        schema: REQUEST_SCHEMA.to_string(),
        captures: vec![CaptureRequest {
            condition_id: "probe-local-face".to_string(),
            selected_fonts: vec![selected_font(
                fixture.family_name.clone(),
                fixture.face.post_script_name.clone(),
                3,
                false,
            )],
            resource_files: vec![fixture.nonfont_resource.clone()],
        }],
    }
}

fn assert_provenance(
    capture: &super::super::CaptureResult,
    family_name: &str,
    face_index: u32,
) -> Result<(), String> {
    let [mapped] = capture.font_provenance.as_slice() else {
        return Err("local font provenance count differs".to_string());
    };
    assert_eq!(mapped.scope, "capture");
    assert_eq!(mapped.family, family_name);
    if !Path::new(&mapped.file_path).is_absolute() {
        return Err("resolved font path is not absolute".to_string());
    }
    assert_eq!(mapped.face_index, face_index);
    assert_eq!(mapped.selected_node_id, None);
    assert!(mapped.selected && mapped.file_bytes_verified);
    Ok(())
}
