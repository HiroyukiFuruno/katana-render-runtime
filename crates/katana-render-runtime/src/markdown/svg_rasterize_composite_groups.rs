use super::{AuthoredClipInfo, RenderContext, TextRasterizer, layers};
use resvg::usvg;
use tiny_skia::{IntRect, PixmapMut, Transform};

pub(super) fn render_clipped_with_extra(
    group: &usvg::Group,
    transform: Transform,
    pixmap: &mut tiny_skia::PixmapMut<'_>,
    context: &RenderContext<'_, '_>,
    extra: Option<usvg::Rect>,
) -> Result<(), String> {
    let planned = extra.and_then(|bounds| planned_crop(group, transform, pixmap, bounds));
    let strict = extra.is_some() && planned.is_none();
    /* WHY: 計画の境界が不確かなら子も旧経路に揃え、描画の途中で切り替えない。 */
    let fallback = RenderContext {
        provenance: context.provenance,
        text: context.text,
        plan: None,
    };
    let selected = if strict { &fallback } else { context };
    let crop = if strict || extra.is_none() {
        layers::crop_context(group, transform, pixmap)?
    } else {
        planned.flatten()
    };
    let Some((crop, crop_transform)) = crop else {
        return Ok(());
    };
    layers::render_cropped_group(group, crop, crop_transform, pixmap, selected)
}

pub(super) fn planned_crop(
    group: &usvg::Group,
    transform: Transform,
    pixmap: &tiny_skia::PixmapMut<'_>,
    extra: usvg::Rect,
) -> Option<Option<(IntRect, Transform)>> {
    let bounds = planned_group_bounds(group, Some(extra)).ok()?;
    let bounds = super::arc_plan::transform_bounds(bounds, transform)?;
    let crop = super::layers::device_crop(bounds, pixmap).ok()?;
    let Some(crop) = crop else {
        return Some(None);
    };
    let crop_transform =
        Transform::from_translate(-(crop.x() as f32), -(crop.y() as f32)).pre_concat(transform);
    if !super::path::transform_is_finite(crop_transform) {
        return None;
    }
    Some(Some((crop, crop_transform)))
}

#[path = "svg_rasterize_composite_opacity.rs"]
mod opacity;

pub(super) fn planned_group_bounds(
    group: &usvg::Group,
    extra: Option<usvg::Rect>,
) -> Result<usvg::Rect, String> {
    let base = group.layer_bounding_box();
    let Some(extra) = extra else {
        return Ok(base.to_rect());
    };
    /* WHY: 旧境界と新しいインク範囲の両方をクリップ前に保持する。 */
    usvg::Rect::from_ltrb(
        base.x().min(extra.x()),
        base.y().min(extra.y()),
        base.right().max(extra.right()),
        base.bottom().max(extra.bottom()),
    )
    .ok_or_else(|| "invalid planned SVG clip bounds".to_owned())
}

#[cfg(all(test, target_os = "macos"))]
#[path = "svg_rasterize_composite_groups_opacity_tests.rs"]
mod opacity_tests;

pub(super) fn render_group_node(
    node: &usvg::Node,
    group: &usvg::Group,
    transform: Transform,
    pixmap: &mut PixmapMut<'_>,
    context: &RenderContext<'_, '_>,
) -> Result<(), String> {
    if super::is_traversable_group(group) {
        super::render_traversable_group(node, group, transform, pixmap, context)
    } else {
        render_effect_group(
            node,
            group,
            transform,
            pixmap,
            context.provenance,
            context.text,
        )
    }
}

fn render_effect_group(
    node: &usvg::Node,
    group: &usvg::Group,
    transform: Transform,
    pixmap: &mut PixmapMut<'_>,
    provenance: &AuthoredClipInfo,
    text: Option<&dyn TextRasterizer>,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    if render_linear_component_transfer(group, transform, pixmap, provenance, text)? {
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    if text.is_some() && render_shadow_group(group, transform, pixmap, provenance, text)? {
        return Ok(());
    }
    if let Some(text_rasterizer) = text.filter(|_| opacity::supports(group)) {
        return opacity::render(group, transform, pixmap, provenance, text_rasterizer);
    }
    layers::render_existing_node(node, transform, pixmap);
    Ok(())
}

#[cfg(target_os = "macos")]
fn render_linear_component_transfer(
    group: &usvg::Group,
    transform: Transform,
    pixmap: &mut PixmapMut<'_>,
    provenance: &AuthoredClipInfo,
    text: Option<&dyn TextRasterizer>,
) -> Result<bool, String> {
    let Some(text_rasterizer) = text else {
        return Ok(false);
    };
    let child_transform = transform.pre_concat(group.transform());
    super::linear_component_transfer::render(
        group,
        child_transform,
        pixmap,
        provenance,
        text_rasterizer,
    )
}

#[cfg(target_os = "macos")]
fn render_shadow_group(
    group: &usvg::Group,
    transform: Transform,
    pixmap: &mut PixmapMut<'_>,
    provenance: &AuthoredClipInfo,
    text: Option<&dyn TextRasterizer>,
) -> Result<bool, String> {
    super::super::html_shadow::HtmlShadowRasterizer::render(
        group,
        transform.pre_concat(group.transform()),
        pixmap,
        &mut |local, target| {
            super::render_children_with_text(group, local, target, provenance, text)
        },
    )
}
