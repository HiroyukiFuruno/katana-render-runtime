use super::super::font::FontSourceGeneration;
use resvg::usvg::fontdb::{Database, ID};
use skrifa::charmap::MappingIndex;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, OnceLock, Weak},
};

const MAX_CACHE_BYTES: usize = 8 * 1024 * 1024;
const MAX_ENTRIES: usize = 1024;

pub(super) struct CachedCmap {
    pub(super) cmap_sfnt: Arc<[u8]>,
    pub(super) mapping: MappingIndex,
}

struct Entry {
    database: Weak<Database>,
    face_id: ID,
    face_index: u32,
    generation: FontSourceGeneration,
    cmap_sfnt: Arc<[u8]>,
    mapping: MappingIndex,
}

#[derive(Default)]
struct Cache {
    entries: VecDeque<Entry>,
    retained_bytes: usize,
}

static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();

pub(super) fn lookup(
    database: &Arc<Database>,
    face_id: ID,
    face_index: u32,
    generation: &FontSourceGeneration,
) -> Result<Option<CachedCmap>, ()> {
    let mut cache = cache().lock().map_err(|_| ())?;
    purge_dead(&mut cache);
    Ok(cache.entries.iter().find_map(|entry| {
        entry_matches(entry, database, face_id, face_index, generation).then(|| CachedCmap {
            cmap_sfnt: Arc::clone(&entry.cmap_sfnt),
            mapping: entry.mapping,
        })
    }))
}

pub(super) fn insert(
    database: &Arc<Database>,
    face_id: ID,
    face_index: u32,
    generation: FontSourceGeneration,
    cmap_sfnt: Vec<u8>,
    mapping: MappingIndex,
) -> bool {
    if !entry_fits(cmap_sfnt.len()) {
        return false;
    }
    let Ok(mut cache) = cache().lock() else {
        return false;
    };
    purge_dead(&mut cache);
    remove_matching(&mut cache, database, face_id, face_index);
    reserve_capacity(&mut cache, cmap_sfnt.len());
    let cmap_sfnt: Arc<[u8]> = Arc::from(cmap_sfnt);
    cache.retained_bytes += cmap_sfnt.len();
    cache.entries.push_back(Entry {
        database: Arc::downgrade(database),
        face_id,
        face_index,
        generation,
        cmap_sfnt,
        mapping,
    });
    true
}

pub(super) fn remove_face(database: &Arc<Database>, face_id: ID, face_index: u32) {
    let Ok(mut cache) = cache().lock() else {
        return;
    };
    cache.entries.retain(|entry| {
        !(entry.face_id == face_id
            && entry.face_index == face_index
            && entry
                .database
                .upgrade()
                .is_some_and(|cached| Arc::ptr_eq(&cached, database)))
    });
    recompute_bytes(&mut cache);
}

fn entry_matches(
    entry: &Entry,
    database: &Arc<Database>,
    face_id: ID,
    face_index: u32,
    generation: &FontSourceGeneration,
) -> bool {
    entry.face_id == face_id
        && entry.face_index == face_index
        && &entry.generation == generation
        && entry
            .database
            .upgrade()
            .is_some_and(|cached| Arc::ptr_eq(&cached, database))
}

fn remove_matching(cache: &mut Cache, database: &Arc<Database>, face_id: ID, face_index: u32) {
    cache.entries.retain(|entry| {
        !(entry.face_id == face_id
            && entry.face_index == face_index
            && entry
                .database
                .upgrade()
                .is_some_and(|cached| Arc::ptr_eq(&cached, database)))
    });
    recompute_bytes(cache);
}

fn reserve_capacity(cache: &mut Cache, bytes: usize) {
    while let Some(oldest) = cache.entries.front() {
        if cache.entries.len() < MAX_ENTRIES
            && cache.retained_bytes.saturating_add(bytes) <= MAX_CACHE_BYTES
        {
            break;
        }
        let released = oldest.cmap_sfnt.len();
        let _ = cache.entries.pop_front();
        cache.retained_bytes = cache.retained_bytes.saturating_sub(released);
    }
}

fn entry_fits(bytes: usize) -> bool {
    bytes <= super::MAX_ENTRY_BYTES
}

fn purge_dead(cache: &mut Cache) {
    cache
        .entries
        .retain(|entry| entry.database.strong_count() > 0);
    recompute_bytes(cache);
}

fn recompute_bytes(cache: &mut Cache) {
    cache.retained_bytes = cache
        .entries
        .iter()
        .map(|entry| entry.cmap_sfnt.len())
        .sum();
}

fn cache() -> &'static Mutex<Cache> {
    CACHE.get_or_init(|| Mutex::new(Cache::default()))
}

#[cfg(test)]
#[path = "svg_rasterize_text_usvg_cmap_storage_tests.rs"]
pub(super) mod tests;
