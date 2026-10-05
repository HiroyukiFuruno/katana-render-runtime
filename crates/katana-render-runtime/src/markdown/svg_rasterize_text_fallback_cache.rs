use super::fallback_generation::HtmlFallbackSelection;
use resvg::usvg;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Weak};

pub(super) const MAX_CACHED_HTML_FALLBACK_DATABASES: usize = 8;
pub(super) const MAX_CACHED_HTML_FALLBACK_FACES: usize = 256;

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub(super) struct HtmlFallbackKey {
    pub(super) base_face_id: usvg::fontdb::ID,
    pub(super) character: char,
    pub(super) requested_weight: u16,
    pub(super) requested_italic: bool,
}

pub(super) struct HtmlFallbackCacheEntry {
    pub(super) database: Weak<usvg::fontdb::Database>,
    pub(super) faces: HashMap<HtmlFallbackKey, HtmlFallbackSelection>,
}

thread_local! {
    static HTML_FALLBACK_CACHE: RefCell<Vec<HtmlFallbackCacheEntry>> = const { RefCell::new(Vec::new()) };
}

pub(super) fn lookup_html_face(
    entries: &mut Vec<HtmlFallbackCacheEntry>,
    database: &Arc<usvg::fontdb::Database>,
    key: HtmlFallbackKey,
) -> Option<HtmlFallbackSelection> {
    entries.retain(|entry| entry.database.upgrade().is_some());
    entries
        .iter()
        .find(|entry| same_html_database(entry, database))
        .and_then(|entry| entry.faces.get(&key).cloned())
}

pub(super) fn remove_html_face(
    entries: &mut [HtmlFallbackCacheEntry],
    database: &Arc<usvg::fontdb::Database>,
    key: HtmlFallbackKey,
) {
    if let Some(entry) = entries
        .iter_mut()
        .find(|entry| same_html_database(entry, database))
    {
        entry.faces.remove(&key);
    }
}

fn same_html_database(
    entry: &HtmlFallbackCacheEntry,
    database: &Arc<usvg::fontdb::Database>,
) -> bool {
    entry
        .database
        .upgrade()
        .is_some_and(|cached| Arc::ptr_eq(&cached, database))
}

pub(super) fn insert_html_face(
    entries: &mut Vec<HtmlFallbackCacheEntry>,
    database: &Arc<usvg::fontdb::Database>,
    key: HtmlFallbackKey,
    selection: HtmlFallbackSelection,
) {
    entries.retain(|entry| entry.database.upgrade().is_some());
    if let Some(entry) = entries
        .iter_mut()
        .find(|entry| same_html_database(entry, database))
    {
        insert_existing_html_face(entry, key, selection);
        return;
    }
    if entries.len() == MAX_CACHED_HTML_FALLBACK_DATABASES {
        entries.remove(0);
    }
    entries.push(HtmlFallbackCacheEntry {
        database: Arc::downgrade(database),
        faces: HashMap::from([(key, selection)]),
    });
}

pub(super) fn insert_existing_html_face(
    entry: &mut HtmlFallbackCacheEntry,
    key: HtmlFallbackKey,
    selection: HtmlFallbackSelection,
) {
    if entry.faces.len() >= MAX_CACHED_HTML_FALLBACK_FACES
        && let Some(evicted_key) = entry.faces.keys().next().copied()
    {
        entry.faces.remove(&evicted_key);
    }
    entry.faces.insert(key, selection);
}

pub(super) fn lookup_cached(
    database: &Arc<usvg::fontdb::Database>,
    key: HtmlFallbackKey,
) -> Option<HtmlFallbackSelection> {
    HTML_FALLBACK_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let selection = lookup_html_face(&mut cache, database, key)?;
        if selection
            .dependencies
            .iter()
            .all(|(_, generation)| generation.durable_reusable())
        {
            Some(selection)
        } else {
            remove_html_face(&mut cache, database, key);
            None
        }
    })
}

pub(super) fn remove_cached(database: &Arc<usvg::fontdb::Database>, key: HtmlFallbackKey) {
    HTML_FALLBACK_CACHE.with(|cache| remove_html_face(&mut cache.borrow_mut(), database, key));
}

pub(super) fn insert_cached(
    database: &Arc<usvg::fontdb::Database>,
    key: HtmlFallbackKey,
    selection: HtmlFallbackSelection,
) {
    if !selection
        .dependencies
        .iter()
        .all(|(_, generation)| generation.durable_reusable())
    {
        remove_cached(database, key);
        return;
    }
    HTML_FALLBACK_CACHE.with(|cache| {
        insert_html_face(&mut cache.borrow_mut(), database, key, selection);
    });
}

#[cfg(test)]
#[path = "svg_rasterize_text_fallback_cache_generation_tests.rs"]
mod generation_tests;
