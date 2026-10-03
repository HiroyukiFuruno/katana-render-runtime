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
