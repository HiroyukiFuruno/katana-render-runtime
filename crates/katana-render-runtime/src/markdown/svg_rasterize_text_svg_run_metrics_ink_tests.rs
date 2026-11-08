use super::{FontShape, shape_svg_run_metrics_with_face, svg_run_endpoints_with_face};
use resvg::usvg::fontdb::{Database, ID};

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const TEST_TEXT: &str = "Noto Sans";
const VALID_SIZE: f32 = 16.0;
const NORMAL_WEIGHT: u16 = 400;

fn checked_in_font_face() -> Result<(Database, ID), String> {
    let mut database = Database::new();
    database.load_font_data(FONT_BYTES.to_vec());
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
    Ok(())
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
    assert_eq!(plain.0.advance.to_bits(), diagnosed.advance.to_bits());
    assert_eq!(plain.0.ascent.to_bits(), diagnosed.ascent.to_bits());
    assert_eq!(plain.0.descent.to_bits(), diagnosed.descent.to_bits());
    let ink = ink.ok_or("ink diagnostic was not produced")?;
    let bounds = ink.bounds.ok_or("real glyphs had no ink extents")?;
    assert!(bounds.iter().all(|value| value.is_finite()));
    assert!(bounds[0] <= bounds[2] && bounds[1] <= bounds[3]);
    assert_eq!(diagnosed.ink_bounds, Some(bounds));
    /* WHY: counterは実測したHarfrust extents欠如数と一致させる。 */
    assert_eq!(ink.glyphs_without_extents, raw_missing);
    eprintln!("raw_harfrust_glyphs_without_extents={raw_missing}");
    Ok(())
}

fn raw_harfrust_glyphs_without_extents(text: &str, shape: FontShape) -> Result<usize, String> {
    let face = harfrust::font::Font::new(FONT_BYTES.to_vec(), 0)
        .ok_or("raw Noto Sans face could not be parsed")?;
    let instance = face
        .instance_builder()
        .variations(super::font_variations(shape))
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
