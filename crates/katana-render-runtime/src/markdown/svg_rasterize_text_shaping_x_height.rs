use super::harfrust::svg_run_x_height_with_face;
use super::svg_metrics::validated_svg_font_shape;
use super::{FontShape, html_font_db_for_measured_text, matching_font_face};

pub(in crate::markdown::svg_rasterize) fn shaped_svg_font_x_height(
    family: &str,
    size: f32,
    weight: u16,
    italic: bool,
) -> Option<Option<f32>> {
    let font: FontShape = validated_svg_font_shape(size, weight, italic)?;
    let database = html_font_db_for_measured_text(family, "x", font.weight, font.italic);
    let face = matching_font_face(&database, family, font.weight, font.italic)?;
    svg_run_x_height_with_face(&database, face, font)
}
