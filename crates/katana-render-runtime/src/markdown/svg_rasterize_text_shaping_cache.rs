use super::shape_text_with_face;
use resvg::usvg;
use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{Arc, Weak},
};

const MAX_CACHED_SHAPED_RUN_DATABASES: usize = 8;
const MAX_CACHED_SHAPED_RUNS: usize = 4096;
const MAX_CACHED_SHAPED_RUN_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Hash, PartialEq, Eq)]
struct ShapedRunKey {
    face_id: usvg::fontdb::ID,
    text: String,
    font_size_bits: u32,
    font_feature_settings: Option<String>,
}

struct ShapedRunCacheEntry {
    database: Weak<usvg::fontdb::Database>,
    values: HashMap<ShapedRunKey, f32>,
    retained_bytes: usize,
}

thread_local! {
    static SHAPED_RUN_CACHE: RefCell<Vec<ShapedRunCacheEntry>> = const { RefCell::new(Vec::new()) };
}

pub(super) fn cached_shaped_run(
    database: &Arc<usvg::fontdb::Database>,
    face_id: usvg::fontdb::ID,
    text: &str,
    font_size: f32,
    font_feature_settings: Option<&str>,
) -> Option<f32> {
    let key = ShapedRunKey {
        face_id,
        text: text.to_string(),
        font_size_bits: font_size.to_bits(),
        font_feature_settings: font_feature_settings.map(str::to_string),
    };
    if let Some(width) = cached_shaped_run_width(database, &key) {
        return Some(width);
    }
    let width = shape_text_with_face(database, face_id, text, font_size, font_feature_settings)?;
    cache_shaped_run_width(database, key, width);
    Some(width)
}

fn cached_shaped_run_width(
    database: &Arc<usvg::fontdb::Database>,
    key: &ShapedRunKey,
) -> Option<f32> {
    SHAPED_RUN_CACHE.with(|cache| {
        let mut entries = cache.borrow_mut();
        entries.retain(|entry| entry.database.upgrade().is_some());
        entries
            .iter()
            .find(|entry| {
                entry
                    .database
                    .upgrade()
                    .is_some_and(|cached| Arc::ptr_eq(&cached, database))
            })
            .and_then(|entry| entry.values.get(key).copied())
    })
}

fn cache_shaped_run_width(database: &Arc<usvg::fontdb::Database>, key: ShapedRunKey, width: f32) {
    SHAPED_RUN_CACHE.with(|cache| {
        let mut entries = cache.borrow_mut();
        entries.retain(|entry| entry.database.upgrade().is_some());
        let entry = shaped_run_cache_entry(&mut entries, database);
        let key_bytes = key.retained_bytes();
        if key_bytes > MAX_CACHED_SHAPED_RUN_BYTES || entry.values.contains_key(&key) {
            return;
        }
        while entry.retained_bytes + key_bytes > MAX_CACHED_SHAPED_RUN_BYTES
            && let Some(evicted_key) = entry.values.keys().next().cloned()
        {
            entry.values.remove(&evicted_key);
            entry.retained_bytes = entry
                .retained_bytes
                .saturating_sub(evicted_key.retained_bytes());
        }
        if entry.values.len() >= MAX_CACHED_SHAPED_RUNS
            && let Some(evicted_key) = entry.values.keys().next().cloned()
        {
            entry.values.remove(&evicted_key);
            entry.retained_bytes = entry
                .retained_bytes
                .saturating_sub(evicted_key.retained_bytes());
        }
        entry.retained_bytes += key_bytes;
        entry.values.insert(key, width);
    });
}

impl ShapedRunKey {
    fn retained_bytes(&self) -> usize {
        self.text.len() + self.font_feature_settings.as_ref().map_or(0, String::len)
    }
}

fn shaped_run_cache_entry<'a>(
    entries: &'a mut Vec<ShapedRunCacheEntry>,
    database: &Arc<usvg::fontdb::Database>,
) -> &'a mut ShapedRunCacheEntry {
    if let Some(index) = entries.iter().position(|entry| {
        entry
            .database
            .upgrade()
            .is_some_and(|cached| Arc::ptr_eq(&cached, database))
    }) {
        return &mut entries[index];
    }
    if entries.len() == MAX_CACHED_SHAPED_RUN_DATABASES {
        entries.remove(0);
    }
    entries.push(ShapedRunCacheEntry {
        database: Arc::downgrade(database),
        values: HashMap::new(),
        retained_bytes: 0,
    });
    let inserted_index = entries.len() - 1;
    &mut entries[inserted_index]
}

#[cfg(test)]
mod tests {
    use super::super::font::matching_font_face;
    use super::{
        MAX_CACHED_SHAPED_RUN_BYTES, MAX_CACHED_SHAPED_RUN_DATABASES, MAX_CACHED_SHAPED_RUNS,
        SHAPED_RUN_CACHE, ShapedRunKey, cache_shaped_run_width, cached_shaped_run,
        shaped_run_cache_entry,
    };
    use crate::markdown::svg_rasterize::font::bundled_font_db;
    use resvg::usvg;
    use std::sync::Arc;

    #[test]
    fn repeated_run_measurements_reuse_the_cached_shaped_width() -> Result<(), String> {
        let database = bundled_font_db();
        let face_id = matching_font_face(&database, "Noto Sans", 400, false)
            .ok_or("bundled Noto Sans must be available")?;
        let text = "性能回帰フィクスチャ";
        let first = cached_shaped_run(&database, face_id, text, 16.0, None)
            .ok_or("the bundled font must shape the run")?;
        let second = cached_shaped_run(&database, face_id, text, 16.0, None)
            .ok_or("the cached run must remain available")?;

        assert_eq!(first, second);
        SHAPED_RUN_CACHE.with(|cache| {
            assert!(cache.borrow().iter().any(|entry| !entry.values.is_empty()));
        });
        Ok(())
    }

    #[test]
    fn shaped_run_database_cache_evicts_the_oldest_entry_at_capacity() {
        let databases = (0..=MAX_CACHED_SHAPED_RUN_DATABASES)
            .map(|_| Arc::new(usvg::fontdb::Database::new()))
            .collect::<Vec<_>>();
        let mut entries = Vec::new();

        for database in &databases {
            shaped_run_cache_entry(&mut entries, database);
        }

        assert_eq!(entries.len(), MAX_CACHED_SHAPED_RUN_DATABASES);
        assert!(
            entries[0]
                .database
                .upgrade()
                .is_some_and(|cached| Arc::ptr_eq(&cached, &databases[1]))
        );
    }

    #[test]
    fn shaped_run_cache_evicts_a_width_at_capacity() -> Result<(), String> {
        let database = Arc::new(usvg::fontdb::Database::new());
        SHAPED_RUN_CACHE.with(|cache| cache.borrow_mut().clear());
        for index in 0..MAX_CACHED_SHAPED_RUNS {
            cache_shaped_run_width(&database, shaped_run_key(index), index as f32);
        }
        let inserted_key = shaped_run_key(MAX_CACHED_SHAPED_RUNS);

        cache_shaped_run_width(&database, inserted_key.clone(), 1.0);

        let cache_state = SHAPED_RUN_CACHE.with(|cache| {
            let entries = cache.borrow();
            entries
                .iter()
                .find(|entry| {
                    entry
                        .database
                        .upgrade()
                        .is_some_and(|cached| Arc::ptr_eq(&cached, &database))
                })
                .map(|entry| (entry.values.len(), entry.values.contains_key(&inserted_key)))
        });
        let (cached_width_count, contains_inserted_key) =
            cache_state.ok_or("shaped run cache entry was not created")?;
        assert_eq!(cached_width_count, MAX_CACHED_SHAPED_RUNS);
        assert!(contains_inserted_key);
        Ok(())
    }

    #[test]
    fn shaped_run_cache_skips_a_run_larger_than_the_byte_budget() {
        let database = Arc::new(usvg::fontdb::Database::new());
        SHAPED_RUN_CACHE.with(|cache| cache.borrow_mut().clear());
        let key = ShapedRunKey {
            face_id: usvg::fontdb::ID::default(),
            text: "x".repeat(MAX_CACHED_SHAPED_RUN_BYTES + 1),
            font_size_bits: 16.0_f32.to_bits(),
            font_feature_settings: None,
        };

        cache_shaped_run_width(&database, key, 1.0);

        let cache_state = SHAPED_RUN_CACHE.with(|cache| {
            cache.borrow().iter().any(|entry| {
                entry
                    .database
                    .upgrade()
                    .is_some_and(|cached| Arc::ptr_eq(&cached, &database))
                    && !entry.values.is_empty()
            })
        });
        assert!(!cache_state);
    }

    #[test]
    fn shaped_run_cache_evicts_entries_to_fit_the_byte_budget() -> Result<(), String> {
        let database = Arc::new(usvg::fontdb::Database::new());
        SHAPED_RUN_CACHE.with(|cache| cache.borrow_mut().clear());
        let retained_text_length = MAX_CACHED_SHAPED_RUN_BYTES / 2 + 1;
        let first_key = shaped_run_key_with_text('a', retained_text_length);
        let second_key = shaped_run_key_with_text('b', retained_text_length);

        cache_shaped_run_width(&database, first_key, 1.0);
        cache_shaped_run_width(&database, second_key.clone(), 2.0);

        let cache_state = SHAPED_RUN_CACHE.with(|cache| {
            cache.borrow().iter().find_map(|entry| {
                entry
                    .database
                    .upgrade()
                    .is_some_and(|cached| Arc::ptr_eq(&cached, &database))
                    .then_some((
                        entry.values.len(),
                        entry.values.contains_key(&second_key),
                        entry.retained_bytes,
                    ))
            })
        });
        let (cached_width_count, contains_second_key, retained_bytes) =
            cache_state.ok_or("shaped run cache entry was not created")?;
        assert_eq!(cached_width_count, 1);
        assert!(contains_second_key);
        assert!(retained_bytes <= MAX_CACHED_SHAPED_RUN_BYTES);
        Ok(())
    }

    fn shaped_run_key_with_text(fill: char, length: usize) -> ShapedRunKey {
        ShapedRunKey {
            face_id: usvg::fontdb::ID::default(),
            text: fill.to_string().repeat(length),
            font_size_bits: 16.0_f32.to_bits(),
            font_feature_settings: None,
        }
    }

    fn shaped_run_key(index: usize) -> ShapedRunKey {
        ShapedRunKey {
            face_id: usvg::fontdb::ID::default(),
            text: format!("run-{index}"),
            font_size_bits: 16.0_f32.to_bits(),
            font_feature_settings: None,
        }
    }
}
