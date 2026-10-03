use super::super::{LoadedCmap, ProbeResult, cache, finish_cmap, probe_file, read_cmap};
use super::fixtures::{bundled_bytes, malformed_parsed_cmap, valid_font_database};
use super::{CacheResetOnDrop, begin_cache_test, poison_global_cache};
use std::sync::Arc;

#[test]
fn face_index_mismatch_uses_original_probe() -> Result<(), String> {
    let _test_lock = begin_cache_test()?;
    let _reset = CacheResetOnDrop;
    let bytes = bundled_bytes()?;
    let file = valid_font_database(&bytes)?;
    let generation =
        super::super::super::file_generation::file_source_stamp(&file.database, file.face_id).2;
    let actual_index = file
        .database
        .face_source(file.face_id)
        .map(|(_, index)| index)
        .ok_or("fixture font source missing")?;
    let mismatched_index = actual_index.wrapping_add(1);
    assert!(matches!(
        read_cmap(&file.database, file.face_id, mismatched_index, 'A'),
        Some(LoadedCmap::UseOriginal)
    ));
    assert!(matches!(
        probe_file(&file.database, file.face_id, &generation, 'A'),
        ProbeResult::Complete(Some(true))
    ));
    Ok(())
}

#[test]
fn invalid_parsed_cmap_with_raw_table_keeps_ttf_parser_result_uncached() -> Result<(), String> {
    let _test_lock = begin_cache_test()?;
    let _reset = CacheResetOnDrop;
    let original = bundled_bytes()?;
    let malformed = malformed_parsed_cmap(&original)?;
    let file = valid_font_database(&original)?;
    std::fs::write(&file.file.0, &malformed).map_err(|error| error.to_string())?;
    let generation =
        super::super::super::file_generation::file_source_stamp(&file.database, file.face_id).2;
    assert!(matches!(
        probe_file(&file.database, file.face_id, &generation, 'A'),
        ProbeResult::Complete(Some(false))
    ));
    let (_, face_index) = file
        .database
        .face_source(file.face_id)
        .ok_or("fixture font source missing")?;
    assert!(
        cache()
            .lock()
            .map_err(|_| "cmap cache lock poisoned after unsupported cmap probe")?
            .lookup(&file.database, file.face_id, face_index, &generation)
            .is_none()
    );
    Ok(())
}

#[test]
fn stale_generation_does_not_insert_ready_cmap() -> Result<(), String> {
    let _test_lock = begin_cache_test()?;
    let _reset = CacheResetOnDrop;
    let bytes = bundled_bytes()?;
    let file = valid_font_database(&bytes)?;
    let old =
        super::super::super::file_generation::file_source_stamp(&file.database, file.face_id).2;
    std::fs::write(&file.file.0, [0; 32]).map_err(|error| error.to_string())?;
    assert!(matches!(
        finish_cmap(
            &file.database,
            file.face_id,
            0,
            &old,
            true,
            Arc::from([1_u8])
        ),
        ProbeResult::Complete(Some(true))
    ));
    Ok(())
}

#[test]
fn poisoned_lock_preserves_valid_probe_and_finish_semantics_then_recovers() -> Result<(), String> {
    let _test_lock = begin_cache_test()?;
    let _reset = CacheResetOnDrop;
    let bytes = bundled_bytes()?;
    let file = valid_font_database(&bytes)?;
    let generation =
        super::super::super::file_generation::file_source_stamp(&file.database, file.face_id).2;
    let parsed = rustybuzz::ttf_parser::Face::parse(&bytes, 0)
        .map_err(|_| "fixture font face did not parse")?;
    let supported = parsed.glyph_index('A').is_some();
    let cmap_bytes = parsed
        .raw_face()
        .table(rustybuzz::ttf_parser::Tag::from_bytes(b"cmap"))
        .ok_or("fixture raw cmap missing")?;
    let recovery = CacheResetOnDrop;
    poison_global_cache()?;
    super::super::remove_face(&file.database, file.face_id);
    assert_poisoned_probe_and_finish(
        &file.database,
        file.face_id,
        &generation,
        supported,
        cmap_bytes,
    )?;
    drop(recovery);
    assert!(!cache().is_poisoned());
    assert_recovered_probe(&file.database, file.face_id, &generation, supported);
    Ok(())
}

fn assert_poisoned_probe_and_finish(
    database: &Arc<resvg::usvg::fontdb::Database>,
    face_id: resvg::usvg::fontdb::ID,
    generation: &super::super::super::file_generation::FontSourceGeneration,
    supported: bool,
    cmap_bytes: &[u8],
) -> Result<(), String> {
    if !cache().is_poisoned() {
        return Err("cache was not poisoned for the fault probe".into());
    }
    let probe = probe_file(database, face_id, generation, 'A');
    if generation.durable_reusable() {
        assert!(matches!(probe, ProbeResult::UseOriginal));
    } else {
        assert!(matches!(probe, ProbeResult::Complete(Some(value)) if value == supported));
    }
    assert!(matches!(
        finish_cmap(
            database,
            face_id,
            0,
            generation,
            supported,
            Arc::from(cmap_bytes)
        ),
        ProbeResult::Complete(Some(_))
    ));
    Ok(())
}

fn assert_recovered_probe(
    database: &Arc<resvg::usvg::fontdb::Database>,
    face_id: resvg::usvg::fontdb::ID,
    generation: &super::super::super::file_generation::FontSourceGeneration,
    supported: bool,
) {
    assert!(matches!(
        probe_file(database, face_id, generation, 'A'),
        ProbeResult::Complete(Some(value)) if value == supported
    ));
}

#[test]
fn absent_raw_cmap_preserves_original_unsupported_result() -> Result<(), String> {
    let _test_lock = begin_cache_test()?;
    let _reset = CacheResetOnDrop;
    let mut bytes = bundled_bytes()?;
    let file = valid_font_database(&bytes)?;
    let count = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
    let record = (0..count)
        .map(|index| 12 + index * 16)
        .find(|index| bytes.get(*index..*index + 4) == Some(b"cmap"))
        .ok_or("fixture cmap directory missing")?;
    bytes[record..record + 4].copy_from_slice(b"zzzz");
    std::fs::write(&file.file.0, &bytes).map_err(|error| error.to_string())?;
    let generation =
        super::super::super::file_generation::file_source_stamp(&file.database, file.face_id).2;
    let face = rustybuzz::ttf_parser::Face::parse(&bytes, 0)
        .map_err(|_| "fixture face did not parse without cmap")?;
    assert!(
        face.raw_face()
            .table(rustybuzz::ttf_parser::Tag::from_bytes(b"cmap"))
            .is_none()
    );
    assert!(matches!(
        probe_file(&file.database, file.face_id, &generation, 'A'),
        ProbeResult::Complete(Some(false))
    ));
    Ok(())
}
