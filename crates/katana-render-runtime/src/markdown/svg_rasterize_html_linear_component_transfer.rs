use super::{AuthoredClipInfo, TextRasterizer};
use resvg::usvg::{self, BlendMode, filter};
use tiny_skia::{Pixmap, PixmapMut, Transform};

const MATRIX_ELEMENT_COUNT: usize = 20;
const MATRIX_ZERO: f32 = 0.0;
const IDENTITY_SCALE: f32 = 1.0;

#[derive(Clone, Copy)]
struct TransferSpec {
    matrix: [f32; MATRIX_ELEMENT_COUNT],
    bounds: super::super::html_shadow::Bounds,
}

pub(super) fn render(
    group: &usvg::Group,
    child_transform: Transform,
    target: &mut PixmapMut<'_>,
    provenance: &AuthoredClipInfo,
    text: &dyn TextRasterizer,
) -> Result<bool, String> {
    if !super::super::html_shadow::supported_transform(child_transform) {
        return Ok(false);
    }
    let Some(spec) = transfer_spec(group, child_transform)? else {
        return Ok(false);
    };
    let source = source_bounds(group, child_transform)?;
    let Some(bounds) = intersect(spec.bounds, source) else {
        return Ok(false);
    };
    let Some(crop) = super::super::html_shadow::bounded_crop(bounds, target)? else {
        return Ok(false);
    };
    let source = render_source(group, child_transform, crop, provenance, text)?;
    let filter = skia_safe::color_filters::matrix_row_major(&spec.matrix, None);
    let filtered = super::super::html_shadow::apply_color_filter(&source, filter)?;
    super::super::html_shadow::copy_source_over(target, &filtered, crop.left, crop.top);
    Ok(true)
}

fn transfer_spec(
    group: &usvg::Group,
    transform: Transform,
) -> Result<Option<TransferSpec>, String> {
    let Some(filter) = supported_filter(group) else {
        return Ok(None);
    };
    let Some(primitive) = supported_primitive(filter)? else {
        return Ok(None);
    };
    let Some(matrix) = transfer_matrix(primitive)? else {
        return Ok(None);
    };
    let Some(bounds) = spec_bounds(filter, primitive, transform)? else {
        return Ok(None);
    };
    Ok(Some(TransferSpec { matrix, bounds }))
}

fn supported_filter(group: &usvg::Group) -> Option<&filter::Filter> {
    if group.opacity().get() != IDENTITY_SCALE
        || group.blend_mode() != BlendMode::Normal
        || group.isolate()
        || group.clip_path().is_some()
        || group.mask().is_some()
    {
        return None;
    }
    let [filter] = group.filters() else {
        return None;
    };
    Some(filter)
}

fn supported_primitive(filter: &filter::Filter) -> Result<Option<&filter::Primitive>, String> {
    let [primitive] = filter.primitives() else {
        return Ok(None);
    };
    if primitive.color_interpolation() != filter::ColorInterpolation::SRGB {
        return Ok(None);
    }
    let filter::Kind::ComponentTransfer(transfer) = primitive.kind() else {
        return Ok(None);
    };
    if transfer.input() != &filter::Input::SourceGraphic {
        return Ok(None);
    }
    Ok(Some(primitive))
}

fn transfer_matrix(
    primitive: &filter::Primitive,
) -> Result<Option<[f32; MATRIX_ELEMENT_COUNT]>, String> {
    let filter::Kind::ComponentTransfer(transfer) = primitive.kind() else {
        return Ok(None);
    };
    let Some(red) = linear(transfer.func_r())? else {
        return Ok(None);
    };
    let Some(green) = linear(transfer.func_g())? else {
        return Ok(None);
    };
    let Some(blue) = linear(transfer.func_b())? else {
        return Ok(None);
    };
    if !matches!(transfer.func_a(), filter::TransferFunction::Identity) {
        return Ok(None);
    }
    Ok(Some(component_matrix(red, green, blue)))
}
fn component_matrix(
    red: (f32, f32),
    green: (f32, f32),
    blue: (f32, f32),
) -> [f32; MATRIX_ELEMENT_COUNT] {
    [
        red.0,
        MATRIX_ZERO,
        MATRIX_ZERO,
        MATRIX_ZERO,
        red.1,
        MATRIX_ZERO,
        green.0,
        MATRIX_ZERO,
        MATRIX_ZERO,
        green.1,
        MATRIX_ZERO,
        MATRIX_ZERO,
        blue.0,
        MATRIX_ZERO,
        blue.1,
        MATRIX_ZERO,
        MATRIX_ZERO,
        MATRIX_ZERO,
        IDENTITY_SCALE,
        MATRIX_ZERO,
    ]
}
fn linear(function: &filter::TransferFunction) -> Result<Option<(f32, f32)>, String> {
    let filter::TransferFunction::Linear { slope, intercept } = function else {
        return Ok(None);
    };
    if slope.is_finite() && intercept.is_finite() {
        Ok(Some((*slope, *intercept)))
    } else {
        Err("non-finite component-transfer function".to_owned())
    }
}
fn spec_bounds(
    filter: &filter::Filter,
    primitive: &filter::Primitive,
    transform: Transform,
) -> Result<Option<super::super::html_shadow::Bounds>, String> {
    let filter_bounds = super::super::html_shadow::map_rect(filter.rect().to_rect(), transform)?;
    let primitive_bounds =
        super::super::html_shadow::map_rect(primitive.rect().to_rect(), transform)?;
    Ok(intersect(filter_bounds, primitive_bounds))
}

fn source_bounds(
    group: &usvg::Group,
    transform: Transform,
) -> Result<super::super::html_shadow::Bounds, String> {
    let rect = group.layer_bounding_box().to_rect();
    super::super::html_shadow::map_rect(rect, transform)
}

fn intersect(
    first: super::super::html_shadow::Bounds,
    second: super::super::html_shadow::Bounds,
) -> Option<super::super::html_shadow::Bounds> {
    let bounds = super::super::html_shadow::Bounds {
        left: first.left.max(second.left),
        top: first.top.max(second.top),
        right: first.right.min(second.right),
        bottom: first.bottom.min(second.bottom),
    };
    (bounds.left < bounds.right && bounds.top < bounds.bottom).then_some(bounds)
}

fn render_source(
    group: &usvg::Group,
    transform: Transform,
    crop: super::super::html_shadow::Crop,
    provenance: &AuthoredClipInfo,
    text: &dyn TextRasterizer,
) -> Result<Pixmap, String> {
    let mut source = Pixmap::new(crop.width(), crop.height())
        .ok_or_else(|| "component-transfer source allocation failed".to_owned())?;
    let local =
        Transform::from_translate(-(crop.left as f32), -(crop.top as f32)).pre_concat(transform);
    super::render_children_with_text(group, local, &mut source.as_mut(), provenance, Some(text))?;
    Ok(source)
}
#[cfg(test)]
#[path = "svg_rasterize_html_linear_component_transfer_tests.rs"]
mod tests;
