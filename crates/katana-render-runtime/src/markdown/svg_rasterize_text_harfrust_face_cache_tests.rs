use super::{FontFaceCacheScope, font_for_face};
use resvg::usvg::fontdb::{Database, Source};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const MAX_SOURCE_BYTES: usize = 64 * 1024 * 1024;
const SHAPER_SCALE: f32 = 65_536.0;
const VARIATION_SIZE: f32 = 18.25;
const VARIATION_WEIGHT: u16 = 700;
const SAMPLE_SIZES: [f32; 3] = [12.0, 16.0, 21.0];

fn temp_font_path(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map_or(0, |duration| duration.as_nanos());
    std::env::temp_dir().join(format!(
        "krr-font-cache-{label}-{}-{stamp}.ttf",
        std::process::id()
    ))
}

fn file_database(path: &Path) -> Result<(Database, resvg::usvg::fontdb::ID), String> {
    let mut database = Database::new();
    database.load_font_source(Source::File(path.to_path_buf()));
    let face = database.faces().next().ok_or("file face missing")?.id;
    Ok((database, face))
}

fn binary_database() -> Result<(Database, resvg::usvg::fontdb::ID), String> {
    let mut database = Database::new();
    database.load_font_data(FONT_BYTES.to_vec());
    let face = database.faces().next().ok_or("binary face missing")?.id;
    Ok((database, face))
}

fn shape_width(font: &harfrust::font::Font, size: f32, text: &str) -> Result<f32, String> {
    let shaper = harfrust::ShaperFont::new(font).with_scale((size * SHAPER_SCALE) as i32);
    let mut buffer = harfrust::Buffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();
    harfrust::shape(&shaper, &mut buffer, harfrust::ShapeOptions::default())
        .map_err(|error| error.to_string())?;
    Ok(buffer
        .glyph_positions()
        .iter()
        .map(|position| position.x_advance as f32 / SHAPER_SCALE)
        .sum())
}

fn assert_variation_metrics(font: &harfrust::font::Font) -> Result<(), String> {
    let shape =
        super::super::super::variations::FontShape::new(VARIATION_SIZE, VARIATION_WEIGHT, true);
    let uncached = harfrust::font::Font::new(FONT_BYTES.to_vec(), 0)
        .ok_or("uncached variation face missing")?;
    let cached_metrics = super::super::shape_font(font, "AV漢字", shape, Some("kern"))
        .and_then(|buffer| {
            super::super::collect_character_metrics(&buffer, "AV漢字", 1.0 / SHAPER_SCALE)
        })
        .ok_or("cached variation metrics missing")?;
    let uncached_metrics = super::super::shape_font(&uncached, "AV漢字", shape, Some("kern"))
        .and_then(|buffer| {
            super::super::collect_character_metrics(&buffer, "AV漢字", 1.0 / SHAPER_SCALE)
        })
        .ok_or("uncached variation metrics missing")?;
    assert_eq!(cached_metrics.len(), uncached_metrics.len());
    for ((cached_advance, cached_offset), (uncached_advance, uncached_offset)) in
        cached_metrics.iter().zip(uncached_metrics.iter())
    {
        assert!((cached_advance - uncached_advance).abs() < 0.001);
        assert!((cached_offset - uncached_offset).abs() < 0.001);
    }
    Ok(())
}

#[test]
fn binary_face_reuse_preserves_shaping_across_font_inputs() -> Result<(), String> {
    let (database, face) = binary_database()?;
    let scope = FontFaceCacheScope::enter();
    let first = font_for_face(&database, face).ok_or("binary face missing from cache")?;
    assert_cached_widths(&database, face, &first)?;
    assert_variation_metrics(&first)?;
    drop(scope);
    Ok(())
}

fn assert_cached_widths(
    database: &Database,
    face: resvg::usvg::fontdb::ID,
    first: &harfrust::font::Font,
) -> Result<(), String> {
    for (size, text) in SAMPLE_SIZES.into_iter().zip(["AV", "漢字", "office"]) {
        let cached = font_for_face(database, face).ok_or("cached binary face missing")?;
        let expected = shape_width(first, size, text)?;
        let actual = shape_width(&cached, size, text)?;
        assert!(
            (expected - actual).abs() < 0.001,
            "width changed for {text}"
        );
    }
    Ok(())
}

#[test]
fn font_lookup_outside_scope_does_not_retain_real_face() -> Result<(), String> {
    let (database, face) = binary_database()?;
    let scope = FontFaceCacheScope::enter();
    assert!(font_for_face(&database, face).is_some());
    drop(scope);
    assert!(font_for_face(&database, face).is_some());
    assert!(super::ACTIVE_SCOPE.with(|cache| {
        let cache = cache.borrow();
        cache.depth == 0 && cache.entry.is_none()
    }));
    Ok(())
}

#[test]
fn shared_file_face_is_not_retained_by_render_cache() -> Result<(), String> {
    let mut database = Database::new();
    database.load_font_data(FONT_BYTES.to_vec());
    let mut info = database.faces().next().ok_or("font face missing")?.clone();
    info.source = Source::SharedFile(
        "shared-cache-test.ttf".into(),
        Arc::new(FONT_BYTES.to_vec()),
    );
    let mut database = Database::new();
    database.push_face_info(info);
    let face = database.faces().next().ok_or("shared face missing")?.id;
    let _scope = FontFaceCacheScope::enter();
    assert!(font_for_face(&database, face).is_some());
    assert!(super::ACTIVE_SCOPE.with(|cache| cache.borrow().entry.is_none()));
    assert!(font_for_face(&database, face).is_some());
    assert!(super::ACTIVE_SCOPE.with(|cache| cache.borrow().entry.is_none()));
    Ok(())
}

#[test]
fn oversize_binary_source_is_not_retained() -> Result<(), String> {
    let mut bytes = Vec::with_capacity(MAX_SOURCE_BYTES + FONT_BYTES.len());
    bytes.extend_from_slice(FONT_BYTES);
    bytes.resize(MAX_SOURCE_BYTES + 1, 0);
    let mut database = Database::new();
    database.load_font_data(bytes);
    let face = database
        .faces()
        .next()
        .ok_or("oversize binary face missing")?
        .id;
    let _scope = FontFaceCacheScope::enter();
    assert!(font_for_face(&database, face).is_some());
    assert!(super::ACTIVE_SCOPE.with(|cache| cache.borrow().entry.is_none()));
    Ok(())
}

#[test]
fn nested_scopes_share_the_active_face_entry_and_cleanup() -> Result<(), String> {
    let (database, face) = binary_database()?;
    let outer = FontFaceCacheScope::enter();
    let outer_font = font_for_face(&database, face).ok_or("outer face missing")?;
    let outer_key = super::ACTIVE_SCOPE
        .with(|cache| cache.borrow().entry.as_ref().map(|entry| entry.key.clone()));
    let inner = FontFaceCacheScope::enter();
    let inner_font = font_for_face(&database, face).ok_or("inner face missing")?;
    let inner_key = super::ACTIVE_SCOPE
        .with(|cache| cache.borrow().entry.as_ref().map(|entry| entry.key.clone()));
    assert_eq!(outer_key, inner_key);
    assert!(
        (shape_width(&outer_font, 16.0, "AV")? - shape_width(&inner_font, 16.0, "AV")?).abs()
            < 0.001
    );
    drop(inner);
    drop(outer);
    assert!(super::ACTIVE_SCOPE.with(|cache| {
        let cache = cache.borrow();
        cache.depth == 0 && cache.entry.is_none()
    }));
    Ok(())
}

#[test]
fn missing_face_clears_previous_real_entry() -> Result<(), String> {
    let (database, face) = binary_database()?;
    let mut other = Database::new();
    let scope = FontFaceCacheScope::enter();
    assert!(font_for_face(&database, face).is_some());
    assert!(font_for_face(&other, face).is_none());
    assert!(super::ACTIVE_SCOPE.with(|cache| cache.borrow().entry.is_none()));
    other.load_font_data(FONT_BYTES.to_vec());
    let replacement = other.faces().next().ok_or("replacement face missing")?.id;
    assert!(font_for_face(&other, replacement).is_some());
    drop(scope);
    Ok(())
}

#[path = "svg_rasterize_text_harfrust_face_cache_file_tests.rs"]
mod file_tests;
use file_tests::{
    BACKED_FONT_COUNT, FIRST_BACKED_FONT_BYTES, FIRST_BACKED_FONT_COUNT, FontHandle, FontShape,
    NEXT_BACKED_FONT_BYTES, POISON_WEIGHT, SFNT_NUM_TABLES_FIELD_OFFSET,
    SFNT_TABLE_DIRECTORY_START, SFNT_TABLE_LENGTH_FIELD_END, SFNT_TABLE_LENGTH_FIELD_START,
    SFNT_TABLE_OFFSET_FIELD_END, SFNT_TABLE_OFFSET_FIELD_START, SFNT_TABLE_RECORD_SIZE,
    SFNT_TAG_BYTE_LENGTH, TRUNCATION_DIVISOR, TTC_SIZE, TTC_TEXT, TTC_WEIGHT,
    assert_all_handles_cleared, font_source_generation, shape_and_commit, shape_font,
};

#[path = "svg_rasterize_text_harfrust_face_cache_file_budget_tests.rs"]
mod file_budget_tests;
#[path = "svg_rasterize_text_harfrust_face_cache_file_generation_failure_tests.rs"]
mod file_generation_failure_tests;
#[path = "svg_rasterize_text_harfrust_face_cache_file_mutation_tests.rs"]
mod file_mutation_tests;
#[path = "svg_rasterize_text_harfrust_face_cache_file_reuse_tests.rs"]
mod file_reuse_tests;
#[path = "svg_rasterize_text_harfrust_face_cache_file_ttc_tests.rs"]
mod file_ttc_tests;

#[cfg(unix)]
#[path = "svg_rasterize_text_harfrust_face_cache_generation_race_tests.rs"]
mod generation_race_tests;

#[test]
#[cfg(unix)]
fn cached_file_rechecks_uncached_generation_before_returning()
-> Result<(), Box<dyn std::error::Error>> {
    let path = temp_font_path("cached-clean");
    std::fs::write(&path, FONT_BYTES)?;
    let (database, face) = file_database(&path)?;
    let mut calls = 0;
    let result = super::super::super::font::with_validated_stamp_batch(|| {
        calls += 1;
        check_cached_file_generation(&database, face, &path, calls)
    });
    std::fs::remove_file(path)?;
    result?;
    assert_eq!(calls, 2);
    Ok(())
}

#[cfg(unix)]
fn check_cached_file_generation(
    database: &Database,
    face: resvg::usvg::fontdb::ID,
    path: &Path,
    calls: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let _scope = FontFaceCacheScope::enter();
    let first = font_for_face(database, face).ok_or("real cached file missing")?;
    assert!(super::commit_file_font(database, face, &first));
    if calls == 1 {
        std::fs::write(path, [FONT_BYTES, &[0_u8][..]].concat())?;
        assert!(font_for_face(database, face).is_none());
        assert!(super::ACTIVE_SCOPE.with(|cache| cache.borrow().entry.is_none()));
    }
    Ok(())
}

#[path = "svg_rasterize_text_harfrust_face_cache_frame_scope_tests.rs"]
mod frame_scope_tests;
