use super::super::{MAX_FILE_STAMPS, cached_font_has_char, file_source_stamp, insert, lookup};
use crate::markdown::svg_rasterize::font::bundled_font_db;
use resvg::usvg::fontdb::{Database, Source};
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

static TEMP_FONT_COUNTER: AtomicUsize = AtomicUsize::new(0);
type TestId = resvg::usvg::fontdb::ID;
type TestFace = resvg::usvg::fontdb::FaceInfo;
type FileFixture = (
    Arc<Database>,
    TestId,
    TestId,
    PathBuf,
    TempFontFile,
    Vec<u8>,
);
type RecoveryFixture = (Arc<Database>, TestId, PathBuf, TempFontFile, Vec<u8>);
type StampCacheFixture = (Vec<super::super::GlyphCacheEntry>, Arc<Database>);
type FontFixture = (TestFace, Vec<u8>, Vec<u8>);
type FixtureResult<T> = Result<T, Box<dyn std::error::Error>>;

struct TempFontFile(PathBuf);

impl Drop for TempFontFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn changed_file_source_rechecks_glyphs_for_a_new_style() -> Result<(), Box<dyn std::error::Error>> {
    use super::super::super::super::fallback::html_font_runs;
    let (database, file_id, fallback_id, path, _guard, replacement) = file_fixture("refresh")?;

    assert_eq!(
        html_font_runs(&database, file_id, "A", 400, false),
        [(file_id, "A".into())]
    );

    write_file_with_new_modified_time(&path, &replacement)?;
    assert_eq!(
        html_font_runs(&database, file_id, "A", 700, true),
        [(fallback_id, "A".into())]
    );
    assert_eq!(cached_font_has_char(&database, file_id, 'A'), Some(false));
    Ok(())
}

#[test]
fn missing_file_expires_glyphs_and_valid_replacement_recovers()
-> Result<(), Box<dyn std::error::Error>> {
    let (database, id, path, _guard, replacement) = file_recovery_fixture()?;

    assert_eq!(cached_font_has_char(&database, id, 'A'), Some(true));
    std::fs::remove_file(&path)?;
    assert_eq!(cached_font_has_char(&database, id, 'A'), None);
    std::fs::write(&path, replacement)?;
    assert_eq!(cached_font_has_char(&database, id, 'A'), Some(false));
    Ok(())
}

#[test]
fn unavailable_file_restored_after_stamp_is_rechecked_without_caching()
-> Result<(), Box<dyn std::error::Error>> {
    let (database, id, path, guard, replacement) = file_recovery_fixture()?;
    let restore = std::fs::read(&path)?;
    std::fs::remove_file(&path)?;
    let restore_path = path.clone();
    let restored = std::rc::Rc::new(std::cell::RefCell::new(None));
    let restore_result = restored.clone();
    super::super::set_after_file_stamp_for_test(move || {
        *restore_result.borrow_mut() = Some(std::fs::write(&restore_path, &restore));
    });
    assert_eq!(cached_font_has_char(&database, id, 'A'), Some(true));
    restored
        .borrow_mut()
        .take()
        .ok_or("restore hook did not run")??;
    super::super::GLYPH_CACHE.with(|entries| {
        assert!(
            !entries
                .borrow()
                .iter()
                .any(|entry| super::super::same_database(entry, &database))
        );
    });
    std::fs::write(&path, replacement)?;
    assert_eq!(cached_font_has_char(&database, id, 'A'), Some(false));
    drop(guard);
    Ok(())
}

#[test]
fn file_stamp_capacity_evicts_the_matching_glyphs() -> Result<(), Box<dyn std::error::Error>> {
    let (entries, _database) = full_stamp_cache()?;
    assert_eq!(entries[0].file_stamps.len(), MAX_FILE_STAMPS);
    assert_eq!(entries[0].glyphs.len(), MAX_FILE_STAMPS);
    assert!(
        entries[0]
            .glyphs
            .keys()
            .all(|(id, _)| entries[0].file_stamps.contains_key(id))
    );
    Ok(())
}

#[test]
fn missing_file_stamp_removes_only_that_face_cache() -> Result<(), Box<dyn std::error::Error>> {
    let (database, file_id, binary_id, _path, _guard, _) = file_fixture("missing-stamp")?;
    let (is_file, stamp, _) = file_source_stamp(&database, file_id);
    assert!(is_file);
    let stamp = stamp.ok_or("file stamp missing")?;
    let mut entries = Vec::new();
    insert(&mut entries, &database, (file_id, 'A'), true, Some(stamp));
    insert(&mut entries, &database, (binary_id, 'A'), true, None);

    assert_eq!(
        lookup(&mut entries, &database, (file_id, 'A'), true, None),
        None
    );
    assert!(entries[0].glyphs.keys().all(|(id, _)| *id != file_id));
    assert!(!entries[0].file_stamps.contains_key(&file_id));
    assert_eq!(
        lookup(&mut entries, &database, (binary_id, 'A'), false, None),
        Some(true)
    );
    Ok(())
}

fn file_fixture(prefix: &str) -> FixtureResult<FileFixture> {
    let (face, original, replacement) = fixture_font()?;
    let (path, guard) = temp_font(prefix, &original)?;
    let (database, file_id, fallback_id) = file_and_binary_database(face, original, &path);
    Ok((database, file_id, fallback_id, path, guard, replacement))
}

fn write_file_with_new_modified_time(path: &std::path::Path, bytes: &[u8]) -> FixtureResult<()> {
    let before = std::fs::metadata(path)?;
    let replacement_len = u64::try_from(bytes.len())?;
    assert_eq!(before.len(), replacement_len);
    let before_modified = before.modified()?;
    std::fs::write(path, bytes)?;
    let modified = before_modified + std::time::Duration::from_secs(1);
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)?
        .set_times(std::fs::FileTimes::new().set_modified(modified))?;
    assert_ne!(std::fs::metadata(path)?.modified()?, before_modified);
    Ok(())
}

fn file_and_binary_database(
    face: TestFace,
    original: Vec<u8>,
    path: &std::path::Path,
) -> (Arc<Database>, TestId, TestId) {
    let mut file_face = face.clone();
    file_face.source = Source::File(path.to_path_buf());
    let mut database = Database::new();
    let file_id = database.push_face_info(file_face);
    let mut binary_face = face;
    binary_face.source = Source::Binary(Arc::new(original));
    let fallback_id = database.push_face_info(binary_face);
    (Arc::new(database), file_id, fallback_id)
}

fn file_recovery_fixture() -> FixtureResult<RecoveryFixture> {
    let (face, original, replacement) = fixture_font()?;
    let (path, guard) = temp_font("recovery", &original)?;
    let mut file_face = face;
    file_face.source = Source::File(path.clone());
    let mut database = Database::new();
    let id = database.push_face_info(file_face);
    Ok((Arc::new(database), id, path, guard, replacement))
}

fn full_stamp_cache() -> FixtureResult<StampCacheFixture> {
    let (path, _guard) = temp_font("stamps", b"file stamp fixture")?;
    let mut face = bundled_font_db()
        .faces()
        .next()
        .ok_or("bundled face missing")?
        .clone();
    face.source = Source::File(path);
    let mut database = Database::new();
    let ids = (0..=MAX_FILE_STAMPS)
        .map(|_| database.push_face_info(face.clone()))
        .collect::<Vec<_>>();
    let database = Arc::new(database);
    let mut entries = Vec::new();
    for id in ids {
        let stamp = file_source_stamp(&database, id)
            .1
            .ok_or("file stamp missing")?;
        insert(&mut entries, &database, (id, 'A'), true, Some(stamp));
    }
    Ok((entries, database))
}

fn font_with_only_b_cmap(font: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let cmap = single_b_cmap(font)?;
    let (cmap_record, cmap_offset) = table_record(font, b"cmap")?;
    let (_, head_offset) = table_record(font, b"head")?;
    let mut changed = font.to_vec();
    changed[cmap_offset..cmap_offset + cmap.len()].copy_from_slice(&cmap);
    changed[cmap_record + 4..cmap_record + 8].copy_from_slice(&sfnt_checksum(&cmap).to_be_bytes());
    changed[cmap_record + 12..cmap_record + 16].copy_from_slice(&(cmap.len() as u32).to_be_bytes());
    changed[head_offset + 8..head_offset + 12].fill(0);
    let adjustment = 0xB1B0_AFBA_u32.wrapping_sub(sfnt_checksum(&changed));
    changed[head_offset + 8..head_offset + 12].copy_from_slice(&adjustment.to_be_bytes());
    Ok(changed)
}

fn single_b_cmap(font: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let glyph_id = rustybuzz::ttf_parser::Face::parse(font, 0)
        .map_err(|_| "bundled font could not be parsed")?
        .glyph_index('B')
        .ok_or("bundled font has no B glyph")?
        .0;
    let mut cmap = Vec::with_capacity(40);
    cmap.extend_from_slice(&[0, 0, 0, 1, 0, 3, 0, 10, 0, 0, 0, 12]);
    cmap.extend_from_slice(&12_u16.to_be_bytes());
    cmap.extend_from_slice(&0_u16.to_be_bytes());
    cmap.extend_from_slice(&28_u32.to_be_bytes());
    cmap.extend_from_slice(&0_u32.to_be_bytes());
    cmap.extend_from_slice(&1_u32.to_be_bytes());
    cmap.extend_from_slice(&u32::from('B').to_be_bytes());
    cmap.extend_from_slice(&u32::from('B').to_be_bytes());
    cmap.extend_from_slice(&u32::from(glyph_id).to_be_bytes());
    Ok(cmap)
}

fn table_record(font: &[u8], tag: &[u8; 4]) -> Result<(usize, usize), Box<dyn std::error::Error>> {
    let table_count = u16::from_be_bytes(font[4..6].try_into()?) as usize;
    for index in 0..table_count {
        let record = 12 + index * 16;
        let current_tag = &font[record..record + 4];
        let offset = u32::from_be_bytes(font[record + 8..record + 12].try_into()?) as usize;
        if current_tag == tag {
            return Ok((record, offset));
        }
    }
    Err("font table missing".into())
}

fn fixture_font() -> FixtureResult<FontFixture> {
    let bundled = bundled_font_db();
    let face = bundled
        .faces()
        .next()
        .ok_or("bundled face missing")?
        .clone();
    let original = bundled
        .with_face_data(face.id, |data, _| data.to_vec())
        .ok_or("bundled font bytes missing")?;
    let replacement = font_with_only_b_cmap(&original)?;
    validate_cmap_fixture(&original, &replacement)?;
    Ok((face, original, replacement))
}

fn validate_cmap_fixture(original: &[u8], replacement: &[u8]) -> Result<(), &'static str> {
    let original =
        rustybuzz::ttf_parser::Face::parse(original, 0).map_err(|_| "invalid original font")?;
    let replacement = rustybuzz::ttf_parser::Face::parse(replacement, 0)
        .map_err(|_| "invalid replacement font")?;
    assert!(original.glyph_index('A').is_some());
    assert!(replacement.glyph_index('A').is_none());
    assert_eq!(replacement.glyph_index('B'), original.glyph_index('B'));
    Ok(())
}

fn temp_font(prefix: &str, bytes: &[u8]) -> Result<(PathBuf, TempFontFile), std::io::Error> {
    let path = std::env::temp_dir().join(format!(
        "krr-glyph-file-{prefix}-{}-{}.ttf",
        std::process::id(),
        TEMP_FONT_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, bytes)?;
    Ok((path.clone(), TempFontFile(path)))
}

fn sfnt_checksum(bytes: &[u8]) -> u32 {
    bytes.chunks(4).fold(0_u32, |sum, chunk| {
        let mut word = [0; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}

#[path = "svg_rasterize_text_glyph_cache_generation_tests.rs"]
mod generation_tests;
