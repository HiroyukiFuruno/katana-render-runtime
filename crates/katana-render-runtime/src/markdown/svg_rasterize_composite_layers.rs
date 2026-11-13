use super::{AuthoredClipInfo, TextRasterizer, mask, render_children_at_origin};
use resvg::{render_node, usvg};
use tiny_skia::{BlendMode, IntRect, Pixmap, PixmapMut, PixmapPaint, Transform};

const LAYER_EDGE_PADDING: i32 = 2;
const LAYER_SIZE_PADDING: u32 = 4;

type LayerPainter<'a> = dyn FnMut(&mut PixmapMut<'_>) -> Result<(), String> + 'a;

#[cfg(test)]
#[path = "svg_rasterize_composite_layers_boundary_tests.rs"]
mod boundary_tests;

#[cfg(test)]
pub(super) fn render_clipped_group(
    group: &usvg::Group,
    transform: Transform,
    pixmap: &mut tiny_skia::PixmapMut<'_>,
    authored_clip_info: &AuthoredClipInfo,
    text_rasterizer: Option<&dyn TextRasterizer>,
) -> Result<(), String> {
    let Some(clip) = group.clip_path() else {
        return Ok(());
    };
    render_group_with_viewport_clip(
        group,
        (Some(clip), transform),
        pixmap,
        authored_clip_info,
        text_rasterizer,
        None,
    )
}

pub(super) fn render_group_with_viewport_clip(
    group: &usvg::Group,
    (clip, transform): (Option<&usvg::ClipPath>, Transform),
    pixmap: &mut tiny_skia::PixmapMut<'_>,
    authored_clip_info: &AuthoredClipInfo,
    text_rasterizer: Option<&dyn TextRasterizer>,
    origin: Option<super::TileContext>,
) -> Result<(), String> {
    let Some(clip) = clip else {
        return render_children_at_origin(
            group,
            transform,
            pixmap,
            authored_clip_info,
            text_rasterizer,
            origin,
        );
    };
    let Some((crop, crop_transform)) = crop_context(group, transform, pixmap)? else {
        return Ok(());
    };
    render_cropped_group(
        group,
        (clip, crop, crop_transform),
        pixmap,
        authored_clip_info,
        text_rasterizer,
        origin,
    )
}

pub(super) fn crop_context(
    group: &usvg::Group,
    transform: Transform,
    pixmap: &tiny_skia::PixmapMut<'_>,
) -> Result<Option<(IntRect, Transform)>, String> {
    let Some(crop) = clipped_group_rect(group, transform, pixmap)? else {
        return Ok(None);
    };
    let crop_transform =
        Transform::from_translate(-(crop.x() as f32), -(crop.y() as f32)).pre_concat(transform);
    Ok(Some((crop, crop_transform)))
}

fn render_cropped_group(
    group: &usvg::Group,
    (clip, rect, transform): (&usvg::ClipPath, IntRect, Transform),
    pixmap: &mut tiny_skia::PixmapMut<'_>,
    authored_clip_info: &AuthoredClipInfo,
    text_rasterizer: Option<&dyn TextRasterizer>,
    origin: Option<super::TileContext>,
) -> Result<(), String> {
    let mut composited = crop_pixmap(pixmap, rect)?;
    #[cfg(all(test, target_os = "macos"))]
    let _crop_scope = super::super::SvgRasterizeOps::enter_path_crop(rect);
    let child_origin = origin.map(|context| context.cropped(rect)).transpose()?;
    render_children_at_origin(
        group,
        transform,
        &mut composited.as_mut(),
        authored_clip_info,
        text_rasterizer,
        child_origin,
    )?;
    composite_clipped_crop(pixmap, rect, &composited, clip, transform)
}

fn composite_clipped_crop(
    pixmap: &mut tiny_skia::PixmapMut<'_>,
    crop: IntRect,
    composited: &Pixmap,
    clip: &usvg::ClipPath,
    transform: Transform,
) -> Result<(), String> {
    let mask = mask::clip_mask(clip, transform, composited.width(), composited.height())?;
    mask::composite_crop_with_mask(pixmap, crop, composited, &mask)
}

pub(super) fn render_layer(
    (width, height): (u32, u32),
    error: &str,
    paint: &mut LayerPainter<'_>,
) -> Result<Pixmap, String> {
    let mut layer = Pixmap::new(width, height).ok_or_else(|| error.to_owned())?;
    paint(&mut layer.as_mut())?;
    Ok(layer)
}

pub(super) fn clipped_group_rect(
    group: &usvg::Group,
    transform: Transform,
    pixmap: &tiny_skia::PixmapMut<'_>,
) -> Result<Option<IntRect>, String> {
    let Some(bbox) = group.layer_bounding_box().transform(transform) else {
        return Ok(None);
    };
    let x = (bbox.x().floor() as i32)
        .checked_sub(LAYER_EDGE_PADDING)
        .ok_or_else(|| "SVG clip layer x coordinate overflow".to_string())?;
    let y = (bbox.y().floor() as i32)
        .checked_sub(LAYER_EDGE_PADDING)
        .ok_or_else(|| "SVG clip layer y coordinate overflow".to_string())?;
    let width = (bbox.width().ceil().max(1.0) as u32)
        .checked_add(LAYER_SIZE_PADDING)
        .ok_or_else(|| "SVG clip layer width overflow".to_string())?;
    let height = (bbox.height().ceil().max(1.0) as u32)
        .checked_add(LAYER_SIZE_PADDING)
        .ok_or_else(|| "SVG clip layer height overflow".to_string())?;
    let layer = IntRect::from_xywh(x, y, width, height)
        .ok_or_else(|| "invalid SVG clip layer bounds".to_string())?;
    let canvas = canvas_rect(pixmap.width(), pixmap.height())?;
    Ok(layer.intersect(&canvas))
}

fn canvas_rect(width: u32, height: u32) -> Result<IntRect, String> {
    IntRect::from_xywh(0, 0, width, height).ok_or_else(|| "invalid SVG canvas bounds".to_owned())
}

fn crop_pixmap(pixmap: &tiny_skia::PixmapMut<'_>, crop: IntRect) -> Result<Pixmap, String> {
    render_layer(
        (crop.width(), crop.height()),
        "failed to allocate cropped SVG compositing pixmap",
        &mut |cropped| {
            let paint = PixmapPaint {
                blend_mode: BlendMode::Source,
                ..PixmapPaint::default()
            };
            cropped.draw_pixmap(
                -crop.x(),
                -crop.y(),
                pixmap.as_ref(),
                &paint,
                Transform::identity(),
                None,
            );
            Ok(())
        },
    )
}

pub(super) fn render_existing_node(
    node: &usvg::Node,
    transform: Transform,
    pixmap: &mut tiny_skia::PixmapMut<'_>,
) {
    let Some(bbox) = node.abs_layer_bounding_box() else {
        return;
    };
    let transform = transform.pre_translate(bbox.x(), bbox.y());
    let _ = render_node(node, transform, pixmap);
}
