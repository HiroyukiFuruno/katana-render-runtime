use super::{
    FontShape, HARFRUST_SCALE, SvgTextRunMetrics, face_cache, measure_svg_run, svg_font_instance,
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
    let (face, instance) = svg_font_instance(database, face_id, font)?;
    let run = measure_svg_run(database, face_id, text, font, &instance)?;
    let advances = ordered_scalar_advances(text, &run.glyphs, &instance, font.size);
    if !face_cache::commit_file_font(database, face_id, &face) {
        return Err("SVG font source changed during shaping".into());
    }
    Ok(advances.map(|advances| (run.metrics, advances)))
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
