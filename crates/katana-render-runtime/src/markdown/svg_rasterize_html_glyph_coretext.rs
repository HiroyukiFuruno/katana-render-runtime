use super::{HtmlGlyphRasterizer, PreparedSpan, surface};
use core_graphics::{
    base::kCGImageAlphaPremultipliedLast,
    color_space::CGColorSpace,
    context::CGContext,
    font::CGFont,
    geometry::{CGAffineTransform, CGPoint},
};
use core_text::font::new_from_CGFont;
use tiny_skia::Transform;

const CHANNELS: usize = 4;
const BITS_PER_COMPONENT: usize = 8;

pub(super) fn draw_prepared(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    prepared: Vec<PreparedSpan>,
    full_transform: Transform,
    crop: &surface::Crop,
) -> Result<Vec<u8>, String> {
    let mut context = bitmap_context(crop.width, crop.height)?;
    let raster_scale = super::uniform_raster_scale(full_transform);
    let canvas_transform = if raster_scale == 1.0 {
        full_transform
    } else {
        full_transform.pre_scale(1.0 / raster_scale, 1.0 / raster_scale)
    };
    configure_context(&context);
    for span in prepared {
        draw_span(
            rasterizer,
            &mut context,
            &span,
            canvas_transform,
            raster_scale,
            crop,
        )?;
    }
    Ok(context.data().to_vec())
}

fn bitmap_context(width: u32, height: u32) -> Result<CGContext, String> {
    let color_space = CGColorSpace::create_device_rgb();
    let width = usize::try_from(width).map_err(|_| "CoreText crop width overflow")?;
    let height = usize::try_from(height).map_err(|_| "CoreText crop height overflow")?;
    Ok(CGContext::create_bitmap_context(
        None,
        width,
        height,
        BITS_PER_COMPONENT,
        width * CHANNELS,
        &color_space,
        kCGImageAlphaPremultipliedLast,
    ))
}

fn configure_context(context: &CGContext) {
    context.set_allows_font_smoothing(true);
    context.set_should_smooth_fonts(true);
    context.set_allows_antialiasing(true);
    context.set_should_antialias(true);
    context.set_allows_font_subpixel_positioning(true);
    context.set_should_subpixel_position_fonts(true);
}

fn draw_span(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    context: &mut CGContext,
    span: &PreparedSpan,
    canvas_transform: Transform,
    raster_scale: f32,
    crop: &surface::Crop,
) -> Result<(), String> {
    let Some(face) = rasterizer.ensure_face(span.font)? else {
        return Err(format!(
            "cached SVG font face {:?} became unavailable",
            span.font
        ));
    };
    let super::types::CachedTypeface::CoreText(cgfont) = &face.typeface else {
        return Err("Skia face reached CoreText renderer".to_string());
    };
    let font = span_font(cgfont, span.font_size, raster_scale);
    set_fill_color(context, span);
    let transforms = normalized_transforms(span, raster_scale);
    draw_span_glyphs(context, &font, span, transforms, canvas_transform, crop);
    Ok(())
}

fn normalized_transforms(span: &PreparedSpan, raster_scale: f32) -> Vec<Transform> {
    span.glyphs
        .iter()
        .map(|glyph| super::paint::normalized_transform(span, glyph, raster_scale))
        .collect()
}

fn draw_span_glyphs(
    context: &CGContext,
    font: &core_text::font::CTFont,
    span: &PreparedSpan,
    transforms: Vec<Transform>,
    canvas_transform: Transform,
    crop: &surface::Crop,
) {
    if transforms
        .iter()
        .all(|transform| super::paint::is_identity_linear(*transform))
    {
        draw_identity_glyphs(context, font, span, &transforms, canvas_transform, crop);
    } else {
        draw_transformed_glyphs(context, font, span, transforms, canvas_transform, crop);
    }
}

fn span_font(cgfont: &CGFont, font_size: f32, raster_scale: f32) -> core_text::font::CTFont {
    let size = super::super::SvgRasterizeOps::effective_html_font_size(font_size * raster_scale);
    new_from_CGFont(cgfont, f64::from(size))
}

fn set_fill_color(context: &CGContext, span: &PreparedSpan) {
    context.set_rgb_fill_color(
        f64::from(span.fill[super::RED_CHANNEL]) / f64::from(u8::MAX),
        f64::from(span.fill[super::GREEN_CHANNEL]) / f64::from(u8::MAX),
        f64::from(span.fill[super::BLUE_CHANNEL]) / f64::from(u8::MAX),
        f64::from(span.fill[super::ALPHA_CHANNEL]) / f64::from(u8::MAX),
    );
}

fn draw_identity_glyphs(
    context: &CGContext,
    font: &core_text::font::CTFont,
    span: &PreparedSpan,
    transforms: &[Transform],
    canvas_transform: Transform,
    crop: &surface::Crop,
) {
    context.save();
    context.concat_ctm(canvas_affine(canvas_transform, crop));
    let positions = transforms
        .iter()
        .map(|transform| CGPoint::new(f64::from(transform.tx), f64::from(-transform.ty)))
        .collect::<Vec<_>>();
    font.draw_glyphs(&span.glyph_ids, &positions, context.clone());
    context.restore();
}

fn draw_transformed_glyphs(
    context: &CGContext,
    font: &core_text::font::CTFont,
    span: &PreparedSpan,
    transforms: Vec<Transform>,
    canvas_transform: Transform,
    crop: &surface::Crop,
) {
    for (glyph_id, transform) in span.glyph_ids.iter().zip(transforms) {
        draw_glyph(
            context,
            font,
            *glyph_id,
            canvas_transform.pre_concat(transform),
            crop,
        );
    }
}

fn draw_glyph(
    context: &CGContext,
    font: &core_text::font::CTFont,
    glyph_id: u16,
    transform: Transform,
    crop: &surface::Crop,
) {
    context.save();
    context.concat_ctm(canvas_affine(transform, crop));
    font.draw_glyphs(&[glyph_id], &[CGPoint::new(0.0, 0.0)], context.clone());
    context.restore();
}

fn canvas_affine(transform: Transform, crop: &surface::Crop) -> CGAffineTransform {
    CGAffineTransform::new(
        f64::from(transform.sx),
        f64::from(-transform.ky),
        f64::from(-transform.kx),
        f64::from(transform.sy),
        f64::from(transform.tx - crop.x as f32),
        f64::from(crop.height as f32 + crop.y as f32 - transform.ty),
    )
}
