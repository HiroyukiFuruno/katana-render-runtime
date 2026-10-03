use super::file_generation::{FileStamp, FontSourceGeneration, file_source_stamp};
use super::probe_font_has_char;
use resvg::usvg::fontdb::{Database, ID};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Weak};

const MAX_DATABASES: usize = 8;
const MAX_GLYPHS: usize = 65_536;
const MAX_FILE_STAMPS: usize = 1_024;
type GlyphKey = (ID, char);

struct GlyphCacheEntry {
    database: Weak<Database>,
    glyphs: HashMap<GlyphKey, bool>,
    file_stamps: HashMap<ID, FileStamp>,
}

thread_local! {
    static GLYPH_CACHE: RefCell<Vec<GlyphCacheEntry>> = const { RefCell::new(Vec::new()) };
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
    if is_file && stamp.is_none() {
        GLYPH_CACHE.with(|cache| remove_face(&mut cache.borrow_mut(), database, id));
    }
    if let Some(support) = GLYPH_CACHE.with(|cache| {
        lookup(
            &mut cache.borrow_mut(),
            database,
            key,
            is_file,
            stamp.as_ref(),
        )
    }) {
        return (Some(support), generation);
    }
    /* WHY: file読み込みやfont解析中は借用せず、失敗は復帰後に再検査する。 */
    let Some(support) = probe_font_has_char(database, id, ch) else {
        return (None, generation);
    };
    if !is_file || stamp.is_some() {
        GLYPH_CACHE.with(|cache| insert(&mut cache.borrow_mut(), database, key, support, stamp));
    }
    (Some(support), generation)
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
