use super::PreparedSpan;
use skia_safe::{Canvas, Font, Paint, Point as SkiaPoint, Typeface};

#[path = "svg_rasterize_html_glyph_paint_style.rs"]
mod style;
#[path = "svg_rasterize_html_glyph_paint_transform.rs"]
mod transform;
pub(super) use style::font_for_face;
use style::{text_font, text_paint};
pub(super) use transform::is_identity_linear;
#[cfg(test)]
pub(super) use transform::normalized_transform;
use transform::normalized_transform_at;
#[cfg(test)]
use transform::{aligned_position_x, aligned_x, normalize_font_linear, scale_translation};

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

pub(super) fn draw_span(
    canvas: &Canvas,
    span: &PreparedSpan,
    typeface: &Typeface,
    raster_scale: f32,
) -> Result<(), String> {
    #[cfg(test)]
    if let Some(instance) = &span.variable_instance {
        if instance.face.typeface.as_skia().unique_id() != typeface.unique_id() {
            return Err("variable paint typeface differs from the measured instance".into());
        }
        capture::record_instance(typeface, &instance.applied_axes);
    }
    let font = text_font(span, typeface, raster_scale);
    let fill = text_paint(span);
    let Some(effects) = &span.effects else {
        return draw_with_paint(canvas, span, &font, &fill, raster_scale);
    };
    draw_effects(canvas, span, &font, &fill, effects, raster_scale)
}

fn draw_effects(
    canvas: &Canvas,
    span: &PreparedSpan,
    font: &Font,
    fill: &Paint,
    effects: &super::decorations::Effects,
    raster_scale: f32,
) -> Result<(), String> {
    super::decorations::draw_before(canvas, effects, raster_scale);
    let mut stroke = effects.stroke.clone();
    if let Some(paint) = &mut stroke {
        paint.set_stroke_width(paint.stroke_width() * raster_scale);
    }
    let paints = match effects.order {
        resvg::usvg::PaintOrder::FillAndStroke => [Some(fill), stroke.as_ref()],
        resvg::usvg::PaintOrder::StrokeAndFill => [stroke.as_ref(), Some(fill)],
    };
    for paint in paints.into_iter().flatten() {
        draw_with_paint(canvas, span, font, paint, raster_scale)?;
    }
    super::decorations::draw_after(canvas, effects, raster_scale);
    Ok(())
}

fn draw_with_paint(
    canvas: &Canvas,
    span: &PreparedSpan,
    font: &Font,
    paint: &Paint,
    raster_scale: f32,
) -> Result<(), String> {
    if span.glyphs.iter().enumerate().all(|(index, glyph)| {
        is_identity_linear(normalized_transform_at(span, glyph, index, raster_scale))
    }) {
        return draw_identity_glyphs(canvas, span, font, paint, raster_scale);
    }
    draw_transformed_glyphs(canvas, span, font, paint, raster_scale)
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
        .enumerate()
        .map(|(index, glyph)| {
            let transform = normalized_transform_at(span, glyph, index, raster_scale);
            SkiaPoint::new(transform.tx, transform.ty)
        })
        .collect::<Vec<_>>();
    apply_run_origin_snap(span, &mut positions, raster_scale);
    #[cfg(test)]
    capture::record_source(span, raster_scale, None);
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
    for (index, (glyph, glyph_id)) in span.glyphs.iter().zip(&span.glyph_ids).enumerate() {
        let save = canvas.save();
        let matrix = super::surface::skia_matrix(
            normalized_transform_at(span, glyph, index, raster_scale),
            0.0,
            0.0,
        );
        canvas.concat(&matrix);
        #[cfg(test)]
        capture::record_source(span, raster_scale, Some(index));
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
    capture::record_surface_props(canvas.top_props());
    #[cfg(test)]
    capture::record_draw(font, glyph_ids, positions, origin, canvas.local_to_device());
    canvas.draw_glyphs_at(glyph_ids, positions, origin, font, paint);
}
