use super::probe_font_has_char;
use resvg::usvg::fontdb::{Database, ID};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Weak};

const MAX_DATABASES: usize = 8;
const MAX_GLYPHS: usize = 65_536;
type GlyphKey = (ID, char);

struct GlyphCacheEntry {
    database: Weak<Database>,
    glyphs: HashMap<GlyphKey, bool>,
}

thread_local! {
    static GLYPH_CACHE: RefCell<Vec<GlyphCacheEntry>> = const { RefCell::new(Vec::new()) };
}

pub(super) fn cached_font_has_char(database: &Arc<Database>, id: ID, ch: char) -> Option<bool> {
    let key = (id, ch);
    if let Some(support) = GLYPH_CACHE.with(|cache| lookup(&mut cache.borrow_mut(), database, key))
    {
        return Some(support);
    }
    /* WHY: file読み込みやfont解析中は借用せず、失敗は復帰後に再検査する。 */
    let support = probe_font_has_char(database, id, ch)?;
    GLYPH_CACHE.with(|cache| insert(&mut cache.borrow_mut(), database, key, support));
    Some(support)
}

fn lookup(
    entries: &mut Vec<GlyphCacheEntry>,
    database: &Arc<Database>,
    key: GlyphKey,
) -> Option<bool> {
    entries.retain(|entry| entry.database.upgrade().is_some());
    entries
        .iter()
        .find(|entry| same_database(entry, database))
        .and_then(|entry| entry.glyphs.get(&key).copied())
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
) {
    entries.retain(|entry| entry.database.upgrade().is_some());
    if let Some(entry) = entries
        .iter_mut()
        .find(|entry| same_database(entry, database))
    {
        insert_glyph(entry, key, support);
        return;
    }
    if entries.len() == MAX_DATABASES {
        entries.remove(0);
    }
    entries.push(GlyphCacheEntry {
        database: Arc::downgrade(database),
        glyphs: HashMap::from([(key, support)]),
    });
}

fn insert_glyph(entry: &mut GlyphCacheEntry, key: GlyphKey, support: bool) {
    if entry.glyphs.len() >= MAX_GLYPHS
        && let Some(evicted) = entry.glyphs.keys().next().copied()
    {
        entry.glyphs.remove(&evicted);
    }
    entry.glyphs.insert(key, support);
}

#[cfg(test)]
#[path = "svg_rasterize_text_glyph_cache_tests.rs"]
mod tests;
