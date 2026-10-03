use super::{ProbeResult, probe_file};
use crate::markdown::svg_rasterize::font::bundled_font_db;
use resvg::usvg::fontdb::{Database, Source};
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

static TEMP_FILE: AtomicUsize = AtomicUsize::new(0);

struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn file_database(
    bytes: &[u8],
) -> Result<(Arc<Database>, resvg::usvg::fontdb::ID, TempFile), String> {
    let face = bundled_font_db()
        .faces()
        .next()
        .ok_or("bundled face missing")?
        .clone();
    let source = std::env::temp_dir().join(format!(
        "krr-cmap-cache-{}-{}.ttf",
        std::process::id(),
        TEMP_FILE.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::write(&source, bytes).map_err(|error| error.to_string())?;
    let mut face = face;
    face.source = Source::File(source.clone());
    let mut database = Database::new();
    let id = database.push_face_info(face);
    Ok((Arc::new(database), id, TempFile(source)))
}

fn bundled_bytes() -> Result<Vec<u8>, String> {
    let database = bundled_font_db();
    let face = database.faces().next().ok_or("bundled face missing")?;
    database
        .with_face_data(face.id, |data, _| data.to_vec())
        .ok_or_else(|| "bundled bytes missing".to_string())
}

#[test]
fn file_cmap_matches_ttf_parser_for_positive_and_negative_characters() -> Result<(), String> {
    let bytes = bundled_bytes()?;
    let (database, id, _guard) = file_database(&bytes)?;
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
    let (database, id, _guard) = file_database(&[0; 32])?;
    let generation = super::super::file_generation::file_source_stamp(&database, id).2;
    assert!(matches!(
        probe_file(&database, id, &generation, 'A'),
        ProbeResult::Complete(None)
    ));
    Ok(())
}

#[test]
fn changed_file_generation_does_not_reuse_old_cmap() -> Result<(), String> {
    let bytes = bundled_bytes()?;
    let (database, id, guard) = file_database(&bytes)?;
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
    let bytes = bundled_bytes()?;
    let (database, id, guard) = file_database(&bytes)?;
    std::fs::remove_file(&guard.0).map_err(|error| error.to_string())?;
    let generation = super::super::file_generation::file_source_stamp(&database, id).2;
    assert!(matches!(
        probe_file(&database, id, &generation, 'A'),
        ProbeResult::UseOriginal
    ));
    Ok(())
}

#[cfg(windows)]
#[test]
fn same_length_same_modified_file_rewrite_does_not_reuse_cached_cmap() -> Result<(), String> {
    let original = bundled_bytes()?;
    let replacement = font_with_only_b_cmap(&original)?;
    let (database, id, guard) = file_database(&original)?;
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
fn rewrite_with_same_modified_time(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    let modified = std::fs::metadata(path)
        .map_err(|error| error.to_string())?
        .modified()
        .map_err(|error| error.to_string())?;
    std::fs::write(path, bytes).map_err(|error| error.to_string())?;
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(|error| error.to_string())?
        .set_times(std::fs::FileTimes::new().set_modified(modified))
        .map_err(|error| error.to_string())
}

#[cfg(windows)]
fn font_with_only_b_cmap(font: &[u8]) -> Result<Vec<u8>, String> {
    let (record, offset, head) = cmap_directory(font)?;
    let glyph = rustybuzz::ttf_parser::Face::parse(font, 0)
        .map_err(|_| "font invalid")?
        .glyph_index('B')
        .ok_or("B glyph missing")?
        .0;
    rewrite_cmap(font, record, offset, head, b_cmap(glyph))
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

#[cfg(windows)]
fn rewrite_cmap(
    font: &[u8],
    record: usize,
    offset: usize,
    head: usize,
    cmap: Vec<u8>,
) -> Result<Vec<u8>, String> {
    let mut changed = font.to_vec();
    changed[offset..offset + cmap.len()].copy_from_slice(&cmap);
    changed[record + 4..record + 8].copy_from_slice(&sfnt_checksum(&cmap).to_be_bytes());
    let cmap_len = u32::try_from(cmap.len())
        .map_err(|_| "cmap too large")?
        .to_be_bytes();
    changed[record + 12..record + 16].copy_from_slice(&cmap_len);
    changed[head + 8..head + 12].fill(0);
    let adjustment = 0xB1B0_AFBA_u32.wrapping_sub(sfnt_checksum(&changed));
    changed[head + 8..head + 12].copy_from_slice(&adjustment.to_be_bytes());
    if changed.len() != font.len() {
        return Err("cmap rewrite changed font length".into());
    }
    Ok(changed)
}

#[cfg(windows)]
fn b_cmap(glyph: u16) -> Vec<u8> {
    let mut cmap = Vec::with_capacity(40);
    cmap.extend_from_slice(&[0, 0, 0, 1, 0, 3, 0, 10, 0, 0, 0, 12]);
    cmap.extend_from_slice(&12_u16.to_be_bytes());
    cmap.extend_from_slice(&0_u16.to_be_bytes());
    cmap.extend_from_slice(&28_u32.to_be_bytes());
    cmap.extend_from_slice(&0_u32.to_be_bytes());
    cmap.extend_from_slice(&1_u32.to_be_bytes());
    cmap.extend_from_slice(&u32::from('B').to_be_bytes());
    cmap.extend_from_slice(&u32::from('B').to_be_bytes());
    cmap.extend_from_slice(&u32::from(glyph).to_be_bytes());
    cmap
}

#[cfg(windows)]
fn sfnt_checksum(bytes: &[u8]) -> u32 {
    bytes.chunks(4).fold(0_u32, |sum, chunk| {
        let mut word = [0; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}
