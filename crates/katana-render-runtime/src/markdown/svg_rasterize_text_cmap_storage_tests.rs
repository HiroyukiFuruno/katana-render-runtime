use super::{Cache, MAX_CACHE_BYTES, MAX_CACHE_ENTRIES, MAX_ENTRY_BYTES};
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
    let mut face = bundled_font_db()
        .faces()
        .next()
        .ok_or("bundled face missing")?
        .clone();
    let path = std::env::temp_dir().join(format!(
        "krr-cmap-storage-{}-{}.ttf",
        std::process::id(),
        TEMP_FILE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, bytes).map_err(|error| error.to_string())?;
    face.source = Source::File(path.clone());
    let mut database = Database::new();
    let id = database.push_face_info(face);
    Ok((Arc::new(database), id, TempFile(path)))
}
fn bundled_bytes() -> Result<Vec<u8>, String> {
    let db = bundled_font_db();
    let face = db.faces().next().ok_or("bundled face missing")?;
    db.with_face_data(face.id, |data, _| data.to_vec())
        .ok_or_else(|| "bundled bytes missing".into())
}

#[test]
fn cache_rejects_an_oversized_cmap_entry() -> Result<(), String> {
    let bytes = bundled_bytes()?;
    let (database, id, _guard) = file_database(&bytes)?;
    let generation = super::super::super::file_generation::file_source_stamp(&database, id).2;
    let mut cache = Cache::default();
    cache.insert(
        &database,
        id,
        0,
        generation,
        Arc::from(vec![0; MAX_ENTRY_BYTES + 1]),
    );
    assert!(cache.entries.is_empty());
    assert_eq!(cache.retained_bytes, 0);
    Ok(())
}

#[test]
fn cache_enforces_entry_and_byte_limits() -> Result<(), String> {
    let bytes = bundled_bytes()?;
    let (database, first_id, _guard) = file_database(&bytes)?;
    let generation = super::super::super::file_generation::file_source_stamp(&database, first_id).2;
    let mut cache = Cache::default();
    let face = database.face(first_id).ok_or("face missing")?.clone();
    let mut ids = Vec::with_capacity(MAX_CACHE_ENTRIES + 1);
    let mut database = (*database).clone();
    for _ in 0..=MAX_CACHE_ENTRIES {
        ids.push(database.push_face_info(face.clone()));
    }
    let database = Arc::new(database);
    let entry = Arc::from(vec![0; MAX_ENTRY_BYTES]);
    for id in ids {
        cache.insert(&database, id, 0, generation.clone(), Arc::clone(&entry));
    }
    assert!(cache.entries.len() <= MAX_CACHE_ENTRIES);
    assert!(cache.retained_bytes <= MAX_CACHE_BYTES);
    Ok(())
}

#[test]
fn cache_purges_dropped_databases_and_removes_a_face() -> Result<(), String> {
    let bytes = bundled_bytes()?;
    let (database, id, _guard) = file_database(&bytes)?;
    let generation = super::super::super::file_generation::file_source_stamp(&database, id).2;
    let mut cache = Cache::default();
    cache.insert(&database, id, 0, generation, Arc::from([1_u8, 2, 3]));
    cache.remove_face(&database, id);
    assert!(cache.entries.is_empty());
    cache.insert(
        &database,
        id,
        0,
        super::super::super::file_generation::file_source_stamp(&database, id).2,
        Arc::from([1_u8]),
    );
    drop(database);
    let (other, other_id, _other_guard) = file_database(&bytes)?;
    let generation = super::super::super::file_generation::file_source_stamp(&other, other_id).2;
    assert!(cache.lookup(&other, other_id, 0, &generation).is_none());
    assert!(cache.entries.is_empty());
    Ok(())
}
