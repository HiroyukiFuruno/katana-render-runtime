#[cfg(windows)]
#[path = "svg_rasterize_text_cmap_test_support.rs"]
mod cmap_test_support;
#[cfg(windows)]
use cmap_test_support::{font_with_only_b_cmap, rewrite_with_same_modified_time};

use super::file_test_support::CmapFileFixture;
use super::{ProbeResult, probe_file};
#[cfg(windows)]
use crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES;
#[cfg(unix)]
use resvg::usvg::fontdb::Database;
use std::sync::atomic::AtomicUsize;

static TEMP_FILE: AtomicUsize = AtomicUsize::new(0);
const FILES: CmapFileFixture = CmapFileFixture::new("krr-cmap-cache", &TEMP_FILE);
#[cfg(windows)]
type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn file_cmap_matches_ttf_parser_for_positive_and_negative_characters() -> Result<(), String> {
    let (database, id, _guard) = FILES.bundled_database()?;
    let generation = super::super::file_generation::file_source_stamp(&database, id).2;
    assert!(matches!(
        probe_file(&database, id, &generation, 'A'),
        ProbeResult::Complete(Some(true))
    ));
    assert!(matches!(
        probe_file(&database, id, &generation, '\u{10ffff}'),
        ProbeResult::Complete(Some(false))
    ));
    Ok(())
}

#[test]
fn invalid_file_keeps_original_probe_failure_semantics() -> Result<(), String> {
    let (database, id, _guard) = FILES.database(&[0; 32])?;
    let generation = super::super::file_generation::file_source_stamp(&database, id).2;
    assert!(matches!(
        probe_file(&database, id, &generation, 'A'),
        ProbeResult::Complete(None)
    ));
    Ok(())
}

#[test]
fn changed_file_generation_does_not_reuse_old_cmap() -> Result<(), String> {
    let (database, id, guard) = FILES.bundled_database()?;
    let first = super::super::file_generation::file_source_stamp(&database, id).2;
    assert!(matches!(
        probe_file(&database, id, &first, 'A'),
        ProbeResult::Complete(Some(true))
    ));
    std::fs::write(&guard.0, [0; 32]).map_err(|error| error.to_string())?;
    let second = super::super::file_generation::file_source_stamp(&database, id).2;
    assert_ne!(first, second);
    assert!(matches!(
        probe_file(&database, id, &second, 'A'),
        ProbeResult::Complete(None)
    ));
    Ok(())
}

#[test]
fn missing_file_uses_original_probe_without_cache_entry() -> Result<(), String> {
    let (database, id, guard) = FILES.bundled_database()?;
    std::fs::remove_file(&guard.0).map_err(|error| error.to_string())?;
    let generation = super::super::file_generation::file_source_stamp(&database, id).2;
    assert!(matches!(
        probe_file(&database, id, &generation, 'A'),
        ProbeResult::UseOriginal
    ));
    Ok(())
}

#[cfg(unix)]
fn assert_cached_cmap(
    database: &std::sync::Arc<Database>,
    id: resvg::usvg::fontdb::ID,
    generation: &super::super::file_generation::FontSourceGeneration,
    expected: bool,
) -> Result<(), String> {
    let face_index = database
        .face_source(id)
        .map(|(_, index)| index)
        .ok_or("file face missing")?;
    let cached = super::cached_file_cmap(database, id, face_index, generation)
        .map_err(|()| "cmap cache lock poisoned")?;
    assert_eq!(cached.is_some(), expected);
    Ok(())
}

#[cfg(unix)]
#[test]
fn unavailable_generation_invalidates_existing_durable_cmap_entry() -> Result<(), String> {
    let (database, id, guard) = FILES.bundled_database()?;
    let durable = super::super::file_generation::file_source_stamp(&database, id).2;
    let filesystem_is_durable = super::super::file_stamp_durable_reusable(&guard.0);
    assert_eq!(durable.durable_reusable(), filesystem_is_durable);
    assert!(matches!(
        probe_file(&database, id, &durable, 'A'),
        ProbeResult::Complete(Some(true))
    ));
    assert_cached_cmap(&database, id, &durable, filesystem_is_durable)?;
    std::fs::remove_file(&guard.0).map_err(|error| error.to_string())?;
    let unavailable = super::super::file_generation::file_source_stamp(&database, id).2;
    assert!(!unavailable.reusable());
    assert!(!unavailable.durable_reusable());
    assert_cached_cmap(&database, id, &unavailable, false)?;
    assert_cached_cmap(&database, id, &durable, false)?;
    Ok(())
}

#[cfg(windows)]
#[test]
fn same_length_same_modified_file_rewrite_does_not_reuse_cached_cmap() -> TestResult {
    let original = FONT_BYTES;
    let replacement = font_with_only_b_cmap(original)?;
    let (database, id, guard) = FILES.database(original)?;
    let before = super::super::file_generation::file_source_stamp(&database, id).2;
    assert!(!before.durable_reusable());
    assert!(matches!(
        probe_file(&database, id, &before, 'A'),
        ProbeResult::Complete(Some(true))
    ));

    rewrite_with_same_modified_time(&guard.0, &replacement)?;
    let after = super::super::file_generation::file_source_stamp(&database, id).2;

    assert_eq!(before, after);
    assert!(matches!(
        probe_file(&database, id, &after, 'A'),
        ProbeResult::Complete(Some(false))
    ));
    Ok(())
}

#[cfg(windows)]
fn cmap_directory(font: &[u8]) -> Result<(usize, usize, usize), String> {
    let table_count = usize::from(u16::from_be_bytes([font[4], font[5]]));
    let mut cmap_record = None;
    let mut head_offset = None;
    for index in 0..table_count {
        let record = 12 + index * 16;
        let tag = font
            .get(record..record + 4)
            .ok_or("truncated sfnt directory")?;
        let offset = u32::from_be_bytes(
            font.get(record + 8..record + 12)
                .ok_or("truncated sfnt table record")?
                .try_into()
                .map_err(|_| "invalid sfnt table offset")?,
        ) as usize;
        if tag == b"cmap" {
            cmap_record = Some((record, offset));
        } else if tag == b"head" {
            head_offset = Some(offset);
        }
    }
    let (record, offset) = cmap_record.ok_or("cmap table missing")?;
    Ok((record, offset, head_offset.ok_or("head table missing")?))
}
