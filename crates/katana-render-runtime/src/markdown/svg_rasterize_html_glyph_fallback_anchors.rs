use super::RunLayout;
use resvg::usvg;

pub(super) fn apply(
    anchor: usvg::TextAnchor,
    advance: f64,
    layouts: &mut [RunLayout],
) -> Option<()> {
    let shift = anchor_shift(anchor, advance)?;
    for position in layouts
        .iter_mut()
        .flat_map(|layout| &mut layout.css_positions)
    {
        let x = position.x + shift;
        super::super::anchor::trunc_layout_unit(x)?;
        position.x = x;
    }
    Some(())
}

fn anchor_shift(anchor: usvg::TextAnchor, advance: f64) -> Option<f32> {
    if !advance.is_finite() || advance <= 0.0 {
        return None;
    }
    let width = super::super::chunk::ceil_layout_unit(advance);
    let factor = match anchor {
        usvg::TextAnchor::Start => 0.0,
        usvg::TextAnchor::Middle => super::super::anchor::MIDDLE_ANCHOR_FACTOR,
        usvg::TextAnchor::End => -1.0,
    };
    let shift = (width * factor) as f32;
    super::super::anchor::trunc_layout_unit(width as f32)?;
    super::super::anchor::trunc_layout_unit(shift).map(|_| shift)
}
