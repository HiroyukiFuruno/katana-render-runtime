use super::cmap_cache;
#[cfg(not(test))]
use super::file_generation::file_source_stamp;
use super::file_generation::{FileStamp, FontSourceGeneration};
use super::probe_font_has_char;
use resvg::usvg::fontdb::{Database, ID};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Weak};

const MAX_DATABASES: usize = 8;
const MAX_GLYPHS: usize = 65_536;
const MAX_FILE_STAMPS: usize = 1_024;
type GlyphKey = (ID, char);
#[cfg(test)]
type FileStampCallback = Box<dyn FnOnce()>;

struct GlyphCacheEntry {
    database: Weak<Database>,
    glyphs: HashMap<GlyphKey, bool>,
    file_stamps: HashMap<ID, FileStamp>,
}

thread_local! {
    static GLYPH_CACHE: RefCell<Vec<GlyphCacheEntry>> = const { RefCell::new(Vec::new()) };
    #[cfg(test)]
    static AFTER_FILE_STAMP: RefCell<Option<FileStampCallback>> = const { RefCell::new(None) };
}

#[cfg(test)]
pub(super) fn set_after_file_stamp_for_test(callback: impl FnOnce() + 'static) {
    AFTER_FILE_STAMP.with(|pending| *pending.borrow_mut() = Some(Box::new(callback)));
}

#[cfg(test)]
fn file_source_stamp(
    database: &Database,
    id: ID,
) -> (bool, Option<FileStamp>, FontSourceGeneration) {
    let result = super::file_generation::file_source_stamp(database, id);
    if let Some(callback) = AFTER_FILE_STAMP.with(|pending| pending.borrow_mut().take()) {
        callback();
    }
    result
}

#[cfg(test)]
pub(super) fn cached_font_has_char(database: &Arc<Database>, id: ID, ch: char) -> Option<bool> {
    cached_font_has_char_with_generation(database, id, ch).0
}

pub(in super::super) fn cached_font_has_char_with_generation(
    database: &Arc<Database>,
    id: ID,
    ch: char,
) -> (Option<bool>, FontSourceGeneration) {
    let key = (id, ch);
    let (is_file, stamp, generation) = file_source_stamp(database, id);
    if is_file && (!generation.durable_reusable() || stamp.is_none()) {
        GLYPH_CACHE.with(|cache| remove_face(&mut cache.borrow_mut(), database, id));
        cmap_cache::remove_face(database, id);
    }
    if generation.durable_reusable()
        && let Some(support) = cached_glyph_support(database, key, is_file, stamp.as_ref())
    {
        return (Some(support), generation);
    }
    /* WHY: file読み込みやfont解析中は借用せず、失敗は復帰後に再検査する。 */
    let probe = probe_font_with_cmap(database, id, ch, &generation);
    let Some(support) = probe else {
        return (None, generation);
    };
    if generation.durable_reusable() && (!is_file || stamp.is_some()) {
        GLYPH_CACHE.with(|cache| insert(&mut cache.borrow_mut(), database, key, support, stamp));
    }
    (Some(support), generation)
}

fn cached_glyph_support(
    database: &Arc<Database>,
    key: GlyphKey,
    is_file: bool,
    stamp: Option<&FileStamp>,
) -> Option<bool> {
    GLYPH_CACHE.with(|cache| lookup(&mut cache.borrow_mut(), database, key, is_file, stamp))
}

fn probe_font_with_cmap(
    database: &Arc<Database>,
    id: ID,
    character: char,
    generation: &FontSourceGeneration,
) -> Option<bool> {
    if !generation.is_file() {
        return probe_font_has_char(database, id, character);
    }
    match cmap_cache::probe_file(database, id, generation, character) {
        cmap_cache::ProbeResult::Complete(result) => result,
        cmap_cache::ProbeResult::UseOriginal => probe_font_has_char(database, id, character),
    }
}

fn lookup(
    entries: &mut Vec<GlyphCacheEntry>,
    database: &Arc<Database>,
    key: GlyphKey,
    is_file: bool,
    stamp: Option<&FileStamp>,
) -> Option<bool> {
    entries.retain(|entry| entry.database.upgrade().is_some());
    let entry = entries
        .iter()
        .position(|entry| same_database(entry, database))?;
    if is_file {
        let Some(stamp) = stamp else {
            remove_face_from_entry(&mut entries[entry], key.0);
            return None;
        };
        if entries[entry].file_stamps.get(&key.0) != Some(stamp) {
            remove_face_from_entry(&mut entries[entry], key.0);
            return None;
        }
    }
    entries[entry].glyphs.get(&key).copied()
}

fn remove_face(entries: &mut [GlyphCacheEntry], database: &Arc<Database>, id: ID) {
    if let Some(entry) = entries
        .iter_mut()
        .find(|entry| same_database(entry, database))
    {
        remove_face_from_entry(entry, id);
    }
}

fn remove_face_from_entry(entry: &mut GlyphCacheEntry, id: ID) {
    entry.glyphs.retain(|(face_id, _), _| *face_id != id);
    entry.file_stamps.remove(&id);
}

fn same_database(entry: &GlyphCacheEntry, database: &Arc<Database>) -> bool {
    entry
        .database
        .upgrade()
        .is_some_and(|cached| Arc::ptr_eq(&cached, database))
}

fn insert(
    entries: &mut Vec<GlyphCacheEntry>,
    database: &Arc<Database>,
    key: GlyphKey,
    support: bool,
    stamp: Option<FileStamp>,
) {
    entries.retain(|entry| entry.database.upgrade().is_some());
    if let Some(entry) = entries
        .iter_mut()
        .find(|entry| same_database(entry, database))
    {
        insert_glyph(entry, key, support, stamp);
        return;
    }
    if entries.len() == MAX_DATABASES {
        entries.remove(0);
    }
    entries.push(GlyphCacheEntry {
        database: Arc::downgrade(database),
        glyphs: HashMap::from([(key, support)]),
        file_stamps: match stamp {
            Some(stamp) => HashMap::from([(key.0, stamp)]),
            None => HashMap::new(),
        },
    });
}

fn insert_glyph(
    entry: &mut GlyphCacheEntry,
    key: GlyphKey,
    support: bool,
    stamp: Option<FileStamp>,
) {
    if entry.glyphs.len() >= MAX_GLYPHS
        && let Some(evicted) = entry.glyphs.keys().next().copied()
    {
        entry.glyphs.remove(&evicted);
    }
    if let Some(stamp) = stamp {
        if !entry.file_stamps.contains_key(&key.0)
            && entry.file_stamps.len() >= MAX_FILE_STAMPS
            && let Some(evicted) = entry.file_stamps.keys().next().copied()
        {
            remove_face_from_entry(entry, evicted);
        }
        entry.file_stamps.insert(key.0, stamp);
    }
    entry.glyphs.insert(key, support);
}

#[cfg(test)]
#[path = "svg_rasterize_text_glyph_cache_tests.rs"]
mod tests;
