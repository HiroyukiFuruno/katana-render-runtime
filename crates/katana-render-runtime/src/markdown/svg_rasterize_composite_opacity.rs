use super::super::{AuthoredClipInfo, TextRasterizer, layers, render_children_with_text};
use resvg::usvg;
use tiny_skia::{BlendMode, IntRect, Pixmap, PixmapMut, PixmapPaint, Transform};

const FULL_OPACITY: f32 = 1.0;

pub(super) fn supports(group: &usvg::Group) -> bool {
    group.opacity().get() < FULL_OPACITY
        && group.blend_mode() == usvg::BlendMode::Normal
        && group.filters().is_empty()
        && group.mask().is_none()
        && group.clip_path().is_none()
        && contains_text(group)
}

fn contains_text(group: &usvg::Group) -> bool {
    group.children().iter().any(|child| match child {
        usvg::Node::Text(_) => true,
        usvg::Node::Group(nested) => contains_text(nested),
        _ => false,
    })
}

pub(super) fn render(
    group: &usvg::Group,
    transform: Transform,
    pixmap: &mut PixmapMut<'_>,
    authored_clip_info: &AuthoredClipInfo,
    text_rasterizer: &dyn TextRasterizer,
) -> Result<(), String> {
    let group_transform = transform.pre_concat(group.transform());
    let Some((crop, crop_transform)) = layers::crop_context(group, group_transform, pixmap)? else {
        return Ok(());
    };
    let mut layer = layers::allocate_pixmap(
        crop.width(),
        crop.height(),
        "failed to allocate SVG opacity layer",
    )?;
    render_children_with_text(
        group,
        crop_transform,
        &mut layer.as_mut(),
        authored_clip_info,
        Some(text_rasterizer),
    )?;
    composite_layer(pixmap, crop, &layer, group.opacity().get());
    Ok(())
}

fn composite_layer(pixmap: &mut PixmapMut<'_>, crop: IntRect, layer: &Pixmap, opacity: f32) {
    let paint = PixmapPaint {
        opacity,
        blend_mode: BlendMode::SourceOver,
        ..PixmapPaint::default()
    };
    pixmap.draw_pixmap(
        crop.x(),
        crop.y(),
        layer.as_ref(),
        &paint,
        Transform::identity(),
        None,
    );
}
