use super::font::{cached_font_has_char, matching_fallback_with_probe};
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
    let mut render_faces = HashMap::new();
    for character in text.chars() {
        let face_id = cached_html_face(
            database,
            base_face_id,
            character,
            requested_weight,
            requested_italic,
            &mut render_faces,
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
    render_faces: &mut HashMap<HtmlFallbackKey, usvg::fontdb::ID>,
) -> usvg::fontdb::ID {
    let key = HtmlFallbackKey {
        base_face_id,
        character,
        requested_weight,
        requested_italic,
    };
    cached_html_face_for_key(database, key, render_faces)
}

fn cached_html_face_for_key(
    database: &Arc<usvg::fontdb::Database>,
    key: HtmlFallbackKey,
    render_faces: &mut HashMap<HtmlFallbackKey, usvg::fontdb::ID>,
) -> usvg::fontdb::ID {
    if let Some(face_id) = render_faces.get(&key) {
        return *face_id;
    }
    let cached =
        HTML_FALLBACK_CACHE.with(|cache| lookup_html_face(&mut cache.borrow_mut(), database, key));
    if let Some(face_id) = cached {
        render_faces.insert(key, face_id);
        return face_id;
    }
    let (face_id, cacheable) = resolve_html_fallback_face(database, key);
    if cacheable {
        HTML_FALLBACK_CACHE.with(|cache| {
            insert_html_face(&mut cache.borrow_mut(), database, key, face_id);
        });
    }
    render_faces.insert(key, face_id);
    face_id
}

fn lookup_html_face(
    entries: &mut Vec<HtmlFallbackCacheEntry>,
    database: &Arc<usvg::fontdb::Database>,
    key: HtmlFallbackKey,
) -> Option<usvg::fontdb::ID> {
    entries.retain(|entry| entry.database.upgrade().is_some());
    entries
        .iter()
        .find(|entry| same_html_database(entry, database))
        .and_then(|entry| entry.faces.get(&key).copied())
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

fn insert_html_face(
    entries: &mut Vec<HtmlFallbackCacheEntry>,
    database: &Arc<usvg::fontdb::Database>,
    key: HtmlFallbackKey,
    face_id: usvg::fontdb::ID,
) {
    entries.retain(|entry| entry.database.upgrade().is_some());
    if let Some(entry) = entries
        .iter_mut()
        .find(|entry| same_html_database(entry, database))
    {
        insert_existing_html_face(entry, key, face_id);
        return;
    }
    if entries.len() == MAX_CACHED_HTML_FALLBACK_DATABASES {
        entries.remove(0);
    }
    entries.push(HtmlFallbackCacheEntry {
        database: Arc::downgrade(database),
        faces: HashMap::from([(key, face_id)]),
    });
}

fn insert_existing_html_face(
    entry: &mut HtmlFallbackCacheEntry,
    key: HtmlFallbackKey,
    face_id: usvg::fontdb::ID,
) {
    if entry.faces.len() >= MAX_CACHED_HTML_FALLBACK_FACES
        && let Some(evicted_key) = entry.faces.keys().next().copied()
    {
        entry.faces.remove(&evicted_key);
    }
    entry.faces.insert(key, face_id);
}

fn resolve_html_fallback_face(
    database: &Arc<usvg::fontdb::Database>,
    key: HtmlFallbackKey,
) -> (usvg::fontdb::ID, bool) {
    let support = cached_font_has_char(database, key.base_face_id, key.character);
    if support == Some(true) {
        return (key.base_face_id, true);
    }
    let (fallback, cacheable) = matching_fallback_with_probe(
        database,
        key.base_face_id,
        key.character,
        key.requested_weight,
        key.requested_italic,
        |id, ch| cached_font_has_char(database, id, ch),
    );
    (
        fallback.unwrap_or(key.base_face_id),
        support.is_some() && cacheable,
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

#[cfg(test)]
mod tests {
    use super::super::font::{font_has_char, matching_font_face};
    use super::{
        HtmlFallbackCacheEntry, HtmlFallbackKey, MAX_CACHED_HTML_FALLBACK_DATABASES,
        MAX_CACHED_HTML_FALLBACK_FACES, cached_html_face, html_font_runs,
        insert_existing_html_face, insert_html_face,
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
    fn html_font_fallback_is_memoized_within_a_render() -> Result<(), String> {
        let database = html_font_db_for_text("Noto Sans", "日");
        let base_face_id = matching_font_face(&database, "Noto Sans", 400, false)
            .ok_or("HTML database must contain the requested base face")?;
        let mut render_faces = HashMap::new();

        let first = cached_html_face(&database, base_face_id, '日', 400, false, &mut render_faces);
        let second = cached_html_face(&database, base_face_id, '日', 400, false, &mut render_faces);

        assert_eq!(first, second);
        assert_eq!(render_faces.len(), 1);
        Ok(())
    }

    type RecoveryFixture = (
        Arc<usvg::fontdb::Database>,
        usvg::fontdb::ID,
        usvg::fontdb::ID,
        Vec<u8>,
        std::path::PathBuf,
    );

    fn recovery_fixture(missing_base: bool) -> Result<RecoveryFixture, Box<dyn std::error::Error>> {
        let bundled = bundled_font_db();
        let original = bundled.faces().next().ok_or("bundled face missing")?;
        let bytes = bundled
            .with_face_data(original.id, |data, _| data.to_vec())
            .ok_or("bundled data missing")?;
        let path = std::env::temp_dir().join(format!(
            "krr-glyph-recovery-{}-{missing_base}.ttf",
            std::process::id()
        ));
        let mut database = usvg::fontdb::Database::new();
        let mut missing = original.clone();
        missing.source = usvg::fontdb::Source::File(path.clone());
        if !missing_base {
            let mut corrupt = original.clone();
            corrupt.source = usvg::fontdb::Source::Binary(Arc::new(vec![0; 32]));
            database.push_face_info(corrupt);
        }
        let recovered = database.push_face_info(missing);
        let fallback = database.push_face_info(original.clone());
        Ok((Arc::new(database), recovered, fallback, bytes, path))
    }

    #[test]
    fn missing_font_file_does_not_cache_the_fallback_choice()
    -> Result<(), Box<dyn std::error::Error>> {
        let (database, base, fallback, bytes, path) = recovery_fixture(true)?;
        let mut render_faces = HashMap::new();
        assert_eq!(
            cached_html_face(&database, base, 'A', 400, false, &mut render_faces),
            fallback
        );
        assert_eq!(render_faces.len(), 1);
        assert_eq!(
            cached_html_face(&database, base, 'A', 400, false, &mut render_faces),
            fallback
        );
        std::fs::write(&path, bytes)?;
        assert_eq!(
            cached_html_face(&database, base, 'A', 400, false, &mut render_faces),
            fallback
        );
        let mut next_render_faces = HashMap::new();
        let recovered = cached_html_face(&database, base, 'A', 400, false, &mut next_render_faces);
        std::fs::remove_file(path)?;
        assert_eq!(recovered, base);
        assert_eq!(next_render_faces.len(), 1);
        Ok(())
    }

    #[test]
    fn unavailable_fallback_candidate_is_retried_after_its_file_returns()
    -> Result<(), Box<dyn std::error::Error>> {
        let (database, candidate, fallback, bytes, path) = recovery_fixture(false)?;
        let base = database.faces().next().ok_or("base missing")?.id;
        let mut render_faces = HashMap::new();
        assert_eq!(
            cached_html_face(&database, base, 'A', 400, false, &mut render_faces),
            fallback
        );
        assert_eq!(render_faces.len(), 1);
        std::fs::write(&path, bytes)?;
        assert_eq!(
            cached_html_face(&database, base, 'A', 400, false, &mut render_faces),
            fallback
        );
        let mut next_render_faces = HashMap::new();
        let recovered = cached_html_face(&database, base, 'A', 400, false, &mut next_render_faces);
        std::fs::remove_file(path)?;
        assert_eq!(recovered, candidate);
        assert_eq!(next_render_faces.len(), 1);
        Ok(())
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

        insert_html_face(&mut entries, &database, key, key.base_face_id);

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
            insert_html_face(&mut entries, database, key, key.base_face_id);
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
        let mut entry = full_face_cache();
        let original_keys = entry.faces.keys().copied().collect::<HashSet<_>>();
        let new_key = HtmlFallbackKey {
            base_face_id: usvg::fontdb::ID::default(),
            character: 'x',
            requested_weight: MAX_CACHED_HTML_FALLBACK_FACES as u16,
            requested_italic: false,
        };

        insert_existing_html_face(&mut entry, new_key, new_key.base_face_id);

        assert_eq!(entry.faces.len(), MAX_CACHED_HTML_FALLBACK_FACES);
        assert!(entry.faces.contains_key(&new_key));
        assert!(entry.faces.keys().any(|key| !original_keys.contains(key)));
    }
}
