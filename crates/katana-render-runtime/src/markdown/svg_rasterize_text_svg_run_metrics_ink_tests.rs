use super::super::font_variations;
use super::super::svg_font_instance::validate_shaping_size;
use super::{FontShape, shape_svg_run_metrics_with_face, svg_run_endpoints_with_face};
use crate::markdown::svg_rasterize::text_shaping::test_font_data::font_with_empty_table;
use resvg::usvg::fontdb::{Database, ID};
use rustybuzz::ttf_parser::Tag;

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const TEST_TEXT: &str = "Noto Sans";
const VALID_SIZE: f32 = 16.0;
const NORMAL_WEIGHT: u16 = 400;
const SPACE: &str = " ";
const FIRST_VISIBLE_CSS_SIZE: f32 = 0.01;
const FIRST_OVERSIZED_SHAPING_SIZE: f32 = 32_768.0;

fn checked_in_font_face() -> Result<(Database, ID), String> {
    font_face(FONT_BYTES.to_vec())
}

fn font_face(data: Vec<u8>) -> Result<(Database, ID), String> {
    let mut database = Database::new();
    database.load_font_data(data);
    let face = database
        .faces()
        .next()
        .ok_or("checked-in Noto Sans face missing")?
        .id;
    Ok((database, face))
}

#[test]
fn invalid_sizes_reject_real_font_run_and_endpoints() -> Result<(), String> {
    let (database, face) = checked_in_font_face()?;
    let valid = FontShape::new(VALID_SIZE, NORMAL_WEIGHT, false);
    assert!(shape_svg_run_metrics_with_face(&database, face, TEST_TEXT, valid, false).is_some());
    assert!(svg_run_endpoints_with_face(&database, face, valid).is_some());
    for size in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::MAX] {
        let invalid = FontShape::new(size, NORMAL_WEIGHT, false);
        assert!(
            shape_svg_run_metrics_with_face(&database, face, TEST_TEXT, invalid, false).is_none()
        );
        assert!(svg_run_endpoints_with_face(&database, face, invalid).is_none());
    }
    assert_adjacent_shaping_size_boundaries();
    assert_adjacent_css_size_boundaries(&database, face);
    Ok(())
}

fn assert_adjacent_shaping_size_boundaries() {
    let minimum = 1.0 / super::HARFRUST_SCALE;
    let below_minimum = f32::from_bits(minimum.to_bits() - 1);
    let below_limit = f32::from_bits(FIRST_OVERSIZED_SHAPING_SIZE.to_bits() - 1);
    assert!(validate_shaping_size(below_minimum).is_none());
    assert!(validate_shaping_size(minimum).is_some());
    assert!(validate_shaping_size(below_limit).is_some());
    assert!(validate_shaping_size(FIRST_OVERSIZED_SHAPING_SIZE).is_none());
}

fn assert_adjacent_css_size_boundaries(database: &Database, face: ID) {
    let previous = f32::from_bits(FIRST_VISIBLE_CSS_SIZE.to_bits() - 1);
    let too_small = FontShape::new(previous, NORMAL_WEIGHT, false);
    let visible = FontShape::new(FIRST_VISIBLE_CSS_SIZE, NORMAL_WEIGHT, false);
    assert!(shape_svg_run_metrics_with_face(database, face, TEST_TEXT, too_small, false).is_none());
    assert!(svg_run_endpoints_with_face(database, face, too_small).is_none());
    assert!(svg_run_endpoints_with_face(database, face, visible).is_some());
}

#[test]
fn foreign_face_id_is_rejected_and_original_database_recovers() -> Result<(), String> {
    let _scope = super::face_cache::FontFaceCacheScope::enter();
    let (database, face) = checked_in_font_face()?;
    let font = FontShape::new(VALID_SIZE, NORMAL_WEIGHT, false);
    let missing = Database::new();
    for (database, expected) in [(&database, true), (&missing, false), (&database, true)] {
        assert_eq!(
            shape_svg_run_metrics_with_face(database, face, TEST_TEXT, font, false).is_some(),
            expected
        );
        assert_eq!(
            svg_run_endpoints_with_face(database, face, font).is_some(),
            expected
        );
    }
    Ok(())
}

#[test]
fn space_run_has_advance_without_ink_and_diagnostics_preserve_metrics() -> Result<(), String> {
    let (database, face) = checked_in_font_face()?;
    let font = FontShape::new(VALID_SIZE, NORMAL_WEIGHT, false);
    let expected_advance = space_advance_from_font()?;
    let plain = shape_svg_run_metrics_with_face(&database, face, SPACE, font, false)
        .ok_or("plain space run unavailable")?;
    let (metrics, diagnostic) = shape_svg_run_metrics_with_face(&database, face, SPACE, font, true)
        .ok_or("diagnostic space run unavailable")?;
    assert!(plain.1.is_none());
    assert_run_metrics_equal(plain.0, metrics);
    assert!((metrics.advance - expected_advance).abs() <= 1.0 / super::HARFRUST_SCALE);
    assert!(metrics.advance > 0.0);
    assert!(metrics.ink_bounds.is_none());
    let diagnostic = diagnostic.ok_or("space diagnostic missing")?;
    assert!(diagnostic.bounds.is_none());
    assert_eq!(
        diagnostic.glyphs_without_extents,
        raw_harfrust_glyphs_without_extents(SPACE, font)?
    );
    Ok(())
}

fn assert_run_metrics_equal(plain: super::SvgTextRunMetrics, diagnosed: super::SvgTextRunMetrics) {
    assert_eq!(plain.advance.to_bits(), diagnosed.advance.to_bits());
    assert_eq!(plain.ascent.to_bits(), diagnosed.ascent.to_bits());
    assert_eq!(plain.descent.to_bits(), diagnosed.descent.to_bits());
}

fn space_advance_from_font() -> Result<f32, String> {
    let face = rustybuzz::ttf_parser::Face::parse(FONT_BYTES, 0)
        .map_err(|error| format!("Noto Sans parse failed: {error:?}"))?;
    let glyph = face
        .glyph_index(' ')
        .ok_or("space glyph missing from cmap")?;
    let advance = face
        .glyph_hor_advance(glyph)
        .ok_or("space advance missing")?;
    assert!(advance > 0);
    assert!(face.glyph_bounding_box(glyph).is_none());
    Ok(f32::from(advance) * VALID_SIZE / f32::from(face.units_per_em()))
}

#[test]
fn ink_diagnostic_preserves_metrics_for_checked_in_font() -> Result<(), String> {
    let (database, face) = checked_in_font_face()?;
    let shape = FontShape::new(18.0, 400, false);
    let raw_missing = raw_harfrust_glyphs_without_extents(TEST_TEXT, shape)?;
    let plain = shape_svg_run_metrics_with_face(&database, face, TEST_TEXT, shape, false)
        .ok_or("plain real-font metrics unavailable")?;
    let (diagnosed, ink) = shape_svg_run_metrics_with_face(&database, face, TEST_TEXT, shape, true)
        .ok_or("diagnostic real-font metrics unavailable")?;
    assert!(plain.1.is_none());
    assert_run_metrics_equal(plain.0, diagnosed);
    let ink = ink.ok_or("ink diagnostic was not produced")?;
    let bounds = ink.bounds.ok_or("real glyphs had no ink extents")?;
    assert!(bounds.iter().all(|value| value.is_finite()));
    assert!(bounds[0] <= bounds[2] && bounds[1] <= bounds[3]);
    assert_eq!(diagnosed.ink_bounds, Some(bounds));
    /* WHY: counterは実測したHarfrust extents欠如数と一致させる。 */
    assert_eq!(ink.glyphs_without_extents, raw_missing);
    eprintln!("raw_harfrust_glyphs_without_extents={raw_missing}");
    assert_missing_outline_is_reported(shape)
}

fn assert_missing_outline_is_reported(shape: FontShape) -> Result<(), String> {
    let data = font_with_empty_table(FONT_BYTES, Tag::from_bytes(b"glyf"))?;
    let (database, face) = font_face(data)?;
    let (metrics, diagnostic) = shape_svg_run_metrics_with_face(&database, face, "N", shape, true)
        .ok_or("font with missing outlines should retain line metrics")?;
    let diagnostic = diagnostic.ok_or("missing-outline diagnostic unavailable")?;
    assert!(metrics.advance > 0.0);
    assert!(metrics.ink_bounds.is_none());
    assert!(diagnostic.bounds.is_none());
    assert_eq!(diagnostic.glyphs_without_extents, 1);
    Ok(())
}

fn raw_harfrust_glyphs_without_extents(text: &str, shape: FontShape) -> Result<usize, String> {
    let face = harfrust::font::Font::new(FONT_BYTES.to_vec(), 0)
        .ok_or("raw Noto Sans face could not be parsed")?;
    let instance = face
        .instance_builder()
        .variations(font_variations(shape))
        .build();
    let shaper = harfrust::ShaperFont::new(&instance)
        .with_scale((shape.size * super::HARFRUST_SCALE) as i32);
    let mut glyphs = harfrust::Buffer::new();
    glyphs.push_str(text);
    glyphs.guess_segment_properties();
    harfrust::shape(
        &shaper,
        &mut glyphs,
        harfrust::ShapeOptions::new().point_size(Some(shape.size)),
    )
    .map_err(|error| error.to_string())?;
    Ok(glyphs
        .glyph_infos()
        .iter()
        .filter(|info| {
            shaper
                .glyph_extents(harfrust::GlyphId::new(info.glyph_id))
                .is_none()
        })
        .count())
}
