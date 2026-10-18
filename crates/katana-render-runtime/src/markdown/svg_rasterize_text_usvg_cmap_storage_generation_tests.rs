#[path = "svg_rasterize_text_cmap_test_support.rs"]
mod cmap_test_support;
use cmap_test_support::{font_with_only_b_cmap, rewrite_with_same_modified_time};

use super::super::super::super::font::FontSourceGeneration;
use super::{CacheResetOnDrop, begin};
use super::{FONT_BYTES, cache, entry, generation, insert, lookup, mapping};
use resvg::usvg::fontdb::Source;
use resvg::usvg::fontdb::{Database, ID};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct TestFontFile(std::path::PathBuf);

struct FileDatabase {
    database: Arc<Database>,
    base_id: ID,
    file_id: ID,
    file: TestFontFile,
}

impl Drop for TestFontFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn file_database(bytes: &[u8]) -> Result<FileDatabase, String> {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "krr-usvg-cmap-storage-{}-{}.ttf",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, bytes).map_err(|error| error.to_string())?;
    let mut database = Database::new();
    database.load_font_data(FONT_BYTES.to_vec());
    database.load_font_source(Source::File(path.clone()));
    let database = Arc::new(database);
    let ids = database.faces().map(|face| face.id).collect::<Vec<_>>();
    let base_id = *ids.first().ok_or("base font face missing")?;
    let file_id = ids
        .into_iter()
        .find(|id| matches!(database.face_source(*id), Some((Source::File(_), _))))
        .ok_or("file font face missing")?;
    Ok(FileDatabase {
        database,
        base_id,
        file_id,
        file: TestFontFile(path),
    })
}

#[test]
fn unavailable_generation_rejects_global_cmap_storage() -> Result<(), String> {
    const CASE: &str = "markdown::svg_rasterize::text_shaping::usvg_cmap::storage::tests::generation_tests::unavailable_generation_rejects_global_cmap_storage";
    if !super::global_child::run_parent_or_allow_child(CASE)? {
        return Ok(());
    }
    case_bodies::unavailable_generation_rejects_global_cmap_storage()
}

fn assert_legacy_entry_is_removed(
    database: &Arc<Database>,
    id: ID,
    generation: FontSourceGeneration,
) -> Result<(), String> {
    cache()
        .lock()
        .map_err(|_| "cache lock failed")?
        .entries
        .push_back(entry(
            database,
            id,
            generation.clone(),
            Arc::from([1_u8, 2, 3]),
        )?);
    assert!(
        lookup(database, id, 0, &generation)
            .map_err(|_| "cache lookup failed")?
            .is_none()
    );
    assert!(
        cache()
            .lock()
            .map_err(|_| "cache lock failed")?
            .entries
            .is_empty()
    );
    Ok(())
}

fn assert_insert_is_not_persisted(
    database: &Arc<Database>,
    id: ID,
    generation: FontSourceGeneration,
) -> Result<(), String> {
    assert!(insert(
        database,
        id,
        0,
        generation,
        vec![1, 2, 3],
        mapping()?
    ));
    assert!(
        cache()
            .lock()
            .map_err(|_| "cache lock failed")?
            .entries
            .is_empty()
    );
    Ok(())
}

#[cfg(windows)]
#[test]
fn weak_file_generation_never_enters_or_reuses_global_cmap_storage() -> Result<(), String> {
    const CASE: &str = "markdown::svg_rasterize::text_shaping::usvg_cmap::storage::tests::generation_tests::weak_file_generation_never_enters_or_reuses_global_cmap_storage";
    if !super::global_child::run_parent_or_allow_child(CASE)? {
        return Ok(());
    }
    case_bodies::weak_file_generation_never_enters_or_reuses_global_cmap_storage()
}

#[test]
fn same_length_same_modified_rewrite_reprobes_without_stale_cmap_result() -> Result<(), String> {
    const CASE: &str = "markdown::svg_rasterize::text_shaping::usvg_cmap::storage::tests::generation_tests::same_length_same_modified_rewrite_reprobes_without_stale_cmap_result";
    if !super::global_child::run_parent_or_allow_child(CASE)? {
        return Ok(());
    }
    case_bodies::same_length_same_modified_rewrite_reprobes_without_stale_cmap_result()
}

fn verify_rewrite_reprobes(fixture: &FileDatabase, replacement: &[u8]) -> Result<(), String> {
    let before = generation(&fixture.database, fixture.file_id);
    assert_eq!(
        before.durable_reusable(),
        super::super::super::super::font::file_stamp_durable_reusable(&fixture.file.0)
    );
    assert_probe_policy(&fixture.database, fixture.file_id, true);
    assert_selector_matches_stock(&fixture.database, fixture.base_id, Some(fixture.file_id));
    rewrite_same_modified_time(&fixture.file.0, replacement)?;
    #[cfg(windows)]
    assert_eq!(before, generation(&fixture.database, fixture.file_id));
    assert_probe_policy(&fixture.database, fixture.file_id, false);
    assert_selector_matches_stock(&fixture.database, fixture.base_id, None);
    Ok(())
}

fn assert_probe_policy(database: &Arc<Database>, id: ID, _supported: bool) {
    #[cfg(windows)]
    {
        assert_eq!(
            probe_character(database, id, 'A'),
            Err("file cmap probe failed".into())
        );
        assert_global_cache_empty();
    }
    #[cfg(not(windows))]
    assert_eq!(probe_character(database, id, 'A'), Ok(_supported));
}

#[cfg(windows)]
fn assert_global_cache_empty() {
    assert!(cache().lock().is_ok_and(|cache| cache.entries.is_empty()));
}

fn assert_selector_matches_stock(database: &Arc<Database>, base_id: ID, expected: Option<ID>) {
    let excluded = [base_id];
    let stock = resvg::usvg::FontResolver::default_fallback_selector();
    let optimized = super::super::super::selector::html_selector();
    let (stock_result, optimized_result) =
        super::super::super::super::with_validated_tree_parse(|| {
            (
                stock('A', &excluded, &mut Arc::clone(database)),
                optimized('A', &excluded, &mut Arc::clone(database)),
            )
        });
    assert_eq!((stock_result, optimized_result), (expected, expected));
    #[cfg(windows)]
    assert_global_cache_empty();
}

fn probe_character(database: &Arc<Database>, id: ID, character: char) -> Result<bool, String> {
    super::super::super::super::with_validated_tree_parse(|| {
        super::super::super::probe::has_char(database, id, character)
    })
    .map_err(|_| "file cmap probe failed".to_string())
}

fn rewrite_same_modified_time(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    let original_len = std::fs::metadata(path)
        .map_err(|error| error.to_string())?
        .len();
    if u64::try_from(bytes.len()).map_err(|error| error.to_string())? != original_len {
        return Err("rewrite changed font length".into());
    }
    rewrite_with_same_modified_time(path, bytes)
}

fn cmap_directory(font: &[u8]) -> Result<(usize, usize, usize), String> {
    let mut cmap = None;
    let mut head = None;
    let table_count = usize::from(u16::from_be_bytes([font[4], font[5]]));
    for index in 0..table_count {
        let record = 12 + index * 16;
        let tag = font
            .get(record..record + 4)
            .ok_or("truncated sfnt directory")?;
        let offset = u32::from_be_bytes(
            font.get(record + 8..record + 12)
                .ok_or("truncated table record")?
                .try_into()
                .map_err(|_| "invalid table offset")?,
        ) as usize;
        if tag == b"cmap" {
            cmap = Some((record, offset));
        } else if tag == b"head" {
            head = Some(offset);
        }
    }
    let (record, offset) = cmap.ok_or("cmap table missing")?;
    Ok((record, offset, head.ok_or("head table missing")?))
}

#[path = "svg_rasterize_text_usvg_cmap_storage_generation_case_bodies.rs"]
mod case_bodies;
