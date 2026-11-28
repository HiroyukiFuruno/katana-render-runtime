use super::*;

pub(super) fn unavailable_generation_rejects_global_cmap_storage() -> Result<(), String> {
    let _lock = begin()?;
    let _reset = CacheResetOnDrop;
    let fixture = file_database(FONT_BYTES)?;
    remove_font_file(&fixture.file.0)?;
    let database = &fixture.database;
    let id = fixture.file_id;
    let generation = generation(database, id);
    assert!(!generation.durable_reusable());
    assert_legacy_entry_is_removed(database, id, generation.clone())?;
    assert_insert_is_not_persisted(database, id, generation)?;
    Ok(())
}

pub(super) fn remove_font_file(path: &std::path::Path) -> Result<(), String> {
    std::fs::remove_file(path).map_err(|error| error.to_string())
}

pub(super) fn assert_missing_font_removal(path: &std::path::Path) -> Result<(), String> {
    let expected = std::fs::metadata(path)
        .err()
        .ok_or("removed font metadata unexpectedly available")?;
    assert_eq!(expected.kind(), std::io::ErrorKind::NotFound);
    assert_eq!(remove_font_file(path), Err(expected.to_string()));
    Ok(())
}

#[cfg(windows)]
pub(super) fn weak_file_generation_never_enters_or_reuses_global_cmap_storage() -> Result<(), String>
{
    let _lock = begin()?;
    let _reset = CacheResetOnDrop;
    let fixture = file_database(FONT_BYTES)?;
    let generation = generation(&fixture.database, fixture.file_id);
    assert!(generation.reusable());
    assert!(!generation.durable_reusable());
    assert_insert_is_not_persisted(&fixture.database, fixture.file_id, generation.clone())?;
    assert!(
        lookup(&fixture.database, fixture.file_id, 0, &generation)
            .map_err(|_| "cache lookup failed")?
            .is_none()
    );
    Ok(())
}

pub(super) fn same_length_same_modified_rewrite_reprobes_without_stale_cmap_result()
-> Result<(), String> {
    let _lock = begin()?;
    let _reset = CacheResetOnDrop;
    let replacement = font_with_only_b_cmap(FONT_BYTES)?;
    let fixture = file_database(FONT_BYTES)?;
    verify_rewrite_reprobes(&fixture, &replacement)
}
