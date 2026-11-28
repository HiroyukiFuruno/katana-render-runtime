use super::*;

#[test]
fn file_generation_change_reloads_real_font_face() -> Result<(), String> {
    let path = super::temp_font_path("generation");
    std::fs::write(&path, super::FONT_BYTES).map_err(|error| error.to_string())?;
    let (database, face) = super::file_database(&path)?;
    let scope = FontFaceCacheScope::enter();
    let first = font_for_face(&database, face).ok_or("initial file face missing")?;
    shape_and_commit(&database, face, &first, "initial file")?;
    let first_key = active_cache_key();
    std::fs::write(&path, [super::FONT_BYTES, &[0_u8][..]].concat())
        .map_err(|error| error.to_string())?;
    let second = font_for_face(&database, face).ok_or("changed file face missing")?;
    assert!(!super::super::commit_file_font(&database, face, &first));
    shape_and_commit(&database, face, &second, "changed file")?;
    let final_key = active_cache_key();
    assert_generation_keys(first_key, final_key);
    drop(scope);
    std::fs::remove_file(path).map_err(|error| error.to_string())?;
    Ok(())
}
fn assert_generation_keys(
    first: Option<super::super::CacheKey>,
    final_key: Option<super::super::CacheKey>,
) {
    if first.is_some() {
        assert!(final_key.is_some());
        assert_ne!(first, final_key);
    } else {
        assert!(final_key.is_none());
    }
}
fn active_cache_key() -> Option<super::super::CacheKey> {
    super::super::ACTIVE_SCOPE
        .with(|cache| cache.borrow().entry.as_ref().map(|entry| entry.key.clone()))
}
#[test]
fn malformed_file_source_clears_real_face_entry() -> Result<(), String> {
    let mut corrupted = super::FONT_BYTES.to_vec();
    corrupted[0] = !corrupted[0];
    assert_invalid_file_source(corrupted, "corrupt")?;
    assert_invalid_file_source(
        super::FONT_BYTES[..super::FONT_BYTES.len() / TRUNCATION_DIVISOR].to_vec(),
        "truncated",
    )?;
    Ok(())
}
fn assert_invalid_file_source(bytes: Vec<u8>, label: &str) -> Result<(), String> {
    let path = super::temp_font_path(label);
    std::fs::write(&path, super::FONT_BYTES).map_err(|error| error.to_string())?;
    let (database, face) = super::file_database(&path)?;
    let _scope = FontFaceCacheScope::enter();
    let valid = font_for_face(&database, face).ok_or("valid file face missing")?;
    shape_and_commit(&database, face, &valid, "valid file")?;
    std::fs::write(&path, bytes).map_err(|error| error.to_string())?;
    assert!(font_for_face(&database, face).is_none());
    assert!(super::super::ACTIVE_SCOPE.with(|cache| cache.borrow().entry.is_none()));
    assert_all_handles_cleared();
    std::fs::remove_file(path).map_err(|error| error.to_string())?;
    Ok(())
}
#[test]
fn oversize_file_source_is_not_retained() -> Result<(), String> {
    let path = super::temp_font_path("oversize");
    let mut bytes = Vec::with_capacity(super::MAX_SOURCE_BYTES + super::FONT_BYTES.len());
    bytes.extend_from_slice(super::FONT_BYTES);
    bytes.resize(super::MAX_SOURCE_BYTES + 1, 0);
    std::fs::write(&path, bytes).map_err(|error| error.to_string())?;
    let (database, face) = super::file_database(&path)?;
    let _scope = FontFaceCacheScope::enter();
    assert!(font_for_face(&database, face).is_some());
    assert!(super::super::ACTIVE_SCOPE.with(|cache| cache.borrow().entry.is_none()));
    assert_all_handles_cleared();
    std::fs::remove_file(path).map_err(|error| error.to_string())?;
    Ok(())
}
#[test]
fn non_ttc_face_index_is_rejected_without_cache_entry() -> Result<(), String> {
    let path = super::temp_font_path("non-ttc-index");
    std::fs::write(&path, super::FONT_BYTES).map_err(|error| error.to_string())?;
    let (source_database, _) = super::file_database(&path)?;
    let mut info = source_database
        .faces()
        .next()
        .ok_or("source face missing")?
        .clone();
    info.source = Source::File(path.clone());
    info.index = 1;
    let mut database = Database::new();
    let face = database.push_face_info(info);
    let _scope = FontFaceCacheScope::enter();
    assert!(font_for_face(&database, face).is_none());
    assert!(super::super::ACTIVE_SCOPE.with(|cache| cache.borrow().entry.is_none()));
    assert_all_handles_cleared();
    std::fs::remove_file(path).map_err(|error| error.to_string())?;
    Ok(())
}
#[cfg(target_os = "macos")]
#[test]
fn removed_real_file_poison_does_not_enter_cache() -> Result<(), String> {
    let source = Path::new("/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc");
    if !source.exists() {
        return Ok(());
    }
    let path = super::temp_font_path("poison");
    std::fs::copy(source, &path).map_err(|error| error.to_string())?;
    let (database, face) = super::file_database(&path)?;
    let _scope = FontFaceCacheScope::enter();
    let lazy = font_for_face(&database, face).ok_or("lazy file face missing")?;
    std::fs::remove_file(&path).map_err(|error| error.to_string())?;
    let _ = shape_font(
        &lazy,
        TTC_TEXT,
        FontShape::new(TTC_SIZE, POISON_WEIGHT, false),
        Some("kern"),
    );
    assert!(!super::super::commit_file_font(&database, face, &lazy));
    assert!(super::super::ACTIVE_SCOPE.with(|cache| cache.borrow().entry.is_none()));
    assert_all_handles_cleared();
    Ok(())
}
