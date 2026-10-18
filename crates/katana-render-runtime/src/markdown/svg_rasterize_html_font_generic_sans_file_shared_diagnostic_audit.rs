use super::{Arm, Capture, EXPECTED_SELECTED_FACES, FaceSource, SourceRoute};
use sha2::{Digest, Sha256};

pub(super) fn normalized_faces(capture: &Capture) -> Vec<FaceSource> {
    let mut faces = capture.faces.clone();
    for face in &mut faces {
        face.kind.clear();
        face.stamp = None;
        face.bytes_sha256 = None;
    }
    faces
}
pub(super) fn audit_measured_sources(audit: &Capture, measured: &Capture) -> Result<(), String> {
    for face in &measured.faces {
        audit_one_source(audit, face)?;
    }
    Ok(())
}
pub(super) fn audit_one_source(audit: &Capture, face: &FaceSource) -> Result<(), String> {
    let expected = find_audited_face(audit, face)?;
    match &face.path {
        Some(path) => audit_file_source(path, expected),
        None => expected
            .bytes_sha256
            .as_ref()
            .map(|_| ())
            .ok_or_else(|| "binary source SHA missing from audit".into()),
    }
}
pub(super) fn find_audited_face<'a>(
    audit: &'a Capture,
    face: &FaceSource,
) -> Result<&'a FaceSource, String> {
    audit
        .faces
        .iter()
        .find(|saved| {
            saved.path == face.path
                && saved.index == face.index
                && saved.post_script_name == face.post_script_name
        })
        .ok_or_else(|| "measured face missing from untimed audit".into())
}
pub(super) fn audit_file_source(
    path: &std::path::Path,
    expected: &FaceSource,
) -> Result<(), String> {
    let stamp = crate::markdown::svg_rasterize::text_shaping::stamp_path_uncached(path)
        .map_err(|_| "font stamp failed after sample")?;
    if Some(format!("{stamp:?}")) != expected.stamp {
        return Err(format!("font stamp changed: {}", path.display()));
    }
    let bytes = std::fs::read(path).map_err(|_| "font source read failed after sample")?;
    let sha = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if Some(sha) != expected.bytes_sha256 {
        return Err(format!("font bytes changed: {}", path.display()));
    }
    Ok(())
}

pub(super) fn validate_shared_drop(arm: &Arm) -> Result<(), String> {
    if arm.route == SourceRoute::SharedFile
        && (!has_selected_database_weak(&arm.capture) || shared_owners_alive(&arm.capture))
    {
        return Err(
            "SharedFile transient database or byte owner missing or survived close/drop".into(),
        );
    }
    Ok(())
}
pub(super) fn has_selected_database_weak(capture: &Capture) -> bool {
    capture.database_owners.len() == 1
        && capture.selected_database_owners.len() == EXPECTED_SELECTED_FACES
        && capture.shared_byte_owners.len() == EXPECTED_SELECTED_FACES
}

pub(super) fn shared_owners_alive(capture: &Capture) -> bool {
    capture
        .selected_database_owners
        .iter()
        .any(|weak| weak.upgrade().is_some())
        || capture
            .database_owners
            .iter()
            .any(|weak| weak.upgrade().is_some())
        || capture
            .shared_byte_owners
            .iter()
            .any(|weak| weak.upgrade().is_some())
}
