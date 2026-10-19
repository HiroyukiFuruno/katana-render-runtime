use super::features::harfrust_features;
use super::variations::FontShape;
use resvg::usvg;

#[path = "svg_rasterize_text_harfrust_face_cache.rs"]
pub(super) mod face_cache;

#[path = "svg_rasterize_text_harfrust_line_metrics.rs"]
mod line_metrics;
pub(super) use line_metrics::{
    cached_face_line_metrics, diagnosed_cached_face_line_metrics, parsed_face_line_metrics,
};

#[path = "svg_rasterize_text_svg_run_metrics.rs"]
mod svg_run_metrics;
pub(crate) use svg_run_metrics::{SvgInkBounds, SvgTextRunMetrics};
pub(super) use svg_run_metrics::{shape_svg_run_metrics_with_face, svg_run_endpoints_with_face};

const HARFRUST_SCALE: f32 = 65_536.0;
const FONT_VARIATION_COUNT: usize = 3;

#[cfg(test)]
#[path = "svg_rasterize_text_harfrust_source_tests.rs"]
mod source_tests;

#[cfg(test)]
#[path = "svg_rasterize_text_harfrust_fragment_tests.rs"]
mod fragment_tests;

#[cfg(test)]
#[path = "svg_rasterize_text_harfrust_combined_metrics_probe.rs"]
mod combined_metrics_probe;

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_text_harfrust_tree_diagnostic_tests.rs"]
mod tree_diagnostic;

pub(super) fn shape_text_with_face(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    text: &str,
    font: FontShape,
    font_feature_settings: Option<&str>,
) -> Option<f32> {
    let glyphs = shape_database_face(database, face_id, text, font, font_feature_settings)?;
    Some(
        glyphs
            .glyph_positions()
            .iter()
            .map(|position| position.x_advance as f32 / HARFRUST_SCALE)
            .sum::<f32>(),
    )
}

pub(super) fn shape_run_character_metrics(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    text: &str,
    font: FontShape,
    font_feature_settings: Option<&str>,
) -> Option<Vec<(f32, f32)>> {
    if !is_left_to_right(text) {
        return None;
    }
    let face = face_cache::font_for_face(database, face_id)?;
    let glyphs = shape_font(&face, text, font, font_feature_settings)?;
    let metrics = collect_character_metrics(&glyphs, text, 1.0 / HARFRUST_SCALE);
    if metrics.is_some() && !face_cache::commit_file_font(database, face_id, &face) {
        return None;
    }
    metrics
}

fn shape_database_face(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    text: &str,
    font: FontShape,
    font_feature_settings: Option<&str>,
) -> Option<harfrust::Buffer> {
    let face = face_cache::font_for_face(database, face_id)?;
    let glyphs = shape_font(&face, text, font, font_feature_settings)?;
    face_cache::commit_file_font(database, face_id, &face).then_some(glyphs)
}

#[cfg(test)]
pub(super) fn shape_face_character_metrics(
    data: &[u8],
    face_index: u32,
    text: &str,
    font: FontShape,
    font_feature_settings: Option<&str>,
) -> Option<Vec<(f32, f32)>> {
    is_left_to_right(text).then(|| {
        let face = harfrust::font::Font::new(data.to_vec(), face_index)?;
        let glyphs = shape_font(&face, text, font, font_feature_settings)?;
        collect_character_metrics(&glyphs, text, 1.0 / HARFRUST_SCALE)
    })?
}

fn shape_font(
    face: &harfrust::font::Font,
    text: &str,
    font: FontShape,
    font_feature_settings: Option<&str>,
) -> Option<harfrust::Buffer> {
    let instance = face
        .instance_builder()
        .variations(font_variations(font))
        .build();
    shape_buffer(&instance, text, font, font_feature_settings)
}

fn is_left_to_right(text: &str) -> bool {
    let mut buffer = harfrust::Buffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();
    buffer.direction() == harfrust::Direction::LeftToRight
}

fn font_variations(font: FontShape) -> [(&'static str, f32); FONT_VARIATION_COUNT] {
    [
        ("wght", f32::from(font.weight)),
        ("opsz", font.size),
        ("ital", if font.italic { 1.0 } else { 0.0 }),
    ]
}

fn shape_buffer(
    face: &harfrust::font::Font,
    text: &str,
    font: FontShape,
    font_feature_settings: Option<&str>,
) -> Option<harfrust::Buffer> {
    use harfrust::{Buffer, ShapeOptions, ShaperFont};
    let shaper = ShaperFont::new(face).with_scale((font.size * HARFRUST_SCALE) as i32);
    let mut buffer = Buffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();
    harfrust::shape(
        &shaper,
        &mut buffer,
        ShapeOptions::new()
            .point_size(Some(font.size))
            .features(&harfrust_features(font_feature_settings)),
    )
    .ok()?;
    Some(buffer)
}

fn collect_character_metrics(
    glyphs: &harfrust::Buffer,
    text: &str,
    scale: f32,
) -> Option<Vec<(f32, f32)>> {
    collect_glyph_character_metrics(text, scale, || {
        glyphs
            .glyph_infos()
            .iter()
            .zip(glyphs.glyph_positions())
            .map(|(info, position)| (info.cluster, position.x_advance, position.x_offset))
    })
}

pub(super) fn collect_glyph_character_metrics<I>(
    text: &str,
    scale: f32,
    glyphs: impl FnOnce() -> I,
) -> Option<Vec<(f32, f32)>>
where
    I: Iterator<Item = (u32, i32, i32)>,
{
    let character_starts = text
        .char_indices()
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let mut metrics = vec![(0.0, 0.0); character_starts.len()];
    let mut seen = vec![false; character_starts.len()];
    for (cluster, advance, offset) in glyphs() {
        let character_index = character_starts.binary_search(&(cluster as usize)).ok()?;
        metrics[character_index].0 += advance as f32 * scale;
        if !seen[character_index] {
            metrics[character_index].1 = offset as f32 * scale;
            seen[character_index] = true;
        }
    }
    seen.into_iter().all(|seen| seen).then_some(metrics)
}
