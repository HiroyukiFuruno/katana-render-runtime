use super::Policy;
use crate::markdown::svg_rasterize::{
    html_glyph::{FaceMetadata, HtmlGlyphRasterizer, anchor, paint},
    text_shaping::svg_dom_layout_ascent,
};
use resvg::usvg::{self, TextFlow, fontdb::ID};
use skia_safe::Font;

const HALF_METRIC: f32 = 0.5;
const USVG_X_HEIGHT_RATIO: f32 = 0.45;

pub(super) fn layout_supported(text: &usvg::Text) -> bool {
    let [chunk] = text.chunks() else { return false };
    let dy = text.dy();
    let Some(first_dy) = dy.first() else {
        return false;
    };
    let Some(first_span) = chunk.spans().first() else {
        return false;
    };
    text.writing_mode() == usvg::WritingMode::LeftToRight
        && matches!(chunk.text_flow(), TextFlow::Linear)
        && super::super::alignment_dy_supported(dy)
        && first_dy.is_finite()
        && *first_dy != 0.0
        && super::super::all_zero(text.dx())
        && super::super::all_zero(text.rotate())
        && chunk.spans().iter().all(source_span_has_default_spacing)
        && layout_spans_supported(text.layouted(), first_span.font_size().get())
}

fn layout_spans_supported(spans: &[usvg::layout::Span], font_size: f32) -> bool {
    spans.iter().any(|span| !span.positioned_glyphs.is_empty())
        && spans.iter().all(|span| {
            let glyphs = &span.positioned_glyphs;
            super::super::horizontal_glyphs(glyphs)
                && glyphs.iter().all(|glyph| glyph.font_size() == font_size)
        })
}

fn source_span_has_default_spacing(span: &usvg::TextSpan) -> bool {
    span.letter_spacing() == 0.0 && span.word_spacing() == 0.0 && span.text_length().is_none()
}

pub(super) fn shift(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    id: ID,
    metadata: &FaceMetadata,
    policy: Policy,
) -> Result<Option<f32>, String> {
    let Some(scale) = policy.css_scale else {
        return Ok(None);
    };
    if !scale.is_finite() || scale <= 0.0 || !valid_face_metrics(metadata) {
        return Ok(None);
    }
    let Some(css_size) = anchor::paint_size(policy.font_size, scale, scale) else {
        return Ok(None);
    };
    let Some(face) = rasterizer.ensure_face(id)? else {
        return Ok(None);
    };
    let Ok(typeface) = face.typeface.as_skia() else {
        return Ok(None);
    };
    let font = paint::font_for_face(typeface, css_size);
    Ok(shift_for_font(
        rasterizer, id, metadata, policy, scale, &font,
    ))
}

fn shift_for_font(
    rasterizer: &HtmlGlyphRasterizer<'_>,
    id: ID,
    metadata: &FaceMetadata,
    policy: Policy,
    scale: f32,
    font: &Font,
) -> Option<f32> {
    let raw_units = raw_x_height(metadata)?;
    let x_height = glyph_x_height(font)?;
    let metric_scale = font.size() / metadata.upem;
    let ascent = metadata.ascent * metric_scale;
    let descent = -metadata.descent * metric_scale;
    let layout_ascent = svg_dom_layout_ascent(rasterizer.database.face(id), ascent, descent)?;
    /* WHY: alignment middleはx-height半分、dominant middleはCSS line-top丸めを再現する。 */
    let target_css = match policy.kind {
        super::PolicyKind::AlignmentMiddle => x_height * HALF_METRIC,
        super::PolicyKind::Middle => {
            let baseline_from_top = layout_ascent - x_height * HALF_METRIC;
            layout_ascent + (HALF_METRIC - baseline_from_top).floor()
        }
        _ => return None,
    };
    let raw_shift = raw_units * policy.font_size / metadata.upem * HALF_METRIC;
    let correction = target_css / scale - raw_shift;
    correction.is_finite().then_some(correction)
}

fn glyph_x_height(font: &Font) -> Option<f32> {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    let height = {
        let glyph = font.unichar_to_glyph('x' as i32);
        if glyph == 0 {
            return None;
        }
        /* WHY: Apple版Blinkと同じ輪郭boundsを使い、AA用のbounds拡張を避ける。 */
        -font.get_path(glyph)?.bounds().top
    };
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
    let height = font.metrics().1.x_height;
    (height.is_finite() && height > 0.0).then_some(height)
}

fn raw_x_height(metadata: &FaceMetadata) -> Option<f32> {
    /* WHY: usvgはx-heightがないfontでascent/descent比を使う。 */
    metadata
        .x_height
        .and_then(quantized_x_height)
        .or_else(|| quantized_x_height((metadata.ascent - metadata.descent) * USVG_X_HEIGHT_RATIO))
}

fn valid_face_metrics(metadata: &FaceMetadata) -> bool {
    metadata.upem.is_finite()
        && metadata.upem > 0.0
        && metadata.ascent.is_finite()
        && metadata.descent.is_finite()
}

fn quantized_x_height(value: f32) -> Option<f32> {
    let rounded = value.round();
    (value.is_finite() && value > 0.0 && rounded >= 1.0 && rounded <= f32::from(u16::MAX))
        .then_some(rounded)
}

#[cfg(test)]
#[path = "svg_rasterize_html_glyph_baseline_middle_tests.rs"]
mod tests;
