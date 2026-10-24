use super::{ALPHA_CHANNEL, BLUE_CHANNEL, GREEN_CHANNEL, PreparedSpan, RED_CHANNEL};
use skia_safe::{Canvas, Font, FontHinting, Paint, Point as SkiaPoint, Typeface};
use tiny_skia::Transform;

const IDENTITY_SCALE: f32 = 1.0;
const IDENTITY_SKEW: f32 = 0.0;

pub(super) fn draw_span(
    canvas: &Canvas,
    span: &PreparedSpan,
    typeface: &Typeface,
    raster_scale: f32,
) -> Result<(), String> {
    let paint = text_paint(span);
    let font = text_font(span, typeface, raster_scale);
    if span
        .glyphs
        .iter()
        .all(|glyph| is_identity_linear(normalized_transform(span, glyph, raster_scale)))
    {
        return draw_identity_glyphs(canvas, span, &font, &paint, raster_scale);
    }
    draw_transformed_glyphs(canvas, span, &font, &paint, raster_scale)
}

fn draw_identity_glyphs(
    canvas: &Canvas,
    span: &PreparedSpan,
    font: &Font,
    paint: &Paint,
    raster_scale: f32,
) -> Result<(), String> {
    let positions = span
        .glyphs
        .iter()
        .map(|glyph| {
            let transform = normalized_transform(span, glyph, raster_scale);
            SkiaPoint::new(transform.tx, transform.ty)
        })
        .collect::<Vec<_>>();
    canvas.draw_glyphs_at(
        &span.glyph_ids,
        positions.as_slice(),
        SkiaPoint::new(0.0, 0.0),
        font,
        paint,
    );
    Ok(())
}

fn draw_transformed_glyphs(
    canvas: &Canvas,
    span: &PreparedSpan,
    font: &Font,
    paint: &Paint,
    raster_scale: f32,
) -> Result<(), String> {
    for (glyph, glyph_id) in span.glyphs.iter().zip(&span.glyph_ids) {
        let save = canvas.save();
        let matrix =
            super::surface::skia_matrix(normalized_transform(span, glyph, raster_scale), 0.0, 0.0);
        canvas.concat(&matrix);
        canvas.draw_glyphs_at(
            std::slice::from_ref(glyph_id),
            &[SkiaPoint::new(0.0, 0.0)][..],
            SkiaPoint::new(0.0, 0.0),
            font,
            paint,
        );
        canvas.restore_to_count(save);
    }
    Ok(())
}

pub(super) fn normalized_transform(
    span: &PreparedSpan,
    glyph: &resvg::usvg::layout::PositionedGlyph,
    raster_scale: f32,
) -> Transform {
    let transform = glyph
        .transform()
        .pre_scale(span.upem / span.font_size, span.upem / span.font_size)
        .post_translate(0.0, span.baseline_shift);
    let mut transform = scale_translation(transform, raster_scale);
    if let Some(alignment) = span.x_alignment {
        transform.tx = aligned_x(
            alignment.anchor_x,
            transform.tx,
            span.font_size,
            raster_scale,
        );
    }
    transform
}

fn aligned_x(anchor_x: f32, old_x: f32, font_size: f32, raster_scale: f32) -> f32 {
    let nominal_size = font_size * raster_scale;
    let effective_size = super::super::SvgRasterizeOps::effective_html_font_size(nominal_size);
    if !nominal_size.is_finite() || nominal_size <= 0.0 || !effective_size.is_finite() {
        return old_x;
    }
    let ratio = effective_size / nominal_size;
    if ratio == 1.0 {
        return old_x;
    }
    let anchor = anchor_x * raster_scale;
    anchor + (old_x - anchor) * ratio
}

fn scale_translation(transform: Transform, raster_scale: f32) -> Transform {
    Transform::from_row(
        transform.sx,
        transform.ky,
        transform.kx,
        transform.sy,
        transform.tx * raster_scale,
        transform.ty * raster_scale,
    )
}

pub(super) fn is_identity_linear(transform: Transform) -> bool {
    transform.sx == IDENTITY_SCALE
        && transform.sy == IDENTITY_SCALE
        && transform.kx == IDENTITY_SKEW
        && transform.ky == IDENTITY_SKEW
}

fn text_paint(span: &PreparedSpan) -> Paint {
    let mut paint = Paint::default();
    paint.set_argb(
        span.fill[ALPHA_CHANNEL],
        span.fill[RED_CHANNEL],
        span.fill[GREEN_CHANNEL],
        span.fill[BLUE_CHANNEL],
    );
    paint
}

fn text_font(span: &PreparedSpan, typeface: &Typeface, raster_scale: f32) -> Font {
    let size =
        super::super::SvgRasterizeOps::effective_html_font_size(span.font_size * raster_scale);
    let mut font = Font::new(typeface.clone(), size);
    font.set_subpixel(true)
        .set_linear_metrics(true)
        .set_baseline_snap(true)
        .set_hinting(FontHinting::Normal)
        .set_edging(skia_safe::font::Edging::SubpixelAntiAlias);
    font
}

#[cfg(test)]
mod tests {
    use super::super::PreparedSpan;
    use super::super::uniform_raster_scale;
    use super::{aligned_x, is_identity_linear, scale_translation, text_font};
    use skia_safe::{FontMgr, FontStyle};
    use tiny_skia::Transform;

    const SOURCE_FONT_SIZE: f32 = 14.0;
    const FRACTIONAL_RASTER_SCALE: f32 = 0.999_974_55;
    const EXPECTED_EFFECTIVE_SIZE: f32 = 13.99;

    #[test]
    fn glyph_batching_rejects_non_identity_linear_transforms() {
        assert!(is_identity_linear(Transform::from_row(
            1.0, 0.0, 0.0, 1.0, 4.0, 8.0
        )));
        assert!(!is_identity_linear(Transform::from_row(
            1.0, 0.1, 0.0, 1.0, 4.0, 8.0
        )));
        assert!(!is_identity_linear(Transform::from_row(
            1.1, 0.0, 0.0, 1.0, 4.0, 8.0
        )));
        assert!(!is_identity_linear(Transform::from_row(
            1.0, 0.0, 0.0, -1.0, 4.0, 8.0
        )));
    }

    #[test]
    fn uniform_scale_preserves_linear_parts_and_scales_baseline_translation() {
        let transform = Transform::from_row(1.0, 0.2, -0.1, 1.0, 3.0, 10.0);
        let scaled = scale_translation(transform, 2.0);

        assert_eq!(scaled.sx, transform.sx);
        assert_eq!(scaled.kx, transform.kx);
        assert_eq!(scaled.ky, transform.ky);
        assert_eq!(scaled.sy, transform.sy);
        assert_eq!(scaled.tx, 6.0);
        assert_eq!(scaled.ty, 20.0);
    }

    #[test]
    fn uniform_scale_does_not_snap_near_identity_or_general_transforms() {
        assert_eq!(
            uniform_raster_scale(Transform::from_row(1.0001, 0.0, 0.0, 1.0001, 0.0, 0.0)),
            1.0001
        );
        assert_eq!(
            uniform_raster_scale(Transform::from_row(2.0, 0.0, 0.0, 3.0, 0.0, 0.0)),
            1.0
        );
        assert_eq!(
            uniform_raster_scale(Transform::from_row(2.0, 0.1, 0.0, 2.0, 0.0, 0.0)),
            1.0
        );
    }

    #[test]
    fn paint_font_uses_effective_scaled_size_before_skia_font_creation() -> Result<(), String> {
        let typeface = FontMgr::default()
            .legacy_make_typeface(Some("Trebuchet MS"), FontStyle::normal())
            .ok_or("Trebuchet MS typeface unavailable")?;
        let span = PreparedSpan {
            fill: [0, 0, 0, 255],
            font: resvg::usvg::fontdb::ID::default(),
            glyphs: Vec::new(),
            glyph_ids: Vec::new(),
            upem: 2048.0,
            font_size: SOURCE_FONT_SIZE,
            baseline_shift: 0.0,
            x_alignment: None,
        };

        let fractional = text_font(&span, &typeface, FRACTIONAL_RASTER_SCALE);
        let identity = text_font(&span, &typeface, 1.0);

        assert_eq!(fractional.size(), EXPECTED_EFFECTIVE_SIZE);
        assert_eq!(identity.size(), SOURCE_FONT_SIZE);
        Ok(())
    }

    #[test]
    fn aligned_x_preserves_identity_and_anchor() {
        assert_eq!(aligned_x(12.0, 18.0, 14.0, 1.0), 18.0);
        assert_eq!(aligned_x(12.0, 40.0, 14.0, 2.0), 40.0);
        let scaled_x = 20.0 * FRACTIONAL_RASTER_SCALE;
        let expected = 12.0 * FRACTIONAL_RASTER_SCALE
            + (scaled_x - 12.0 * FRACTIONAL_RASTER_SCALE)
                * (EXPECTED_EFFECTIVE_SIZE / (14.0 * FRACTIONAL_RASTER_SCALE));
        assert!(
            (aligned_x(12.0, scaled_x, 14.0, FRACTIONAL_RASTER_SCALE) - expected).abs() < 0.0001
        );
    }

    #[test]
    fn aligned_x_keeps_invalid_inputs_unchanged() {
        assert_eq!(aligned_x(12.0, 18.0, 0.0, 2.0), 18.0);
        assert_eq!(aligned_x(12.0, 18.0, f32::NAN, 2.0), 18.0);
    }
}
