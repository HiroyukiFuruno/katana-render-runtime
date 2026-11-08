use super::super::super as cache;
use nix::{mount::MntFlags, sys::statfs::statfs};
use std::{os::unix::fs::MetadataExt, path::Path};

fn immutable_mapping_observation(path: &Path, index: u32) -> Result<serde_json::Value, String> {
    let canonical = path.canonicalize().map_err(|error| error.to_string())?;
    let font_mount = statfs(&canonical).map_err(|error| error.to_string())?;
    let flags = font_mount.flags();
    let same_device = std::fs::metadata(&canonical)
        .map_err(|error| error.to_string())?
        .dev()
        == std::fs::metadata("/")
            .map_err(|error| error.to_string())?
            .dev();
    let under_system_fonts = canonical.starts_with("/System/Library/Fonts/");
    let eligible = production_eligibility(path, index)?;
    Ok(serde_json::json!({
        "path": path.display().to_string(),
        "canonicalPath": canonical.display().to_string(),
        "faceIndex": index,
        "originalPathIsCanonical": path == canonical,
        "underSystemFonts": under_system_fonts,
        "filesystem": font_mount.filesystem_type_name(),
        "readonly": flags.contains(MntFlags::MNT_RDONLY),
        "rootfs": flags.contains(MntFlags::MNT_ROOTFS),
        "snapshot": flags.contains(MntFlags::MNT_SNAPSHOT),
        "sameRootDevice": same_device,
        "eligible": eligible,
    }))
}

fn production_eligibility(path: &Path, index: u32) -> Result<bool, String> {
    let (database, face) = super::database_for_index(path, index)?;
    let length = std::fs::metadata(path)
        .map_err(|error| error.to_string())?
        .len();
    cache::immutable_mapping::eligible_source(&database, face, path, index, length)
}

fn cache_has_entry() -> bool {
    cache::ACTIVE_SCOPE.with(|scope| scope.borrow().entry.is_some())
}

fn assert_uncommitted_mapping(font: &cache::FontHandle) {
    assert!(font.poisoned.is_none());
    assert!(font.key.is_some());
    assert!(!cache_has_entry());
}

#[test]
fn fontique_mmap_and_lazy_helvetica_face_one_repeat_probe() -> Result<(), String> {
    let path = super::helvetica_path();
    let expected_sha = super::source_sha(&path)?;
    let _ = super::database_for_index(&path, 1)?;
    super::run_repeated_probe("Helvetica.ttc", &path, 1, &expected_sha)
}

#[test]
fn immutable_helvetica_face_reuses_only_after_shape_commit() -> Result<(), String> {
    let path = super::helvetica_path();
    let (database, face) = super::database_for_index(&path, 1)?;
    assert!(production_eligibility(&path, 1)?);
    let scope = cache::FontFaceCacheScope::enter();
    let first = cache::font_for_face(&database, face).ok_or("mapped face missing")?;
    assert_uncommitted_mapping(&first);
    assert!(super::super::shape_width(&first, 16.0, "AV")? > 0.0);
    assert!(!cache_has_entry());
    assert!(cache::commit_file_font(&database, face, &first));
    let cached = cache::font_for_face(&database, face).ok_or("cached face missing")?;
    assert!(cached.clean(&database, face));
    assert!(cache_has_entry());
    drop(scope);
    assert!(!cache_has_entry());
    Ok(())
}

#[test]
fn canonical_system_font_immutable_mapping_eligibility() -> Result<(), String> {
    let path = super::helvetica_path();
    let (database, face) = super::database_for_index(&path, 1)?;
    let length = std::fs::metadata(&path)
        .map_err(|error| error.to_string())?
        .len();
    let production_eligible =
        super::super::super::immutable_mapping::eligible_source(&database, face, &path, 1, length)?;
    let observation = immutable_mapping_observation(&path, 1)?;
    assert_eq!(observation["eligible"], production_eligible);
    println!("{observation}");
    Ok(())
}
