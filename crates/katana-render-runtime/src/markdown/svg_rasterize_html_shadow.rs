//! feDropShadow の限定経路。SourceGraphic の走査と既存合成は呼び出し側に委譲する。
//!
//! 既存経路の描画順序・clip・blend を再実装しないため、ここでは bounded な Skia
//! filter surface だけを提供する。

use resvg::usvg::Group;
use tiny_skia::{PixmapMut, Transform};

#[path = "svg_rasterize_html_shadow_bounds.rs"]
mod bounds;
#[path = "svg_rasterize_html_shadow_geometry.rs"]
mod geometry;
#[path = "svg_rasterize_html_shadow_surface.rs"]
mod surface;
#[cfg(test)]
#[path = "svg_rasterize_html_shadow_tests.rs"]
mod tests;

pub(super) use bounds::{Bounds, Crop, bounded_crop, map_rect, supported_transform};

pub(super) type ShadowSourceRenderer<'a> =
    dyn FnMut(Transform, &mut PixmapMut<'_>) -> Result<(), String> + 'a;

pub(super) struct HtmlShadowRasterizer;

pub(super) fn apply_color_filter(
    source: &tiny_skia::Pixmap,
    filter: skia_safe::ColorFilter,
) -> Result<tiny_skia::Pixmap, String> {
    surface::apply_color_filter(source, filter)
}

pub(super) fn copy_source_over(
    target: &mut tiny_skia::PixmapMut<'_>,
    source: &tiny_skia::Pixmap,
    left: i32,
    top: i32,
) {
    surface::copy_source_over(target, source, left, top);
}

impl HtmlShadowRasterizer {
    pub(super) fn render(
        group: &Group,
        child_transform: Transform,
        target: &mut PixmapMut<'_>,
        source_renderer: &mut ShadowSourceRenderer<'_>,
    ) -> Result<bool, String> {
        if !geometry::supported_transform(child_transform) {
            return Ok(false);
        }
        let Some(spec) = geometry::shadow_spec(group)? else {
            return Ok(false);
        };
        let Some(source) = geometry::source_bounds(group, child_transform)? else {
            return Ok(false);
        };
        let Some(effect) = geometry::effect_bounds(source, spec, child_transform)? else {
            return Ok(false);
        };
        let Some(crop) = geometry::bounded_crop(effect, target) else {
            return Ok(false);
        };
        let source_pixmap = render_source(crop, child_transform, source_renderer)?;
        let filtered = surface::apply_skia_shadow(&source_pixmap, spec, child_transform, crop)?;
        surface::copy_source_over(target, &filtered, crop.left, crop.top);
        Ok(true)
    }
}

fn render_source(
    crop: geometry::Crop,
    child_transform: Transform,
    source_renderer: &mut ShadowSourceRenderer<'_>,
) -> Result<tiny_skia::Pixmap, String> {
    let mut source_pixmap = tiny_skia::Pixmap::new(crop.width(), crop.height())
        .ok_or_else(|| "drop-shadow source allocation failed".to_owned())?;
    let local = Transform::from_translate(-(crop.left as f32), -(crop.top as f32))
        .pre_concat(child_transform);
    let mut source_target = source_pixmap.as_mut();
    source_renderer(local, &mut source_target)?;
    Ok(source_pixmap)
}
