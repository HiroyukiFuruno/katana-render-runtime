use super::{ProbeResult, cache, probe_file};
#[path = "svg_rasterize_text_cmap_cache_fault_extra_tests.rs"]
mod extra_tests;
#[cfg(test)]
#[path = "svg_rasterize_text_cmap_cache_fault_fixture_tests.rs"]
mod fixture_tests;
#[path = "svg_rasterize_text_cmap_cache_fault_fixtures.rs"]
mod fixtures;
use fixtures::{bundled_bytes, oversized_cmap, valid_font_database};
use resvg::usvg::fontdb::Database;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::{Arc, Mutex, MutexGuard};

static TEST_LOCK: Mutex<()> = Mutex::new(());

pub(super) struct CacheResetOnDrop;

impl Drop for CacheResetOnDrop {
    fn drop(&mut self) {
        let shared_cache = cache();
        shared_cache.clear_poison();
        match shared_cache.lock() {
            Ok(mut guard) => *guard = super::storage::Cache::default(),
            Err(poisoned) => *poisoned.into_inner() = super::storage::Cache::default(),
        }
        shared_cache.clear_poison();
    }
}

pub(super) fn begin_cache_test() -> Result<MutexGuard<'static, ()>, String> {
    let test_lock = match TEST_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let shared_cache = cache();
    shared_cache.clear_poison();
    let mut guard = shared_cache
        .lock()
        .map_err(|_| "cmap cache lock remained poisoned")?;
    *guard = super::storage::Cache::default();
    drop(guard);
    Ok(test_lock)
}

pub(super) fn poison_global_cache() -> Result<(), String> {
    let guard = cache()
        .lock()
        .map_err(|_| "cmap cache lock was already poisoned")?;
    let result = catch_unwind(AssertUnwindSafe(move || {
        let _guard = guard;
        resume_unwind(Box::new(()));
    }));
    if result.is_err() {
        Ok(())
    } else {
        Err("cache-lock poison path did not unwind".into())
    }
}

#[test]
fn immutable_and_source_mismatch_use_original_probe() -> Result<(), String> {
    let _test_lock = begin_cache_test()?;
    let _reset = CacheResetOnDrop;
    let bytes = bundled_bytes()?;
    let file = valid_font_database(&bytes)?;
    let generation =
        super::super::file_generation::file_source_stamp(&file.database, file.face_id).2;
    let face = crate::markdown::svg_rasterize::font::bundled_font_db()
        .faces()
        .next()
        .ok_or("bundled face missing")?
        .clone();
    let mut binary = Database::new();
    let binary_id = binary.push_face_info(face);
    assert!(matches!(
        probe_file(&Arc::new(binary), binary_id, &generation, 'A'),
        ProbeResult::UseOriginal
    ));
    Ok(())
}

#[test]
fn corrupted_cached_cmap_is_removed_and_reloaded() -> Result<(), String> {
    let _test_lock = begin_cache_test()?;
    let _reset = CacheResetOnDrop;
    let bytes = bundled_bytes()?;
    let file = valid_font_database(&bytes)?;
    let generation =
        super::super::file_generation::file_source_stamp(&file.database, file.face_id).2;
    cache()
        .lock()
        .map_err(|_| "cmap cache lock poisoned before corrupt-entry setup")?
        .insert(
            &file.database,
            file.face_id,
            0,
            generation.clone(),
            Arc::from([0_u8, 1]),
        );
    assert!(matches!(
        probe_file(&file.database, file.face_id, &generation, 'A'),
        ProbeResult::UseOriginal
    ));
    assert!(matches!(
        probe_file(&file.database, file.face_id, &generation, 'A'),
        ProbeResult::Complete(Some(true))
    ));
    Ok(())
}

#[test]
fn deleted_file_after_generation_returns_none() -> Result<(), String> {
    let _test_lock = begin_cache_test()?;
    let _reset = CacheResetOnDrop;
    let bytes = bundled_bytes()?;
    let file = valid_font_database(&bytes)?;
    let generation =
        super::super::file_generation::file_source_stamp(&file.database, file.face_id).2;
    std::fs::remove_file(&file.file.0).map_err(|error| error.to_string())?;
    assert!(matches!(
        probe_file(&file.database, file.face_id, &generation, 'A'),
        ProbeResult::Complete(None)
    ));
    Ok(())
}

#[test]
fn invalid_file_after_generation_returns_none() -> Result<(), String> {
    let _test_lock = begin_cache_test()?;
    let _reset = CacheResetOnDrop;
    let bytes = bundled_bytes()?;
    let file = valid_font_database(&bytes)?;
    let generation =
        super::super::file_generation::file_source_stamp(&file.database, file.face_id).2;
    std::fs::write(&file.file.0, [0; 32]).map_err(|error| error.to_string())?;
    assert!(matches!(
        probe_file(&file.database, file.face_id, &generation, 'A'),
        ProbeResult::Complete(None)
    ));
    Ok(())
}

#[test]
fn oversized_cmap_uses_original_glyph_probe() -> Result<(), String> {
    let _test_lock = begin_cache_test()?;
    let _reset = CacheResetOnDrop;
    let original = bundled_bytes()?;
    let enlarged = oversized_cmap(&original, super::storage::MAX_ENTRY_BYTES)?;
    let file = valid_font_database(&enlarged)?;
    let generation =
        super::super::file_generation::file_source_stamp(&file.database, file.face_id).2;
    let actual = rustybuzz::ttf_parser::Face::parse(&enlarged, 0)
        .map_err(|_| "enlarged font face did not parse")?
        .glyph_index('A');
    assert!(actual.is_some());
    assert!(matches!(
        probe_file(&file.database, file.face_id, &generation, 'A'),
        ProbeResult::UseOriginal
    ));
    let (fallback, observed_generation) =
        super::super::cached_font_has_char_with_generation(&file.database, file.face_id, 'A');
    assert_eq!(fallback, Some(actual.is_some()));
    assert_eq!(observed_generation, generation);
    Ok(())
}
