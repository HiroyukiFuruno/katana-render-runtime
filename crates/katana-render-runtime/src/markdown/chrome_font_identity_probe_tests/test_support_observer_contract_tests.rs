use super::super::identity::physical_file;
use super::super::test_support::physical_font_fixture;
use super::super::{CaptureRequest, ObserverFace, ProbeRequest, REQUEST_SCHEMA, probe::probe};

#[test]
fn font_probe_rejects_duplicate_observer_font_identities() -> Result<(), String> {
    let (temp, _, selected) = physical_font_fixture()?;
    let (file_path, file_sha256, _) = physical_file(&temp.path().join("physical-font.ttf"))?;
    let observer = ObserverFace {
        post_script_name: selected.post_script_name,
        file_path,
        file_sha256,
        face_index: 0,
    };
    let request = ProbeRequest {
        schema: REQUEST_SCHEMA.to_string(),
        captures: vec![CaptureRequest {
            condition_id: "duplicate-observer-font".to_string(),
            selected_fonts: Vec::new(),
            resource_files: Vec::new(),
            observer_faces: vec![observer.clone(), observer],
        }],
    };
    let request_bytes = serde_json::to_vec(&request).map_err(|error| error.to_string())?;
    assert_eq!(
        probe(&request_bytes).err().as_deref(),
        Some("observer font identities must have unique PostScript names")
    );
    Ok(())
}
