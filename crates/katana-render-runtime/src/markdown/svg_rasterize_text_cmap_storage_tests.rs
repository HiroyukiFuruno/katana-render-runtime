use super::super::file_test_support::CmapFileFixture;
use super::{Cache, MAX_CACHE_BYTES, MAX_CACHE_ENTRIES, MAX_ENTRY_BYTES};
use std::sync::{Arc, atomic::AtomicUsize};

static TEMP_FILE: AtomicUsize = AtomicUsize::new(0);
const FILES: CmapFileFixture = CmapFileFixture::new("krr-cmap-storage", &TEMP_FILE);

#[test]
fn file_backed_cmap_fixture_rejects_an_invalid_destination() {
    let files = CmapFileFixture::new("krr-cmap-invalid\0path", &TEMP_FILE);
    assert!(files.bundled_database().is_err());
}

#[test]
fn cache_rejects_an_oversized_cmap_entry() -> Result<(), String> {
    let (database, id, _guard) = FILES.bundled_database()?;
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
    let (database, first_id, _guard) = FILES.bundled_database()?;
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
    let (database, id, _guard) = FILES.bundled_database()?;
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
    let (other, other_id, _other_guard) = FILES.bundled_database()?;
    let generation = super::super::super::file_generation::file_source_stamp(&other, other_id).2;
    assert!(cache.lookup(&other, other_id, 0, &generation).is_none());
    assert!(cache.entries.is_empty());
    Ok(())
}
