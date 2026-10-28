use super::super::super::features::features_for_face;
use super::{
    FontShape, HARFRUST_SCALE, SvgTextRunMetrics, face_cache, font_variations,
    shape_buffer_with_features, svg_run_ink_diagnostic, svg_run_line_metrics, svg_text_run_metrics,
    validate_shaping_size,
};
use resvg::usvg;

type FragmentRun = (SvgTextRunMetrics, Vec<f32>);

pub(in crate::markdown::svg_rasterize) fn shape_fragment_run(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    text: &str,
    font: FontShape,
) -> Result<Option<FragmentRun>, String> {
    if !super::super::is_left_to_right(text) {
        return Ok(None);
    }
    validate_shaping_size(font.size).ok_or("invalid SVG shaping size")?;
    let face = face_cache::font_for_face(database, face_id).ok_or("SVG font face unavailable")?;
    let instance = face
        .instance_builder()
        .variations(font_variations(font))
        .build();
    let result = measure_instance(database, face_id, text, font, &instance)?;
    if !face_cache::commit_file_font(database, face_id, &face) {
        return Err("SVG font source changed during shaping".into());
    }
    Ok(result)
}

fn measure_instance(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    text: &str,
    font: FontShape,
    instance: &harfrust::font::Font,
) -> Result<Option<FragmentRun>, String> {
    let (ascent, descent) =
        svg_run_line_metrics(instance, font.size).ok_or("SVG font line metrics unavailable")?;
    let features = features_for_face(database, face_id, text, None, font.trim)
        .ok_or("SVG font features unavailable")?;
    let glyphs = shape_buffer_with_features(instance, text, font, &features)
        .ok_or("SVG font shaping failed")?;
    let metrics =
        svg_text_run_metrics(&glyphs, ascent, descent).ok_or("invalid SVG run metrics")?;
    let ink = svg_run_ink_diagnostic(instance, &glyphs, font.size);
    let advances = ordered_scalar_advances(text, &glyphs, instance, font.size);
    Ok(advances.map(|advances| {
        (
            SvgTextRunMetrics {
                ink_bounds: ink.bounds,
                ..metrics
            },
            advances,
        )
    }))
}

fn ordered_scalar_advances(
    text: &str,
    glyphs: &harfrust::Buffer,
    instance: &harfrust::font::Font,
    size: f32,
) -> Option<Vec<f32>> {
    let infos = glyphs.glyph_infos();
    let positions = glyphs.glyph_positions();
    if infos.len() != positions.len() || infos.len() != text.chars().count() {
        return None;
    }
    let shaper = harfrust::ShaperFont::new(instance).with_scale((size * HARFRUST_SCALE) as i32);
    text.char_indices()
        .zip(infos.iter().zip(positions))
        .map(|((offset, character), (info, position))| {
            let advance = position.x_advance as f32 / HARFRUST_SCALE;
            (info.glyph_id != 0
                && info.cluster as usize == offset
                && position.y_advance == 0
                && advance.is_finite()
                && advance >= 0.0
                && (character.is_whitespace()
                    || shaper
                        .glyph_extents(harfrust::GlyphId::new(info.glyph_id))
                        .is_some()))
            .then_some(advance)
        })
        .collect()
}
