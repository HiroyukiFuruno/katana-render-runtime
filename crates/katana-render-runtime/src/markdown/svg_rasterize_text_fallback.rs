use super::font::{font_has_char, matching_fallback_face};
use resvg::usvg;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Weak};

const MAX_CACHED_HTML_FALLBACK_DATABASES: usize = 8;
const MAX_CACHED_HTML_FALLBACK_FACES: usize = 256;

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct HtmlFallbackKey {
    base_face_id: usvg::fontdb::ID,
    character: char,
    requested_weight: u16,
    requested_italic: bool,
}

struct HtmlFallbackCacheEntry {
    database: Weak<usvg::fontdb::Database>,
    faces: HashMap<HtmlFallbackKey, usvg::fontdb::ID>,
}

thread_local! {
    static HTML_FALLBACK_CACHE: RefCell<Vec<HtmlFallbackCacheEntry>> = const { RefCell::new(Vec::new()) };
}

pub(super) fn html_font_runs(
    database: &Arc<usvg::fontdb::Database>,
    base_face_id: usvg::fontdb::ID,
    text: &str,
    requested_weight: u16,
    requested_italic: bool,
) -> Vec<(usvg::fontdb::ID, String)> {
    let mut runs: Vec<(usvg::fontdb::ID, String)> = Vec::new();
    for character in text.chars() {
        let face_id = cached_html_face(
            database,
            base_face_id,
            character,
            requested_weight,
            requested_italic,
        );
        append_font_run(&mut runs, face_id, character);
    }
    runs
}

fn cached_html_face(
    database: &Arc<usvg::fontdb::Database>,
    base_face_id: usvg::fontdb::ID,
    character: char,
    requested_weight: u16,
    requested_italic: bool,
) -> usvg::fontdb::ID {
    let key = HtmlFallbackKey {
        base_face_id,
        character,
        requested_weight,
        requested_italic,
    };
    HTML_FALLBACK_CACHE
        .with(|cache| resolve_cached_html_face(&mut cache.borrow_mut(), database, key))
}

fn resolve_cached_html_face(
    entries: &mut Vec<HtmlFallbackCacheEntry>,
    database: &Arc<usvg::fontdb::Database>,
    key: HtmlFallbackKey,
) -> usvg::fontdb::ID {
    entries.retain(|entry| entry.database.upgrade().is_some());
    if let Some(entry) = entries.iter_mut().find(|entry| {
        entry
            .database
            .upgrade()
            .is_some_and(|cached| Arc::ptr_eq(&cached, database))
    }) {
        return resolve_existing_html_fallback_face(entry, database, key);
    }
    if entries.len() == MAX_CACHED_HTML_FALLBACK_DATABASES {
        entries.remove(0);
    }
    let face_id = resolve_html_fallback_face(database, key);
    entries.push(HtmlFallbackCacheEntry {
        database: Arc::downgrade(database),
        faces: HashMap::from([(key, face_id)]),
    });
    face_id
}

fn resolve_existing_html_fallback_face(
    entry: &mut HtmlFallbackCacheEntry,
    database: &usvg::fontdb::Database,
    key: HtmlFallbackKey,
) -> usvg::fontdb::ID {
    if let Some(face_id) = entry.faces.get(&key) {
        return *face_id;
    }
    if entry.faces.len() >= MAX_CACHED_HTML_FALLBACK_FACES
        && let Some(evicted_key) = entry.faces.keys().next().copied()
    {
        entry.faces.remove(&evicted_key);
    }
    let face_id = resolve_html_fallback_face(database, key);
    entry.faces.insert(key, face_id);
    face_id
}

fn resolve_html_fallback_face(
    database: &usvg::fontdb::Database,
    key: HtmlFallbackKey,
) -> usvg::fontdb::ID {
    resolved_html_face(
        database,
        key.base_face_id,
        key.character,
        key.requested_weight,
        key.requested_italic,
    )
}

fn append_font_run(
    runs: &mut Vec<(usvg::fontdb::ID, String)>,
    face_id: usvg::fontdb::ID,
    character: char,
) {
    if let Some((run_face_id, run)) = runs.last_mut()
        && *run_face_id == face_id
    {
        run.push(character);
    } else {
        runs.push((face_id, character.to_string()));
    }
}

fn resolved_html_face(
    database: &usvg::fontdb::Database,
    base_face_id: usvg::fontdb::ID,
    character: char,
    requested_weight: u16,
    requested_italic: bool,
) -> usvg::fontdb::ID {
    if font_has_char(database, base_face_id, character) {
        base_face_id
    } else {
        matching_fallback_face(
            database,
            base_face_id,
            character,
            requested_weight,
            requested_italic,
        )
        .unwrap_or(base_face_id)
    }
}

#[cfg(test)]
mod tests {
    use super::super::font::{font_has_char, matching_font_face};
    use super::{
        HtmlFallbackCacheEntry, HtmlFallbackKey, MAX_CACHED_HTML_FALLBACK_DATABASES,
        MAX_CACHED_HTML_FALLBACK_FACES, html_font_runs, resolve_cached_html_face,
        resolve_existing_html_fallback_face,
    };
    use crate::markdown::svg_rasterize::font::{bundled_font_db, html_font_db_for_text};
    use resvg::usvg;
    use std::collections::{HashMap, HashSet};
    use std::sync::{Arc, Weak};

    #[test]
    fn html_font_fallback_is_scoped_to_the_current_database() {
        let bundled = bundled_font_db();
        let bundled_base = matching_font_face(&bundled, "Noto Sans", 400, false);
        let bundled_runs =
            bundled_base.map(|base| html_font_runs(&bundled, base, "日", 400, false));
        let html = html_font_db_for_text("Noto Sans", "日");
        let html_base = matching_font_face(&html, "Noto Sans", 400, false);
        let used_html_fallback = html_base.is_some_and(|base| {
            !font_has_char(&html, base, '日')
                && html_font_runs(&html, base, "日本日本", 400, false)
                    .first()
                    .is_some_and(|(fallback, _)| *fallback != base)
        });

        assert!(bundled_runs.is_some_and(|runs| runs.len() == 1));
        assert!(used_html_fallback);
    }

    #[test]
    fn html_font_fallback_accepts_requested_weight_and_style() {
        let html = html_font_db_for_text("Noto Sans", "日");
        let requested_faces = [(700, false), (700, true)]
            .into_iter()
            .map(|(weight, italic)| {
                matching_font_face(&html, "Noto Sans", weight, italic).is_some_and(|base| {
                    html_font_runs(&html, base, "日", weight, italic)
                        .first()
                        .is_some_and(|(fallback, _)| {
                            *fallback != base && font_has_char(&html, *fallback, '日')
                        })
                })
            })
            .collect::<Vec<_>>();

        assert_eq!(requested_faces, [true, true]);
    }

    #[test]
    fn expired_html_fallback_database_entries_are_swept() {
        let expired_database = Arc::new(usvg::fontdb::Database::new());
        let expired_entry = HtmlFallbackCacheEntry {
            database: Arc::downgrade(&expired_database),
            faces: HashMap::new(),
        };
        drop(expired_database);

        let database = Arc::new(usvg::fontdb::Database::new());
        let key = HtmlFallbackKey {
            base_face_id: usvg::fontdb::ID::default(),
            character: 'x',
            requested_weight: 400,
            requested_italic: false,
        };
        let mut entries = vec![expired_entry];

        resolve_cached_html_face(&mut entries, &database, key);

        assert_eq!(entries.len(), 1);
        assert!(
            entries[0]
                .database
                .upgrade()
                .is_some_and(|cached| { Arc::ptr_eq(&cached, &database) })
        );
    }

    #[test]
    fn html_fallback_database_cache_evicts_oldest_entry_at_capacity() {
        let databases = (0..=MAX_CACHED_HTML_FALLBACK_DATABASES)
            .map(|_| Arc::new(usvg::fontdb::Database::new()))
            .collect::<Vec<_>>();
        let key = HtmlFallbackKey {
            base_face_id: usvg::fontdb::ID::default(),
            character: 'x',
            requested_weight: 400,
            requested_italic: false,
        };
        let mut entries = Vec::new();

        for database in &databases {
            resolve_cached_html_face(&mut entries, database, key);
        }

        assert_eq!(entries.len(), MAX_CACHED_HTML_FALLBACK_DATABASES);
        assert!(
            entries[0]
                .database
                .upgrade()
                .is_some_and(|cached| { Arc::ptr_eq(&cached, &databases[1]) })
        );
    }

    fn full_face_cache() -> HtmlFallbackCacheEntry {
        HtmlFallbackCacheEntry {
            database: Weak::new(),
            faces: (0..MAX_CACHED_HTML_FALLBACK_FACES)
                .map(|weight| {
                    (
                        HtmlFallbackKey {
                            base_face_id: usvg::fontdb::ID::default(),
                            character: 'x',
                            requested_weight: weight as u16,
                            requested_italic: false,
                        },
                        usvg::fontdb::ID::default(),
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn html_fallback_face_cache_evicts_an_entry_at_capacity() {
        let database = usvg::fontdb::Database::new();
        let mut entry = full_face_cache();
        let original_keys = entry.faces.keys().copied().collect::<HashSet<_>>();
        let new_key = HtmlFallbackKey {
            base_face_id: usvg::fontdb::ID::default(),
            character: 'x',
            requested_weight: MAX_CACHED_HTML_FALLBACK_FACES as u16,
            requested_italic: false,
        };

        resolve_existing_html_fallback_face(&mut entry, &database, new_key);

        assert_eq!(entry.faces.len(), MAX_CACHED_HTML_FALLBACK_FACES);
        assert!(entry.faces.contains_key(&new_key));
        assert!(entry.faces.keys().any(|key| !original_keys.contains(key)));
    }
}
