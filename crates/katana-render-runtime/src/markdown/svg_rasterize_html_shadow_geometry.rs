pub(super) use super::bounds::{Bounds, Crop, bounded_crop, supported_transform};

use super::Transform;
use super::bounds;
use super::surface;
use resvg::usvg::{BlendMode, Group, filter};
use skia_safe::{Color4f, Rect as SkRect};

#[derive(Clone, Copy)]
pub(super) struct ShadowSpec {
    pub(super) dx: f32,
    pub(super) dy: f32,
    pub(super) sigma_x: f32,
    pub(super) sigma_y: f32,
    pub(super) color: Color4f,
    pub(super) filter_bounds: Bounds,
    pub(super) primitive_bounds: Bounds,
}

pub(super) fn shadow_spec(group: &Group) -> Result<Option<ShadowSpec>, String> {
    if !group_is_supported(group) {
        return Ok(None);
    }
    let [filter] = group.filters() else {
        return Ok(None);
    };
    let [primitive] = filter.primitives() else {
        return Ok(None);
    };
    let Some(shadow) = supported_shadow(primitive)? else {
        return Ok(None);
    };
    Ok(Some(build_shadow_spec(shadow, filter, primitive)?))
}

fn build_shadow_spec(
    shadow: &filter::DropShadow,
    filter: &filter::Filter,
    primitive: &filter::Primitive,
) -> Result<ShadowSpec, String> {
    let color = shadow.color();
    let filter_bounds = filter.rect().to_rect();
    let primitive_bounds = primitive.rect().to_rect();
    Ok(ShadowSpec {
        dx: shadow.dx(),
        dy: shadow.dy(),
        sigma_x: shadow.std_dev_x().get(),
        sigma_y: shadow.std_dev_y().get(),
        color: Color4f::new(
            f32::from(color.red) / f32::from(u8::MAX),
            f32::from(color.green) / f32::from(u8::MAX),
            f32::from(color.blue) / f32::from(u8::MAX),
            shadow.opacity().get(),
        ),
        filter_bounds: bounds::rect_bounds(filter_bounds),
        primitive_bounds: bounds::rect_bounds(primitive_bounds),
    })
}

fn group_is_supported(group: &Group) -> bool {
    group.opacity().get() == 1.0
        && group.blend_mode() == BlendMode::Normal
        && !group.isolate()
        && group.clip_path().is_none()
        && group.mask().is_none()
}

fn supported_shadow(primitive: &filter::Primitive) -> Result<Option<&filter::DropShadow>, String> {
    if primitive.color_interpolation() != filter::ColorInterpolation::SRGB {
        return Ok(None);
    }
    let filter::Kind::DropShadow(shadow) = primitive.kind() else {
        return Ok(None);
    };
    if shadow.input() != &filter::Input::SourceGraphic {
        return Ok(None);
    }
    let values = [
        shadow.dx(),
        shadow.dy(),
        shadow.std_dev_x().get(),
        shadow.std_dev_y().get(),
    ];
    if values.iter().any(|value| !value.is_finite()) {
        return Err("drop-shadow contains a non-finite parameter".to_owned());
    }
    Ok(Some(shadow))
}

pub(super) fn source_bounds(group: &Group, transform: Transform) -> Result<Option<Bounds>, String> {
    let bounds = bounds::map_rect(group.stroke_bounding_box(), transform)?;
    if bounds.left >= bounds.right || bounds.top >= bounds.bottom {
        return Ok(None);
    }
    Ok(Some(bounds))
}

pub(super) fn effect_bounds(
    source: Bounds,
    spec: ShadowSpec,
    transform: Transform,
) -> Result<Option<Bounds>, String> {
    let filter = surface::make_skia_filter(spec, transform, None)?;
    let source_rect = SkRect::from_ltrb(source.left, source.top, source.right, source.bottom);
    if !filter.can_compute_fast_bounds() {
        return Ok(None);
    }
    let effect = surface::skia_bounds(filter.compute_fast_bounds(source_rect))?;
    let filter = bounds::map_bounds(spec.filter_bounds, transform)?;
    let primitive = bounds::map_bounds(spec.primitive_bounds, transform)?;
    if !bounds::contains(filter, effect) || !bounds::contains(primitive, effect) {
        return Ok(None);
    }
    Ok(Some(effect))
}
