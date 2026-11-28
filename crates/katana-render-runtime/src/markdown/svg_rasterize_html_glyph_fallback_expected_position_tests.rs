use super::super::super::super::anchor::trunc_layout_unit;
use super::{MetricsContext, PreparedSpan};

pub(super) fn calculate(
    span: &PreparedSpan,
    glyph: &resvg::usvg::layout::PositionedGlyph,
    offset: f32,
    context: &MetricsContext<'_>,
) -> Result<(f32, f32), String> {
    let start_paint = context.start_paint;
    let origin = if start_paint {
        trunc_layout_unit(context.origin_x).ok_or("expected origin is outside LayoutUnit range")?
    } else {
        context.origin_x
    };
    let offset = (context.cursor + f64::from(offset)) as f32;
    let expected_x = origin
        + if start_paint {
            offset
        } else {
            trunc_layout_unit(offset).ok_or("expected glyph x is outside LayoutUnit range")?
        };
    let y = (glyph.transform().ty + span.baseline_shift) * context.css_scale;
    let expected_y = if start_paint {
        trunc_layout_unit(y).ok_or("expected glyph y is outside LayoutUnit range")?
    } else {
        y
    };
    Ok((expected_x, expected_y))
}
