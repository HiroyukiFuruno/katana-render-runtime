use super::PreparedSpan;
use resvg::usvg::{self, Transform};

const LAYOUT_UNIT_SCALE: f64 = 64.0;
const START_ANCHOR_FACTOR: f64 = 0.0;
const MIDDLE_ANCHOR_FACTOR: f64 = -0.5;
const END_ANCHOR_FACTOR: f64 = -1.0;

#[derive(Clone, Copy)]
pub(super) struct LayoutAnchor {
    pub(super) width: f32,
    pub(super) anchor: usvg::TextAnchor,
    pub(super) css_scale: f32,
}

pub(super) fn layout_anchor(text: &usvg::Text) -> Option<LayoutAnchor> {
    let [chunk] = text.chunks() else { return None };
    let [source_span] = chunk.spans() else {
        return None;
    };
    let glyph_count = text
        .layouted()
        .iter()
        .flat_map(|span| &span.positioned_glyphs)
        .count();
    if chunk.text().chars().count() != 1 || glyph_count != 1 {
        return None;
    }
    let decoration = source_span.decoration();
    if decoration.underline().is_some()
        || decoration.overline().is_some()
        || decoration.line_through().is_some()
    {
        return None;
    }
    let width = text.bounding_box().width();
    let css_scale = uniform_css_scale(text.abs_transform())?;
    (width.is_finite() && width > 0.0).then_some(LayoutAnchor {
        width,
        anchor: chunk.anchor(),
        css_scale,
    })
}

pub(super) fn run_origin_css_scale(text: &usvg::Text, prepared: &[PreparedSpan]) -> Option<f32> {
    let [span] = prepared else { return None };
    let [chunk] = text.chunks() else { return None };
    let [source_span] = chunk.spans() else {
        return None;
    };
    if !super::alignment_text_supported(text, chunk, source_span, span)
        || !super::horizontal_glyphs(&span.glyphs)
        || layout_anchor(text).is_some()
    {
        return None;
    }
    let decoration = source_span.decoration();
    if decoration.underline().is_some()
        || decoration.overline().is_some()
        || decoration.line_through().is_some()
    {
        return None;
    }
    if !one_ordered_scalar_per_glyph(chunk.text(), &span.glyphs) {
        return None;
    }
    uniform_css_scale(text.abs_transform())
}

fn one_ordered_scalar_per_glyph(text: &str, glyphs: &[usvg::layout::PositionedGlyph]) -> bool {
    if text.chars().count() != glyphs.len() || glyphs.len() < 2 {
        return false;
    }
    let mut characters = text.chars();
    let mut previous_x = None;
    for glyph in glyphs {
        let Some(character) = characters.next() else {
            return false;
        };
        if glyph.text.chars().count() != 1 || !glyph.text.starts_with(character) {
            return false;
        }
        let transform = glyph.transform();
        if !transform.tx.is_finite()
            || !transform.ty.is_finite()
            || previous_x.is_some_and(|previous| transform.tx <= previous)
        {
            return false;
        }
        previous_x = Some(transform.tx);
    }
    characters.next().is_none()
}

pub(super) fn uniform_css_scale(transform: Transform) -> Option<f32> {
    (transform.sx.is_finite()
        && transform.sx > 0.0
        && transform.sx == transform.sy
        && transform.kx == 0.0
        && transform.ky == 0.0)
        .then_some(transform.sx)
}

pub(super) fn trunc_layout_unit(value: f32) -> Option<f32> {
    let scaled = value * LAYOUT_UNIT_SCALE as f32;
    if !scaled.is_finite() || scaled <= i32::MIN as f32 || scaled >= i32::MAX as f32 {
        return None;
    }
    Some(scaled.trunc() / LAYOUT_UNIT_SCALE as f32)
}

pub(super) fn paint_size(font_size: f32, css_scale: f32, raster_scale: f32) -> Option<f32> {
    let device_scale = raster_scale / css_scale;
    if !font_size.is_finite()
        || font_size <= 0.0
        || !device_scale.is_finite()
        || device_scale <= 0.0
    {
        return None;
    }
    let css_size = css_effective_size(font_size, css_scale)?;
    let size = css_size * device_scale;
    (size.is_finite() && size > 0.0).then_some(size)
}

fn css_effective_size(font_size: f32, css_scale: f32) -> Option<f32> {
    let nominal = font_size * css_scale;
    if !nominal.is_finite() || nominal <= 0.0 {
        return None;
    }
    Some(super::super::SvgRasterizeOps::effective_html_font_size(
        nominal,
    ))
}

pub(super) fn anchor_delta(layout: LayoutAnchor, font_size: f32, raster_scale: f32) -> Option<f32> {
    let device_scale = raster_scale / layout.css_scale;
    let nominal_size = font_size * raster_scale;
    let effective_size = paint_size(font_size, layout.css_scale, raster_scale)?;
    let ratio = effective_size / nominal_size;
    if !device_scale.is_finite()
        || device_scale <= 0.0
        || !nominal_size.is_finite()
        || nominal_size <= 0.0
        || !ratio.is_finite()
    {
        return None;
    }
    let width = f64::from(layout.width) * f64::from(layout.css_scale) * f64::from(ratio);
    let snapped = (width * LAYOUT_UNIT_SCALE).ceil() / LAYOUT_UNIT_SCALE;
    let delta = snapped - width;
    let direction = match layout.anchor {
        usvg::TextAnchor::Start => START_ANCHOR_FACTOR,
        usvg::TextAnchor::Middle => MIDDLE_ANCHOR_FACTOR,
        usvg::TextAnchor::End => END_ANCHOR_FACTOR,
    };
    Some((delta * direction * f64::from(device_scale)) as f32)
}

pub(super) fn anchor_delta_or_zero(
    layout: Option<LayoutAnchor>,
    font_size: f32,
    raster_scale: f32,
) -> f32 {
    layout
        .and_then(|layout| anchor_delta(layout, font_size, raster_scale))
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::{LayoutAnchor, anchor_delta, anchor_delta_or_zero, paint_size};
    use resvg::usvg::TextAnchor;

    #[test]
    fn anchor_rounding_matches_layout_unit_middle_start_and_end() {
        let layout = LayoutAnchor {
            width: 31.130_86,
            anchor: TextAnchor::Middle,
            css_scale: 1.0,
        };
        assert_eq!(anchor_delta(layout, 14.0, 1.0), Some(-0.0048828125));
        assert_eq!(anchor_delta_or_zero(Some(layout), 14.0, 1.0), -0.0048828125);
        for (anchor, expected) in [
            (TextAnchor::Start, 0.0),
            (TextAnchor::Middle, -0.0048828125),
            (TextAnchor::End, -0.009765625),
        ] {
            assert_eq!(
                anchor_delta(LayoutAnchor { anchor, ..layout }, 14.0, 1.0),
                Some(expected)
            );
        }
    }

    #[test]
    fn ratio_adjusted_anchor_width_scales_with_device_ratio() -> Result<(), &'static str> {
        let css_scale = 0.999_974_55_f32;
        let layout = LayoutAnchor {
            width: 31.130_86,
            anchor: TextAnchor::Middle,
            css_scale,
        };
        let at_dpr_one =
            anchor_delta(layout, 14.0, css_scale).ok_or("DPR 1 anchor delta missing")?;
        let at_dpr_two =
            anchor_delta(layout, 14.0, css_scale * 2.0).ok_or("DPR 2 anchor delta missing")?;
        assert!(at_dpr_one < 0.0 && at_dpr_one > -0.001);
        assert!((at_dpr_two - at_dpr_one * 2.0).abs() < 0.000001);
        Ok(())
    }

    #[test]
    fn css_size_is_snapped_before_device_scale() {
        let css_scale = 0.999_974_55_f32;
        assert_eq!(paint_size(14.0, css_scale, css_scale), Some(13.99));
        assert_eq!(paint_size(14.0, css_scale, css_scale * 2.0), Some(27.98));
    }

    #[test]
    fn invalid_scale_keeps_trial_ineligible() {
        let layout = LayoutAnchor {
            width: 31.130_86,
            anchor: TextAnchor::Middle,
            css_scale: f32::NAN,
        };
        assert_eq!(anchor_delta(layout, 14.0, 1.0), None);
        assert_eq!(anchor_delta_or_zero(None, 14.0, 2.0), 0.0);
        assert_eq!(anchor_delta_or_zero(Some(layout), 14.0, 1.0), 0.0);
        assert_eq!(paint_size(14.0, f32::NAN, 2.0), None);
    }
}
