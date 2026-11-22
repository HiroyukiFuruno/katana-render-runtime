use super::super::features::features_for_face;
use super::buffer::shape_font_with_features_and_variations;
use super::{FontShape, HARFRUST_SCALE, committed_character_metrics, face_cache, is_left_to_right};
use resvg::usvg::{FontVariation, fontdb};

#[cfg(any(target_os = "macos", test))]
pub(in crate::markdown::svg_rasterize::text_shaping) fn shape_text_with_face_variations_with_trim(
    database: &fontdb::Database,
    face_id: fontdb::ID,
    text: &str,
    font: FontShape,
    font_feature_settings: Option<&str>,
    variations: &[FontVariation],
) -> Option<f32> {
    let glyphs = shape_database_face(
        database,
        face_id,
        text,
        font,
        font_feature_settings,
        variations,
    )?;
    Some(
        glyphs
            .glyph_positions()
            .iter()
            .map(|position| position.x_advance as f32 / HARFRUST_SCALE)
            .sum(),
    )
}

#[cfg(any(target_os = "macos", test))]
pub(in crate::markdown::svg_rasterize::text_shaping) fn shape_run_character_metrics_with_variations_with_trim(
    database: &fontdb::Database,
    face_id: fontdb::ID,
    text: &str,
    font: FontShape,
    font_feature_settings: Option<&str>,
    variations: &[FontVariation],
) -> Option<Vec<(f32, f32)>> {
    let features = features_for_face(database, face_id, text, font_feature_settings, font.trim)?;
    shape_character_metrics(database, face_id, text, font, variations, &features)
}

#[cfg(target_os = "macos")]
pub(in crate::markdown::svg_rasterize::text_shaping) fn shape_run_character_metrics_without_features_with_variations(
    database: &fontdb::Database,
    face_id: fontdb::ID,
    text: &str,
    font: FontShape,
    variations: &[FontVariation],
) -> Option<Vec<(f32, f32)>> {
    shape_character_metrics(database, face_id, text, font, variations, &[])
}

#[cfg(any(target_os = "macos", test))]
fn shape_database_face(
    database: &fontdb::Database,
    face_id: fontdb::ID,
    text: &str,
    font: FontShape,
    font_feature_settings: Option<&str>,
    variations: &[FontVariation],
) -> Option<harfrust::Buffer> {
    let face = face_cache::font_for_face(database, face_id)?;
    let features = features_for_face(database, face_id, text, font_feature_settings, font.trim)?;
    let glyphs = shape_font_with_features_and_variations(&face, text, font, &features, variations)?;
    face_cache::commit_file_font(database, face_id, &face).then_some(glyphs)
}

#[cfg(any(target_os = "macos", test))]
fn shape_character_metrics(
    database: &fontdb::Database,
    face_id: fontdb::ID,
    text: &str,
    font: FontShape,
    variations: &[FontVariation],
    features: &[harfrust::Feature],
) -> Option<Vec<(f32, f32)>> {
    if !is_left_to_right(text) {
        return None;
    }
    let face = face_cache::font_for_face(database, face_id)?;
    let glyphs = shape_font_with_features_and_variations(&face, text, font, features, variations)?;
    committed_character_metrics(database, face_id, &face, &glyphs, text)
}
