use super::super::super::font::{FontSourceGeneration, font_source_generation};
use super::entry::{self, CacheKey, FontHandle};
use super::{MAX_SOURCE_BYTES, admit_source_budget, cache_font, cached_font, clear_cache};
use harfrust::font::Font;
use resvg::usvg::fontdb::Database;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

pub(super) fn binary_font(
    data: Arc<dyn AsRef<[u8]> + Send + Sync>,
    face_index: u32,
) -> Option<FontHandle> {
    let data_len = data.as_ref().as_ref().len();
    let key = CacheKey::Binary {
        identity: Arc::as_ptr(&data) as *const () as usize,
        len: data.as_ref().as_ref().len(),
        face_index,
    };
    if data.as_ref().as_ref().len() > MAX_SOURCE_BYTES {
        clear_cache();
        return Some(FontHandle {
            font: Font::new(data, face_index)?,
            key: None,
            poisoned: None,
            retained_source_bytes: MAX_SOURCE_BYTES,
        });
    }
    if let Some(font) = cached_font(&key) {
        return Some(font);
    }
    load_and_cache_binary_font(data, face_index, key, data_len)
}

fn load_and_cache_binary_font(
    data: Arc<dyn AsRef<[u8]> + Send + Sync>,
    face_index: u32,
    key: CacheKey,
    data_len: usize,
) -> Option<FontHandle> {
    admit_source_budget(data_len);
    let Some(font) = Font::new(data, face_index) else {
        clear_cache();
        return None;
    };
    let handle = FontHandle {
        font,
        key: Some(key.clone()),
        poisoned: None,
        retained_source_bytes: data_len,
    };
    cache_font(key, handle.clone());
    Some(handle)
}

pub(super) fn file_font(
    database: &Database,
    face_id: resvg::usvg::fontdb::ID,
    path: PathBuf,
    face_index: u32,
) -> Option<FontHandle> {
    let generation = font_source_generation(database, face_id);
    let file_path = path.clone();
    let key = CacheKey::File {
        path,
        generation: generation.clone(),
        face_index,
    };
    if let Some(font) = cached_file_font(database, face_id, &key, &generation) {
        return Some(font);
    }
    if !generation.durable_reusable() {
        clear_cache();
    }
    #[cfg(all(test, target_os = "macos"))]
    crate::markdown::svg_rasterize::font::observation::count(
        crate::markdown::svg_rasterize::font::observation::Metric::MetricsLoad,
    );
    let loaded =
        load_file_font_from_path(database, face_id, &file_path, face_index, key, generation);
    if loaded.is_none() || loaded.as_ref().is_some_and(|font| font.key.is_none()) {
        clear_cache();
    }
    loaded
}

fn load_file_font_from_path(
    database: &Database,
    face_id: resvg::usvg::fontdb::ID,
    path: &Path,
    face_index: u32,
    key: CacheKey,
    generation: FontSourceGeneration,
) -> Option<FontHandle> {
    let metadata = std::fs::metadata(path).ok()?;
    entry::load_file_font(
        database,
        face_id,
        path,
        face_index,
        key,
        generation,
        metadata.len(),
    )
}

fn cached_file_font(
    database: &Database,
    face_id: resvg::usvg::fontdb::ID,
    key: &CacheKey,
    generation: &FontSourceGeneration,
) -> Option<FontHandle> {
    if !generation.durable_reusable() {
        return None;
    }
    let font = cached_font(key)?;
    if font.clean(database, face_id) {
        return Some(font);
    }
    clear_cache();
    None
}

pub(in super::super) fn commit_file_font(
    database: &Database,
    face_id: resvg::usvg::fontdb::ID,
    font: &FontHandle,
) -> bool {
    let Some(CacheKey::File { generation, .. }) = font.key.as_ref() else {
        return true;
    };
    if !generation.reusable() || !font.clean(database, face_id) {
        clear_cache();
        return false;
    }
    if !generation.durable_reusable() {
        clear_cache();
        return true;
    }
    if let Some(key) = font.key.clone() {
        cache_font(key, font.clone());
    }
    true
}
