use super::FontShape;
use super::TextSpacingTrim;
use super::font::{FontSourceGeneration, font_source_generation};
use super::harfrust::shape_text_with_face_with_trim;
use resvg::usvg;
use sha2::{Digest, Sha256};
use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{Arc, Weak},
};

const MAX_CACHED_SHAPED_RUN_DATABASES: usize = 8;
const MAX_CACHED_SHAPED_RUNS: usize = 4096;
const MAX_CACHED_SHAPED_RUN_BYTES: usize = 4 * 1024 * 1024;
const SHA256_DIGEST_BYTES: usize = 32;

#[path = "svg_rasterize_text_shaping_cache_generation.rs"]
mod generation;

#[cfg(test)]
#[path = "svg_rasterize_text_shaping_cache_generation_tests.rs"]
pub(super) mod generation_tests;

#[derive(Clone, Hash, PartialEq, Eq)]
struct ShapedRunKey {
    face_id: usvg::fontdb::ID,
    text_sha256: [u8; SHA256_DIGEST_BYTES],
    source_bytes: usize,
    font_size_bits: u32,
    weight: u16,
    italic: bool,
    font_feature_sha256: Option<[u8; SHA256_DIGEST_BYTES]>,
    trim: TextSpacingTrim,
}

#[derive(Clone, Copy)]
struct CachedShapedRun {
    advance: f32,
    svg_endpoints: Option<(f32, f32)>,
    ink_bounds: Option<super::harfrust::SvgInkBounds>,
}

struct ShapedRunCacheEntry {
    database: Weak<usvg::fontdb::Database>,
    values: HashMap<ShapedRunKey, CachedShapedRun>,
    generations: HashMap<usvg::fontdb::ID, generation::FaceGeneration>,
    retained_bytes: usize,
}

thread_local! {
    static SHAPED_RUN_CACHE: RefCell<Vec<ShapedRunCacheEntry>> = const { RefCell::new(Vec::new()) };
}

#[cfg(test)]
pub(super) fn cached_shaped_run(
    database: &Arc<usvg::fontdb::Database>,
    face_id: usvg::fontdb::ID,
    text: &str,
    font: FontShape,
    font_feature_settings: Option<&str>,
) -> Option<f32> {
    cached_shaped_run_with_trim(database, face_id, text, font, font_feature_settings)
}

pub(super) fn cached_shaped_run_with_trim(
    database: &Arc<usvg::fontdb::Database>,
    face_id: usvg::fontdb::ID,
    text: &str,
    font: FontShape,
    font_feature_settings: Option<&str>,
) -> Option<f32> {
    let key = ShapedRunKey::new(face_id, text, font, font_feature_settings);
    let before = font_source_generation(database, face_id);
    if let Some(run) = cached_shaped_run_record_for_generation(database, &key, &before) {
        return Some(run.advance);
    }
    let (width, full_metrics) =
        shape_run_for_cache(database, face_id, text, font, font_feature_settings)?;
    let after = font_source_generation(database, face_id);
    if before != after {
        return None;
    }
    cache_run_after_generation(database, key, width, full_metrics, after);
    Some(width)
}

fn shape_run_for_cache(
    database: &Arc<usvg::fontdb::Database>,
    face_id: usvg::fontdb::ID,
    text: &str,
    font: FontShape,
    font_feature_settings: Option<&str>,
) -> Option<(f32, Option<super::harfrust::SvgTextRunMetrics>)> {
    let full_metrics = cacheable_svg_metrics(database, face_id, text, font, font_feature_settings);
    let width = full_metrics.map_or_else(
        || shape_text_with_face_with_trim(database, face_id, text, font, font_feature_settings),
        |metrics| Some(metrics.advance),
    )?;
    Some((width, full_metrics))
}

fn cacheable_svg_metrics(
    database: &Arc<usvg::fontdb::Database>,
    face_id: usvg::fontdb::ID,
    text: &str,
    font: FontShape,
    settings: Option<&str>,
) -> Option<super::harfrust::SvgTextRunMetrics> {
    (settings.is_none() && !super::features::has_han_candidate(text)).then(|| {
        super::harfrust::shape_svg_run_metrics_with_face(database, face_id, text, font, false)
            .map(|(metrics, _)| metrics)
    })?
}

fn cache_run_after_generation(
    database: &Arc<usvg::fontdb::Database>,
    key: ShapedRunKey,
    width: f32,
    full_metrics: Option<super::harfrust::SvgTextRunMetrics>,
    generation: FontSourceGeneration,
) {
    if !generation.reusable() {
        return;
    }
    if let Some(metrics) = full_metrics {
        cache_shaped_run_record(
            database,
            key,
            CachedShapedRun {
                advance: metrics.advance,
                svg_endpoints: Some((metrics.ascent, metrics.descent)),
                ink_bounds: metrics.ink_bounds,
            },
            generation,
        );
    } else {
        cache_shaped_run_width(database, key, width, generation);
    }
}

#[cfg(test)]
fn cached_shaped_run_width(
    database: &Arc<usvg::fontdb::Database>,
    key: &ShapedRunKey,
) -> Option<f32> {
    cached_shaped_run_record(database, key).map(|run| run.advance)
}

#[cfg(test)]
fn cached_shaped_run_record(
    database: &Arc<usvg::fontdb::Database>,
    key: &ShapedRunKey,
) -> Option<CachedShapedRun> {
    let generation = font_source_generation(database, key.face_id);
    if !generation.reusable() {
        return None;
    }
    cached_shaped_run_record_for_generation(database, key, &generation)
}

#[path = "svg_rasterize_text_shaping_cache_store.rs"]
mod store;
use store::{
    cache_shaped_run_record, cache_shaped_run_width, cached_shaped_run_record_for_generation,
};
#[cfg(test)]
use store::{evict_shaped_runs_for_insertion, shaped_run_cache_entry};

#[path = "svg_rasterize_text_shaping_cache_svg.rs"]
mod svg;
pub(super) use svg::cached_svg_run;
#[cfg(test)]
use svg::validate_and_cache_svg_run;

#[cfg(test)]
mod tests {
    use super::super::{FontShape, font::matching_font_face};
    use super::{
        MAX_CACHED_SHAPED_RUN_BYTES, MAX_CACHED_SHAPED_RUN_DATABASES, MAX_CACHED_SHAPED_RUNS,
        SHAPED_RUN_CACHE, ShapedRunKey, cache_shaped_run_width, cached_shaped_run,
        cached_shaped_run_record, shaped_run_cache_entry,
    };
    use crate::markdown::svg_rasterize::font::bundled_font_db;
    use resvg::usvg;
    use std::sync::Arc;

    include!("svg_rasterize_text_shaping_cache_svg_metrics_tests.rs");

    #[test]
    fn repeated_run_measurements_reuse_the_cached_shaped_width() -> Result<(), String> {
        let database = bundled_font_db();
        let face_id = matching_font_face(&database, "Noto Sans", 400, false)
            .ok_or("bundled Noto Sans must be available")?;
        let text = "性能回帰フィクスチャ";
        let first = cached_shaped_run(
            &database,
            face_id,
            text,
            FontShape::new(16.0, 400, false),
            None,
        )
        .ok_or("the bundled font must shape the run")?;
        let second = cached_shaped_run(
            &database,
            face_id,
            text,
            FontShape::new(16.0, 400, false),
            None,
        )
        .ok_or("the cached run must remain available")?;

        assert_eq!(first, second);
        SHAPED_RUN_CACHE.with(|cache| {
            assert!(cache.borrow().iter().any(|entry| !entry.values.is_empty()));
        });
        Ok(())
    }

    #[test]
    fn default_width_cache_retains_svg_geometry_for_the_same_shaped_run() -> Result<(), String> {
        let database = bundled_font_db();
        let face = matching_font_face(&database, "Noto Sans", 400, false)
            .ok_or("bundled Noto Sans must be available")?;
        let font = FontShape::new(16.0, 400, false);
        SHAPED_RUN_CACHE.with(|cache| cache.borrow_mut().clear());
        cached_shaped_run(&database, face, "ink", font, None)
            .ok_or("the default run must shape")?;
        let key = ShapedRunKey::new(face, "ink", font, None);
        let cached =
            cached_shaped_run_record(&database, &key).ok_or("the default run must be cached")?;
        assert!(cached.svg_endpoints.is_some());
        assert!(cached.ink_bounds.is_some());
        Ok(())
    }

    #[test]
    fn cached_width_keeps_distinct_text_and_kerning_settings_separate() -> Result<(), String> {
        let database = bundled_font_db();
        let face = matching_font_face(&database, "Noto Sans", 400, false)
            .ok_or("bundled Noto Sans must be available")?;
        let font = FontShape::new(16.0, 400, false);
        let kerned = cached_shaped_run(&database, face, "AV", font, None).ok_or("AV must shape")?;
        let unkerned = cached_shaped_run(&database, face, "AV", font, Some("'kern' 0"))
            .ok_or("unkerned AV must shape")?;
        let narrow = cached_shaped_run(&database, face, "II", font, None).ok_or("II must shape")?;
        assert!(unkerned > kerned);
        assert!(narrow < kerned);
        assert_eq!(
            cached_shaped_run(&database, face, "AV", font, None),
            Some(kerned)
        );
        Ok(())
    }

    #[test]
    fn stale_font_generation_is_rejected_and_replaced() {
        super::generation_tests::assert_file_replacement_rejects_all_stale_runs();
    }

    #[test]
    fn removed_font_sources_do_not_reuse_cached_widths() -> Result<(), String> {
        let mut database = Arc::new((*bundled_font_db()).clone());
        let face = matching_font_face(&database, "Noto Sans", 400, false)
            .ok_or("bundled font must exist")?;
        let font = FontShape::new(16.0, 400, false);
        assert!(cached_shaped_run(&database, face, "AV", font, None).is_some());
        Arc::make_mut(&mut database).remove_face(face);
        assert!(cached_shaped_run(&database, face, "AV", font, None).is_none());
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
            let key = shaped_run_key(index);
            let generation = super::font_source_generation(&database, key.face_id);
            cache_shaped_run_width(&database, key, index as f32, generation);
        }
        let inserted_key = shaped_run_key(MAX_CACHED_SHAPED_RUNS);

        let generation = super::font_source_generation(&database, inserted_key.face_id);
        cache_shaped_run_width(&database, inserted_key.clone(), 1.0, generation);

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
        assert_width_capacity_state(cache_state)?;
        Ok(())
    }

    #[test]
    fn shaped_run_cache_skips_a_run_larger_than_the_byte_budget() {
        let database = Arc::new(usvg::fontdb::Database::new());
        SHAPED_RUN_CACHE.with(|cache| cache.borrow_mut().clear());
        let key = shaped_run_key_with_text('x', MAX_CACHED_SHAPED_RUN_BYTES + 1);

        let generation = super::font_source_generation(&database, key.face_id);
        cache_shaped_run_width(&database, key, 1.0, generation);

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
    fn empty_cache_entry_stops_eviction_when_insertion_cannot_fit() {
        let database = Arc::new(usvg::fontdb::Database::new());
        let mut entries = Vec::new();
        let entry = shaped_run_cache_entry(&mut entries, &database);

        super::evict_shaped_runs_for_insertion(entry, MAX_CACHED_SHAPED_RUN_BYTES + 1);

        assert_eq!(entries[0].retained_bytes, 0);
        assert!(entries[0].values.is_empty());
    }

    #[test]
    fn shaped_run_cache_evicts_entries_to_fit_the_byte_budget() -> Result<(), String> {
        let database = Arc::new(usvg::fontdb::Database::new());
        SHAPED_RUN_CACHE.with(|cache| cache.borrow_mut().clear());
        let retained_text_length = MAX_CACHED_SHAPED_RUN_BYTES / 2 + 1;
        let first_key = shaped_run_key_with_text('a', retained_text_length);
        let second_key = shaped_run_key_with_text('b', retained_text_length);

        let generation = super::font_source_generation(&database, first_key.face_id);
        cache_shaped_run_width(&database, first_key, 1.0, generation);
        let generation = super::font_source_generation(&database, second_key.face_id);
        cache_shaped_run_width(&database, second_key.clone(), 2.0, generation);

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
        assert_byte_budget_state(cache_state)?;
        Ok(())
    }

    fn assert_width_capacity_state(cache_state: Option<(usize, bool)>) -> Result<(), String> {
        let (cached_width_count, contains_inserted_key) =
            cache_state.ok_or("shaped run cache entry was not created")?;
        assert_eq!(cached_width_count, MAX_CACHED_SHAPED_RUNS);
        assert!(contains_inserted_key);
        Ok(())
    }

    fn assert_byte_budget_state(cache_state: Option<(usize, bool, usize)>) -> Result<(), String> {
        let (cached_width_count, contains_second_key, retained_bytes) =
            cache_state.ok_or("shaped run cache entry was not created")?;
        assert_eq!(cached_width_count, 1);
        assert!(contains_second_key);
        assert!(retained_bytes <= MAX_CACHED_SHAPED_RUN_BYTES);
        Ok(())
    }

    fn shaped_run_key_with_text(fill: char, length: usize) -> ShapedRunKey {
        ShapedRunKey::new(
            usvg::fontdb::ID::default(),
            &fill.to_string().repeat(length),
            FontShape::new(16.0, 400, false),
            None,
        )
    }

    fn shaped_run_key(index: usize) -> ShapedRunKey {
        ShapedRunKey::new(
            usvg::fontdb::ID::default(),
            &format!("run-{index}"),
            FontShape::new(16.0, 400, false),
            None,
        )
    }
}
