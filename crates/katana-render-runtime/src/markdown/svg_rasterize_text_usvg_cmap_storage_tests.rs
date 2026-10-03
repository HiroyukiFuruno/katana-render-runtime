use super::{
    Cache, Entry, MAX_CACHE_BYTES, MAX_ENTRIES, cache, entry_matches, insert, lookup, purge_dead,
    remove_face, remove_matching, reserve_capacity,
};
use resvg::usvg::fontdb::{Database, ID};
use skrifa::{FontRef, charmap::MappingIndex};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub(in super::super) struct CacheResetOnDrop;
impl Drop for CacheResetOnDrop {
    fn drop(&mut self) {
        let shared = cache();
        shared.clear_poison();
        match shared.lock() {
            Ok(mut guard) => *guard = Cache::default(),
            Err(poisoned) => *poisoned.into_inner() = Cache::default(),
        }
        shared.clear_poison();
    }
}

pub(in super::super) fn begin() -> Result<MutexGuard<'static, ()>, String> {
    let lock = TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "test lock poisoned")?;
    let shared = cache();
    shared.clear_poison();
    let mut guard = shared.lock().map_err(|_| "cache lock poisoned")?;
    *guard = Cache::default();
    drop(guard);
    Ok(lock)
}

fn database() -> Result<(Arc<Database>, ID), String> {
    let mut database = Database::new();
    database.load_font_data(FONT_BYTES.to_vec());
    let database = Arc::new(database);
    let id = database.faces().next().ok_or("font face missing")?.id;
    Ok((database, id))
}

fn mapping() -> Result<MappingIndex, String> {
    let font = FontRef::new(FONT_BYTES).map_err(|error| format!("font parse: {error:?}"))?;
    Ok(MappingIndex::new(&font))
}

fn generation(database: &Arc<Database>, id: ID) -> super::super::super::font::FontSourceGeneration {
    super::super::super::font::font_source_generation(database, id)
}

fn entry(
    database: &Arc<Database>,
    id: ID,
    generation: super::super::super::font::FontSourceGeneration,
    bytes: Arc<[u8]>,
) -> Result<Entry, String> {
    Ok(Entry {
        database: Arc::downgrade(database),
        face_id: id,
        face_index: 0,
        generation,
        cmap_sfnt: bytes,
        mapping: mapping()?,
    })
}

#[test]
fn local_cache_replacement_and_generation_match_are_exact() -> Result<(), String> {
    let (database, id) = database()?;
    let generation = generation(&database, id);
    let mut cache = Cache::default();
    cache.entries.push_back(entry(
        &database,
        id,
        generation.clone(),
        Arc::from([1_u8, 2, 3]),
    )?);
    remove_matching(&mut cache, &database, id, 0);
    assert!(cache.entries.is_empty());
    cache.entries.push_back(entry(
        &database,
        id,
        generation.clone(),
        Arc::from([4_u8, 5]),
    )?);
    assert!(entry_matches(
        &cache.entries[0],
        &database,
        id,
        0,
        &generation
    ));
    assert_eq!(cache.entries[0].cmap_sfnt.len(), 2);
    Ok(())
}

#[test]
fn local_cache_fifo_and_byte_limits_are_bounded() -> Result<(), String> {
    let (database, id) = database()?;
    let generation = generation(&database, id);
    let bytes = Arc::from(vec![0_u8; MAX_CACHE_BYTES / MAX_ENTRIES]);
    let mut cache = Cache::default();
    for index in 0..MAX_ENTRIES {
        let entry_id = if index == 0 { id } else { ID::dummy() };
        cache.entries.push_back(entry(
            &database,
            entry_id,
            generation.clone(),
            Arc::clone(&bytes),
        )?);
    }
    cache.retained_bytes = MAX_CACHE_BYTES;
    reserve_capacity(&mut cache, bytes.len());
    assert_eq!(cache.entries.len(), MAX_ENTRIES - 1);
    assert!(cache.retained_bytes <= MAX_CACHE_BYTES - bytes.len());
    Ok(())
}

#[test]
fn local_cache_purges_dropped_database() -> Result<(), String> {
    let (database, id) = database()?;
    let generation = generation(&database, id);
    let mut cache = Cache::default();
    cache
        .entries
        .push_back(entry(&database, id, generation, Arc::from([1_u8]))?);
    drop(database);
    purge_dead(&mut cache);
    assert!(cache.entries.is_empty());
    assert_eq!(cache.retained_bytes, 0);
    Ok(())
}

#[test]
fn poisoned_global_cache_fails_closed_and_raii_restores_it() -> Result<(), String> {
    let _lock = begin()?;
    let _reset = CacheResetOnDrop;
    poison_global_cache()?;
    let (database, id) = database()?;
    let generation = generation(&database, id);
    assert!(lookup(&database, id, 0, &generation).is_err());
    assert!(!insert(&database, id, 0, generation, vec![0], mapping()?));
    remove_face(&database, id, 0);
    Ok(())
}

pub(in super::super) fn poison_global_cache() -> Result<(), String> {
    let guard = cache().lock().map_err(|_| "cache already poisoned")?;
    let poisoned = catch_unwind(AssertUnwindSafe(move || {
        let _keep_lock = guard;
        resume_unwind(Box::new(()));
    }));
    assert!(poisoned.is_err());
    Ok(())
}

#[test]
fn global_cache_accepts_exact_entry_bound_and_rejects_excess() -> Result<(), String> {
    let _lock = begin()?;
    let _reset = CacheResetOnDrop;
    let (database, id) = database()?;
    let generation = generation(&database, id);
    let limit = super::super::MAX_ENTRY_BYTES;
    assert!(!insert(
        &database,
        id,
        0,
        generation.clone(),
        vec![0; limit + 1],
        mapping()?
    ));
    assert!(insert(
        &database,
        id,
        0,
        generation.clone(),
        vec![0; limit],
        mapping()?
    ));
    let retained = lookup(&database, id, 0, &generation)
        .map_err(|_| "cache lock poisoned")?
        .ok_or("exact-bound entry absent")?;
    assert_eq!(retained.cmap_sfnt.len(), limit);
    Ok(())
}
