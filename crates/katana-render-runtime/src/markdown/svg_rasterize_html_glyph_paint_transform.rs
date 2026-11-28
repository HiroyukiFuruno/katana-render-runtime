use super::super::PreparedSpan;
use tiny_skia::Transform;

const IDENTITY_SCALE: f32 = 1.0;
const IDENTITY_SKEW: f32 = 0.0;

#[cfg(test)]
pub(in super::super) fn normalized_transform(
    span: &PreparedSpan,
    glyph: &resvg::usvg::layout::PositionedGlyph,
    raster_scale: f32,
) -> Transform {
    let index = span
        .glyphs
        .iter()
        .position(|candidate| std::ptr::eq(candidate, glyph))
        .unwrap_or(0);
    normalized_transform_at(span, glyph, index, raster_scale)
}

pub(super) fn normalized_transform_at(
    span: &PreparedSpan,
    glyph: &resvg::usvg::layout::PositionedGlyph,
    index: usize,
    raster_scale: f32,
) -> Transform {
    let transform = normalize_font_linear(glyph.transform(), span.font_size, span.upem)
        .post_translate(0.0, span.baseline_shift);
    let mut transform = scale_translation(transform, raster_scale);
    if let Some(alignment) = &span.fallback_alignment {
        let position = alignment.css_positions[index];
        let device_scale = raster_scale / alignment.css_scale;
        transform.tx = position.x * device_scale;
        transform.ty = position.y * device_scale;
        return transform;
    }
    if let Some(alignment) = span.chunk_alignment {
        return super::super::chunk::position(alignment, transform, raster_scale);
    }
    if let Some(alignment) = span.x_alignment {
        transform.tx = aligned_position_x(span, alignment, transform.tx, raster_scale);
    }
    transform
}

pub(super) fn normalize_font_linear(transform: Transform, font_size: f32, upem: f32) -> Transform {
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

pub(super) fn aligned_position_x(
    span: &PreparedSpan,
    alignment: super::super::XAlignment,
    old_x: f32,
    raster_scale: f32,
) -> f32 {
    let Some(layout) = alignment.layout else {
        return aligned_x(alignment.anchor_x, old_x, span.font_size, raster_scale);
    };
    let nominal_size = span.font_size * raster_scale;
    let Some(effective_size) =
        super::super::anchor::paint_size(span.font_size, layout.css_scale, raster_scale)
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
    aligned + super::super::anchor::anchor_delta_or_zero(Some(layout), span.font_size, raster_scale)
}

pub(super) fn aligned_x(anchor_x: f32, old_x: f32, font_size: f32, raster_scale: f32) -> f32 {
    let nominal_size = font_size * raster_scale;
    let effective_size =
        super::super::super::SvgRasterizeOps::effective_html_font_size(nominal_size);
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

pub(super) fn scale_translation(transform: Transform, raster_scale: f32) -> Transform {
    Transform::from_row(
        transform.sx,
        transform.ky,
        transform.kx,
        transform.sy,
        transform.tx * raster_scale,
        transform.ty * raster_scale,
    )
}

pub(in super::super) fn is_identity_linear(transform: Transform) -> bool {
    transform.sx == IDENTITY_SCALE
        && transform.sy == IDENTITY_SCALE
        && transform.kx == IDENTITY_SKEW
        && transform.ky == IDENTITY_SKEW
}
