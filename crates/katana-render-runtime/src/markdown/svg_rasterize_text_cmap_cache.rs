use super::file_generation::{FontSourceGeneration, file_source_stamp};
use resvg::usvg::fontdb::{Database, ID, Source};
use rustybuzz::ttf_parser::{Face, Tag, cmap};
use std::sync::{Arc, Mutex, OnceLock};

const CMAP_TAG: Tag = Tag::from_bytes(b"cmap");

pub(super) enum ProbeResult {
    Complete(Option<bool>),
    UseOriginal,
}

#[path = "svg_rasterize_text_cmap_storage.rs"]
mod storage;
use storage::{Cache, MAX_ENTRY_BYTES};

pub(super) fn probe_file(
    database: &Arc<Database>,
    face_id: ID,
    generation: &FontSourceGeneration,
    character: char,
) -> ProbeResult {
    if !generation.is_file() || !generation.reusable() {
        return ProbeResult::UseOriginal;
    }
    let Some((Source::File(_), face_index)) = database.face_source(face_id) else {
        return ProbeResult::UseOriginal;
    };

    let cached = match cached_file_cmap(database, face_id, face_index, generation) {
        Ok(cached) => cached,
        Err(()) => return ProbeResult::UseOriginal,
    };
    if let Some(bytes) = cached {
        return match has_character(&bytes, character) {
            Some(supported) => ProbeResult::Complete(Some(supported)),
            None => {
                remove_face(database, face_id);
                ProbeResult::UseOriginal
            }
        };
    }

    load_cmap(database, face_id, face_index, generation, character)
}

fn cached_file_cmap(
    database: &Arc<Database>,
    face_id: ID,
    face_index: u32,
    generation: &FontSourceGeneration,
) -> Result<Option<Arc<[u8]>>, ()> {
    if !generation.durable_reusable() {
        remove_face(database, face_id);
        return Ok(None);
    }
    cache()
        .lock()
        .map(|mut cache| cache.lookup(database, face_id, face_index, generation))
        .map_err(|_| ())
}

fn load_cmap(
    database: &Arc<Database>,
    face_id: ID,
    face_index: u32,
    generation: &FontSourceGeneration,
    character: char,
) -> ProbeResult {
    let Some(loaded) = read_cmap(database, face_id, face_index, character) else {
        return ProbeResult::Complete(None);
    };
    match loaded {
        LoadedCmap::Failed => ProbeResult::Complete(None),
        LoadedCmap::Uncached(supported) => ProbeResult::Complete(Some(supported)),
        LoadedCmap::UseOriginal => ProbeResult::UseOriginal,
        LoadedCmap::Ready(supported, bytes) => {
            finish_cmap(database, face_id, face_index, generation, supported, bytes)
        }
    }
}

fn read_cmap(
    database: &Arc<Database>,
    face_id: ID,
    face_index: u32,
    character: char,
) -> Option<LoadedCmap> {
    database.with_face_data(face_id, |data, loaded_index| {
        let face = match Face::parse(data, loaded_index) {
            Ok(face) => face,
            Err(_) => return LoadedCmap::Failed,
        };
        let supported = face.glyph_index(character).is_some();
        if loaded_index != face_index {
            return LoadedCmap::UseOriginal;
        }
        let Some(bytes) = face.raw_face().table(CMAP_TAG) else {
            return LoadedCmap::Uncached(supported);
        };
        if face.tables().cmap.is_none() {
            return LoadedCmap::Uncached(supported);
        }
        if bytes.len() > MAX_ENTRY_BYTES {
            return LoadedCmap::UseOriginal;
        }
        LoadedCmap::Ready(supported, Arc::from(bytes))
    })
}

fn finish_cmap(
    database: &Arc<Database>,
    face_id: ID,
    face_index: u32,
    generation: &FontSourceGeneration,
    supported: bool,
    bytes: Arc<[u8]>,
) -> ProbeResult {
    let after = file_source_stamp(database, face_id).2;
    if after == *generation && after.is_file() && after.reusable() {
        if !after.durable_reusable() {
            return ProbeResult::Complete(Some(supported));
        }
        let Ok(mut cache) = cache().lock() else {
            return ProbeResult::Complete(Some(supported));
        };
        cache.insert(database, face_id, face_index, generation.clone(), bytes);
    } else {
        remove_face(database, face_id);
    }
    ProbeResult::Complete(Some(supported))
}

enum LoadedCmap {
    Failed,
    Uncached(bool),
    UseOriginal,
    Ready(bool, Arc<[u8]>),
}

fn has_character(bytes: &[u8], character: char) -> Option<bool> {
    let table = cmap::Table::parse(bytes)?;
    for subtable in table.subtables {
        if subtable.is_unicode() && subtable.glyph_index(u32::from(character)).is_some() {
            return Some(true);
        }
    }
    Some(false)
}

pub(super) fn remove_face(database: &Arc<Database>, face_id: ID) {
    let Ok(mut cache) = cache().lock() else {
        return;
    };
    cache.remove_face(database, face_id);
}

fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Cache::default()))
}

#[cfg(test)]
#[path = "svg_rasterize_text_cmap_cache_fault_tests.rs"]
mod fault_tests;
#[cfg(test)]
#[path = "svg_rasterize_text_cmap_cache_tests.rs"]
mod tests;
