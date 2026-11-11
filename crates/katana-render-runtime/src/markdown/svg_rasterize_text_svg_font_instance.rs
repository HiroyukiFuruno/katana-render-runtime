use super::{FontShape, HARFRUST_SCALE, face_cache, font_variations};
use resvg::usvg;

pub(super) fn svg_font_instance(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    font: FontShape,
) -> Result<(face_cache::FontHandle, harfrust::font::Font), &'static str> {
    validate_shaping_size(font.size).ok_or("invalid SVG shaping size")?;
    let face = face_cache::font_for_face(database, face_id).ok_or("SVG font face unavailable")?;
    let instance = face
        .instance_builder()
        .variations(font_variations(font))
        .build();
    Ok((face, instance))
}

pub(super) fn validate_shaping_size(size: f32) -> Option<()> {
    let shaping_scale = size * HARFRUST_SCALE;
    if !size.is_finite()
        || size <= 0.0
        || !shaping_scale.is_finite()
        || f64::from(shaping_scale) > f64::from(i32::MAX)
        || shaping_scale as i32 == 0
    {
        return None;
    }
    Some(())
}
