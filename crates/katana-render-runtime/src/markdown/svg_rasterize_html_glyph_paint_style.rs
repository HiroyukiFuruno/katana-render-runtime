use super::super::{ALPHA_CHANNEL, BLUE_CHANNEL, GREEN_CHANNEL, PreparedSpan, RED_CHANNEL};
use skia_safe::{Font, FontHinting, Paint, Typeface};

pub(super) fn text_paint(span: &PreparedSpan) -> Paint {
    let mut paint = Paint::default();
    paint.set_argb(
        span.fill[ALPHA_CHANNEL],
        span.fill[RED_CHANNEL],
        span.fill[GREEN_CHANNEL],
        span.fill[BLUE_CHANNEL],
    );
    paint
}

pub(super) fn text_font(span: &PreparedSpan, typeface: &Typeface, raster_scale: f32) -> Font {
    let size = span
        .fallback_alignment
        .as_ref()
        .map(|alignment| alignment.paint_css_size * raster_scale / alignment.css_scale)
        .or_else(|| {
            span.chunk_alignment
                .map(|alignment| alignment.paint_css_size * raster_scale / alignment.css_scale)
        })
        .or_else(|| single_span_font_size(span, raster_scale));
    let size = match size {
        Some(size) => size,
        None => super::super::super::SvgRasterizeOps::effective_html_font_size(
            span.font_size * raster_scale,
        ),
    };
    font_for_face(typeface, size)
}

fn single_span_font_size(span: &PreparedSpan, raster_scale: f32) -> Option<f32> {
    span.x_alignment
        .and_then(|alignment| alignment.layout)
        .and_then(|layout| {
            super::super::anchor::paint_size(span.font_size, layout.css_scale, raster_scale)
        })
}

pub(in super::super) fn font_for_face(typeface: &Typeface, size: f32) -> Font {
    let mut font = Font::new(typeface.clone(), size);
    font.set_subpixel(true)
        .set_linear_metrics(true)
        .set_baseline_snap(true)
        .set_hinting(FontHinting::Normal)
        .set_edging(skia_safe::font::Edging::SubpixelAntiAlias);
    font
}
