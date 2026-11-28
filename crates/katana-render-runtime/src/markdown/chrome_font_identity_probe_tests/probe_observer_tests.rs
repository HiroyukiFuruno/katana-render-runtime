use super::super::ObserverFace;
use super::super::identity::{FontResolutionCache, physical_file};
use super::super::test_support::physical_font_fixture;
use super::{ResolutionInputs, font_provenance, resolve_selected_identity};
use resvg::usvg::fontdb::Database;

#[test]
fn unresolved_system_face_uses_a_verified_retained_observer_face() -> Result<(), String> {
    let (temp, _, selected) = physical_font_fixture()?;
    let (file_path, file_sha256, _) = physical_file(&temp.path().join("physical-font.ttf"))?;
    let system_database = Database::new();
    let mut cache = FontResolutionCache::for_system_database(&system_database);
    let resolved = resolve_selected_identity(
        &system_database,
        ResolutionInputs {
            resource_database: &Database::new(),
            resources: &[],
            resource_error: None,
            observer_faces: &[ObserverFace {
                post_script_name: selected.post_script_name.clone(),
                file_path,
                file_sha256,
                face_index: 0,
            }],
        },
        &selected,
        &mut cache,
    )?;
    assert_eq!(resolved.method, "Chrome retained font observation");
    let provenance = font_provenance(selected, resolved);
    assert!(provenance.source.ends_with(
        "; Chrome retained font observation; exact PostScript-name match; physical file SHA and face index verified"
    ));
    Ok(())
}

#[test]
fn custom_faces_do_not_use_retained_observer_fallback() -> Result<(), String> {
    let (temp, _, mut selected) = physical_font_fixture()?;
    let (file_path, file_sha256, _) = physical_file(&temp.path().join("physical-font.ttf"))?;
    selected.is_custom_font = true;
    let system_database = Database::new();
    let mut cache = FontResolutionCache::for_system_database(&system_database);
    let result = resolve_selected_identity(
        &system_database,
        ResolutionInputs {
            resource_database: &Database::new(),
            resources: &[],
            resource_error: None,
            observer_faces: &[ObserverFace {
                post_script_name: selected.post_script_name.clone(),
                file_path,
                file_sha256,
                face_index: 0,
            }],
        },
        &selected,
        &mut cache,
    );
    assert!(result.is_err());
    Ok(())
}

#[test]
fn missing_retained_face_keeps_both_resolution_failures() -> Result<(), String> {
    let (_temp, _, selected) = physical_font_fixture()?;
    let system_database = Database::new();
    let mut cache = FontResolutionCache::for_system_database(&system_database);
    let Err(error) = resolve_selected_identity(
        &system_database,
        ResolutionInputs {
            resource_database: &Database::new(),
            resources: &[],
            resource_error: None,
            observer_faces: &[],
        },
        &selected,
        &mut cache,
    ) else {
        return Err("unproven system font was unexpectedly accepted".to_string());
    };
    let Some((native, retained)) = error.rsplit_once("; ") else {
        return Err("one of the font resolution failures was lost".to_string());
    };
    assert!(!native.is_empty());
    assert_eq!(
        retained,
        "Chrome retained font observation has no unique matching face"
    );
    Ok(())
}
