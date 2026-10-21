use super::super::{
    CaptureResult,
    identity::{
        FontResolutionCache, face_identity, load_resource_database, physical_file,
        resolve_font_with_cache, unique_file_face,
    },
    test_support::{TempDirectory, probe_single_capture},
};
use super::{
    ResourceFile, SelectedFont, append_changed_byte, duplicate_face_resources, selected_from_face,
};
use resvg::usvg::fontdb::Database;
use std::{fs, path::PathBuf};

pub(in super::super) fn observed_system_face_resolves_to_its_physical_file_and_index()
-> Result<(), String> {
    let mut database = Database::new();
    database.load_system_fonts();
    let (source_path, face) =
        unique_file_face(&database).ok_or("system font inventory has no unique physical face")?;
    let selected = selected_from_face(&face)?;
    let expected = face_identity(&database, &face)?;
    let mut cache = FontResolutionCache::for_system_database(&database);
    let first = resolve_font_with_cache(&database, &Database::new(), &[], &selected, &mut cache)?;
    let second = resolve_font_with_cache(&database, &Database::new(), &[], &selected, &mut cache)?;
    assert_eq!(first, second);
    assert_eq!(first.identity, expected);
    assert!(first.core_text_descriptor.is_none());
    assert_cache_rejects_different_database(&selected, &mut cache)?;
    cache.verify_physical_inputs()?;
    cached_direct_face_rejects_physical_mutation(&source_path)?;
    Ok(())
}

fn assert_cache_rejects_different_database(
    selected: &SelectedFont,
    cache: &mut FontResolutionCache<'_>,
) -> Result<(), String> {
    let other_database = Database::new();
    let error =
        match resolve_font_with_cache(&other_database, &Database::new(), &[], selected, cache) {
            Ok(_) => return Err("direct cache accepted a different system Database".to_string()),
            Err(error) => error,
        };
    if error.contains("Database binding differs") {
        return Ok(());
    }
    Err(format!("unexpected cache binding error: {error}"))
}

fn cached_direct_face_rejects_physical_mutation(source_path: &PathBuf) -> Result<(), String> {
    let temp = TempDirectory::new()?;
    let copied_path = temp
        .path()
        .join(source_path.file_name().ok_or("font path has no name")?);
    fs::copy(source_path, &copied_path).map_err(|error| error.to_string())?;
    let mut copied_database = Database::new();
    copied_database
        .load_font_file(&copied_path)
        .map_err(|error| error.to_string())?;
    let (_, copied_face) =
        unique_file_face(&copied_database).ok_or("copied font inventory has no unique face")?;
    let selected = selected_from_face(&copied_face)?;
    let mut cache = FontResolutionCache::for_system_database(&copied_database);
    resolve_font_with_cache(
        &copied_database,
        &Database::new(),
        &[],
        &selected,
        &mut cache,
    )?;
    append_changed_byte(&copied_path)?;
    let error = match cache.verify_physical_inputs() {
        Ok(()) => return Err("cached direct face accepted changed physical bytes".to_string()),
        Err(error) => error,
    };
    assert!(error.contains("physical font changed"));
    Ok(())
}

pub(in super::super) fn missing_and_ambiguous_physical_faces_remain_unknown() -> Result<(), String>
{
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

fn assert_unknown_capture(capture: &CaptureResult, reason_fragment: &str) -> Result<(), String> {
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

pub(in super::super) fn changed_custom_resource_bytes_are_rejected() -> Result<(), String> {
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

#[test]
fn resource_database_rejects_invalid_path_contracts_before_loading() -> Result<(), String> {
    let temp = TempDirectory::new()?;
    let absolute = temp.path().join("resource.ttf");
    fs::write(
        &absolute,
        include_bytes!("../../../assets/fonts/NotoSans-Regular.ttf"),
    )
    .map_err(|error| error.to_string())?;
    let (_, digest, _) = physical_file(&absolute)?;
    let empty_url = ResourceFile {
        url: String::new(),
        body_path: absolute.clone(),
        bytes_sha256: digest.clone(),
    };
    assert_eq!(
        load_resource_database(&[empty_url]).err().as_deref(),
        Some("resource must have a URL and absolute body path")
    );

    let relative = ResourceFile {
        url: "https://probe.invalid/font".to_string(),
        body_path: PathBuf::from("relative-font.ttf"),
        bytes_sha256: digest,
    };
    assert_eq!(
        load_resource_database(&[relative]).err().as_deref(),
        Some("resource must have a URL and absolute body path")
    );
    Ok(())
}
