use super::{
    ResourceFile, SelectedFont,
    identity::{
        face_identity, load_resource_database, physical_file, resolve_font, unique_file_face,
    },
    test_support::{TempDirectory, probe_single_capture},
};
use resvg::usvg::fontdb::Database;
use std::{fs, path::PathBuf};

pub(super) fn observed_system_face_resolves_to_its_physical_file_and_index() -> Result<(), String> {
    let mut database = Database::new();
    database.load_system_fonts();
    let (_, face) =
        unique_file_face(&database).ok_or("system font inventory has no unique physical face")?;
    let selected = selected_from_face(&face)?;
    let expected = face_identity(&database, &face)?;
    let resolved = resolve_font(&database, &Database::new(), &[], &selected)?;
    assert_eq!(resolved.identity, expected);
    assert!(resolved.core_text_descriptor.is_none());
    Ok(())
}

pub(super) fn selected_from_face(
    face: &resvg::usvg::fontdb::FaceInfo,
) -> Result<SelectedFont, String> {
    let family_name = face
        .families
        .first()
        .map(|(name, _)| name.clone())
        .ok_or("system font face has no family name")?;
    Ok(SelectedFont {
        family_name,
        post_script_name: face.post_script_name.clone(),
        glyph_count: 1,
        is_custom_font: false,
    })
}

pub(super) fn missing_and_ambiguous_physical_faces_remain_unknown() -> Result<(), String> {
    let mut system_database = Database::new();
    system_database.load_system_fonts();
    let (source_path, face) =
        unique_file_face(&system_database).ok_or("no unique physical system face")?;
    let (_temp, resources) = duplicate_face_resources(&source_path)?;
    let selected = selected_from_face(&face)?;
    let ambiguous = probe_single_capture(
        "ambiguous-physical-face",
        SelectedFont {
            is_custom_font: true,
            ..selected.clone()
        },
        resources,
    )?;
    assert_unknown_capture(&ambiguous, "ambiguous")?;
    let missing = probe_single_capture(
        "missing-physical-face",
        SelectedFont {
            post_script_name: "KRR-Unknown-PostScript-Face".to_string(),
            ..selected
        },
        Vec::new(),
    )?;
    assert_unknown_capture(&missing, "KRR-Unknown")?;
    Ok(())
}

fn duplicate_face_resources(
    source_path: &PathBuf,
) -> Result<(TempDirectory, Vec<ResourceFile>), String> {
    let temp = TempDirectory::new()?;
    let file_name = source_path.file_name().ok_or("font path has no name")?;
    let first_path = temp.path().join(file_name);
    let second_path = temp
        .path()
        .join(format!("copy-{}", file_name.to_string_lossy()));
    fs::copy(source_path, &first_path).map_err(|error| error.to_string())?;
    fs::copy(source_path, &second_path).map_err(|error| error.to_string())?;
    let (_, digest, _) = physical_file(&first_path)?;
    let resources = vec![
        resource("https://probe.invalid/font-a", first_path, digest.clone()),
        resource("https://probe.invalid/font-b", second_path, digest),
    ];
    Ok((temp, resources))
}

fn resource(url: &str, body_path: PathBuf, bytes_sha256: String) -> ResourceFile {
    ResourceFile {
        url: url.to_string(),
        body_path,
        bytes_sha256,
    }
}

fn assert_unknown_capture(
    capture: &super::CaptureResult,
    reason_fragment: &str,
) -> Result<(), String> {
    if capture.status != "unknown"
        || !capture.font_provenance.is_empty()
        || !capture
            .unknown_reasons
            .iter()
            .any(|reason| reason.contains(reason_fragment))
    {
        return Err(format!("font probe did not reject {reason_fragment}"));
    }
    Ok(())
}

pub(super) fn changed_custom_resource_bytes_are_rejected() -> Result<(), String> {
    let mut system_database = Database::new();
    system_database.load_system_fonts();
    let (source_path, _) = unique_file_face(&system_database).ok_or("no unique physical face")?;
    let temp = TempDirectory::new()?;
    let copied_path = temp
        .path()
        .join(source_path.file_name().ok_or("font path has no name")?);
    fs::copy(&source_path, &copied_path).map_err(|error| error.to_string())?;
    let (_, captured_sha, _) = physical_file(&copied_path)?;
    append_changed_byte(&copied_path)?;
    let stale_resource = ResourceFile {
        url: "https://probe.invalid/captured-font".to_string(),
        body_path: copied_path,
        bytes_sha256: captured_sha,
    };
    assert!(load_resource_database(&[stale_resource]).is_err());
    Ok(())
}

fn append_changed_byte(path: &PathBuf) -> Result<(), String> {
    let mut bytes = fs::read(path).map_err(|error| error.to_string())?;
    bytes.push(0);
    fs::write(path, bytes).map_err(|error| error.to_string())
}
