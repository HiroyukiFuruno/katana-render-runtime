use super::{ALPHA_CHANNEL, BLUE_CHANNEL, GREEN_CHANNEL, PreparedSpan, RED_CHANNEL};
use skia_safe::{Canvas, Font, FontHinting, Paint, Point as SkiaPoint, Typeface};
use tiny_skia::Transform;

#[cfg(test)]
#[path = "svg_rasterize_html_glyph_paint_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "svg_rasterize_html_glyph_paint_capture.rs"]
mod capture;

#[cfg(test)]
#[path = "svg_rasterize_html_glyph_paint_capture_tests.rs"]
mod capture_tests;

#[cfg(test)]
pub(in crate::markdown::svg_rasterize) use capture::Scope as PaintCaptureScope;

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_html_glyph_paint_hanging_origin_tests.rs"]
mod hanging_origin_tests;

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
    let mut positions = span
        .glyphs
        .iter()
        .map(|glyph| {
            let transform = normalized_transform(span, glyph, raster_scale);
            SkiaPoint::new(transform.tx, transform.ty)
        })
        .collect::<Vec<_>>();
    apply_run_origin_snap(span, &mut positions, raster_scale);
    draw_glyphs_at(
        canvas,
        &span.glyph_ids,
        positions.as_slice(),
        SkiaPoint::new(0.0, 0.0),
        font,
        paint,
    );
    Ok(())
}

fn apply_run_origin_snap(span: &PreparedSpan, positions: &mut [SkiaPoint], raster_scale: f32) {
    let Some(origin) = positions.first() else {
        return;
    };
    let Some(device_scale) = run_origin_device_scale(span, raster_scale) else {
        return;
    };
    let x_delta = run_origin_axis_delta(origin.x, device_scale).unwrap_or(0.0);
    let y_delta = run_origin_axis_delta(origin.y, device_scale).unwrap_or(0.0);
    for position in positions {
        position.x += x_delta;
        position.y += y_delta;
    }
}

fn run_origin_device_scale(span: &PreparedSpan, raster_scale: f32) -> Option<f32> {
    let alignment = span.x_alignment?;
    if alignment.layout.is_some() {
        return None;
    }
    let css_scale = alignment.origin_snap_css_scale?;
    let device_scale = raster_scale / css_scale;
    if !device_scale.is_finite() || device_scale <= 0.0 {
        return None;
    }
    Some(device_scale)
}

fn run_origin_axis_delta(origin: f32, device_scale: f32) -> Option<f32> {
    if !origin.is_finite() {
        return None;
    }
    let css_origin = origin / device_scale;
    let snapped_origin = super::anchor::trunc_layout_unit(css_origin)?;
    Some((snapped_origin - css_origin) * device_scale)
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
        draw_glyphs_at(
            canvas,
            std::slice::from_ref(glyph_id),
            &[SkiaPoint::new(0.0, 0.0)],
            SkiaPoint::new(0.0, 0.0),
            font,
            paint,
        );
        canvas.restore_to_count(save);
    }
    Ok(())
}

fn draw_glyphs_at(
    canvas: &Canvas,
    glyph_ids: &[u16],
    positions: &[SkiaPoint],
    origin: SkiaPoint,
    font: &Font,
    paint: &Paint,
) {
    #[cfg(test)]
    capture::record_draw(font, glyph_ids, positions, origin, canvas.local_to_device());
    canvas.draw_glyphs_at(glyph_ids, positions, origin, font, paint);
}

pub(super) fn normalized_transform(
    span: &PreparedSpan,
    glyph: &resvg::usvg::layout::PositionedGlyph,
    raster_scale: f32,
) -> Transform {
    let transform = normalize_font_linear(glyph.transform(), span.font_size, span.upem)
        .post_translate(0.0, span.baseline_shift);
    let mut transform = scale_translation(transform, raster_scale);
    if let Some(alignment) = span.chunk_alignment {
        return super::chunk::position(alignment, transform, raster_scale);
    }
    if let Some(alignment) = span.x_alignment {
        transform.tx = aligned_position_x(span, alignment, transform.tx, raster_scale);
    }
    transform
}

fn normalize_font_linear(transform: Transform, font_size: f32, upem: f32) -> Transform {
    /* WHY: usvgと同じ倍率で除算し、逆数との積の丸めで恒等変換を歪ませない。 */
    let scale = font_size / upem;
    Transform::from_row(
        transform.sx / scale,
        transform.ky / scale,
        transform.kx / scale,
        transform.sy / scale,
        transform.tx,
        transform.ty,
    )
}

fn aligned_position_x(
    span: &PreparedSpan,
    alignment: super::XAlignment,
    old_x: f32,
    raster_scale: f32,
) -> f32 {
    let Some(layout) = alignment.layout else {
        return aligned_x(alignment.anchor_x, old_x, span.font_size, raster_scale);
    };
    let nominal_size = span.font_size * raster_scale;
    let Some(effective_size) =
        super::anchor::paint_size(span.font_size, layout.css_scale, raster_scale)
    else {
        return aligned_x(alignment.anchor_x, old_x, span.font_size, raster_scale);
    };
    let aligned = aligned_x_with_size(
        alignment.anchor_x,
        old_x,
        nominal_size,
        effective_size,
        raster_scale,
    );
    aligned + super::anchor::anchor_delta_or_zero(Some(layout), span.font_size, raster_scale)
}

fn aligned_x(anchor_x: f32, old_x: f32, font_size: f32, raster_scale: f32) -> f32 {
    let nominal_size = font_size * raster_scale;
    let effective_size = super::super::SvgRasterizeOps::effective_html_font_size(nominal_size);
    aligned_x_with_size(anchor_x, old_x, nominal_size, effective_size, raster_scale)
}

fn aligned_x_with_size(
    anchor_x: f32,
    old_x: f32,
    nominal_size: f32,
    effective_size: f32,
    raster_scale: f32,
) -> f32 {
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
    let size = span
        .chunk_alignment
        .map(|alignment| alignment.paint_css_size * raster_scale / alignment.css_scale)
        .or_else(|| single_span_font_size(span, raster_scale));
    let size = match size {
        Some(size) => size,
        None => {
            super::super::SvgRasterizeOps::effective_html_font_size(span.font_size * raster_scale)
        }
    };
    font_for_face(typeface, size)
}

fn single_span_font_size(span: &PreparedSpan, raster_scale: f32) -> Option<f32> {
    span.x_alignment
        .and_then(|alignment| alignment.layout)
        .and_then(|layout| {
            super::anchor::paint_size(span.font_size, layout.css_scale, raster_scale)
        })
}

pub(super) fn font_for_face(typeface: &Typeface, size: f32) -> Font {
    let mut font = Font::new(typeface.clone(), size);
    font.set_subpixel(true)
        .set_linear_metrics(true)
        .set_baseline_snap(true)
        .set_hinting(FontHinting::Normal)
        .set_edging(skia_safe::font::Edging::SubpixelAntiAlias);
    font
}
