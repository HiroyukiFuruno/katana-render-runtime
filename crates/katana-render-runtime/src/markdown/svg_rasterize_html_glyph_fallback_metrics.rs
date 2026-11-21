use super::{PreparedSpan, eligibility, scripts::ScriptFragment};
use crate::markdown::svg_rasterize::text_shaping::{
    FontShape, shape_run_character_metrics_with_trim,
};
use resvg::usvg;

pub(super) fn measure_fragment(
    database: &usvg::fontdb::Database,
    span: &PreparedSpan,
    fragment: &ScriptFragment,
    glyphs: &[usvg::layout::PositionedGlyph],
    font_shape: FontShape,
) -> Option<Vec<(f32, f32)>> {
    let metrics = shape_run_character_metrics_with_trim(
        database,
        span.font,
        &fragment.text,
        font_shape,
        None,
    )?;
    (metrics.len() == glyphs.len()
        && eligibility::glyph_ids_match_cmap(database, span.font, &fragment.text, glyphs))
    .then_some(metrics)
}
