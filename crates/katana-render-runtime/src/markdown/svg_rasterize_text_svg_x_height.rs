use super::svg_font_instance::svg_font_instance;
use super::{FontShape, face_cache};
use harfrust::font::metrics::ScaleF32;
use resvg::usvg;

pub(in super::super) fn svg_run_x_height_with_face(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    font: FontShape,
) -> Option<Option<f32>> {
    let (face, instance) = svg_font_instance(database, face_id, font).ok()?;
    let metrics = instance.metrics();
    /* WHY: Harfrustが利用するread-fontsは無効なunitsPerEmを1000へ正規化する。 */
    let x_height = metrics
        .scaled(ScaleF32::from_ppem(font.size, metrics.units_per_em))
        .x_height()
        .or_else(|| glyph_x_height(&instance, font.size));
    if x_height.is_some_and(|value| !value.is_finite() || value <= 0.0) {
        return None;
    }
    face_cache::commit_file_font(database, face_id, &face).then_some(x_height)
}

fn glyph_x_height(instance: &harfrust::font::Font, size: f32) -> Option<f32> {
    let glyph = instance.charmap().map_unicode('x')?;
    let extents = instance
        .glyph_metrics()
        .scaled(ScaleF32::from_ppem(size, instance.metrics().units_per_em))
        .extents(glyph)?;
    /* WHY: OS/2に値がないfaceは同じvariationsのlowercase x輪郭高を使う。 */
    (extents.y_bearing.is_finite() && extents.y_bearing > 0.0).then_some(extents.y_bearing)
}
