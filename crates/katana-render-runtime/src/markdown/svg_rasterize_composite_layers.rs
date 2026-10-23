use super::{AuthoredClipInfo, render_children};
use resvg::{render_node, usvg};
use tiny_skia::{BlendMode, IntRect, Mask, MaskType, Pixmap, PixmapPaint, Transform};

const LAYER_EDGE_PADDING: i32 = 2;
const LAYER_SIZE_PADDING: u32 = 4;

pub(super) fn render_clipped_group(
    group: &usvg::Group,
    transform: Transform,
    pixmap: &mut tiny_skia::PixmapMut<'_>,
    authored_clip_info: &AuthoredClipInfo,
) -> Result<(), String> {
    let Some((crop, crop_transform)) = crop_context(group, transform, pixmap)? else {
        return Ok(());
    };
    render_cropped_group(group, crop, crop_transform, pixmap, authored_clip_info)
}

fn crop_context(
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
    crop: IntRect,
    crop_transform: Transform,
    pixmap: &mut tiny_skia::PixmapMut<'_>,
    authored_clip_info: &AuthoredClipInfo,
) -> Result<(), String> {
    let mut composited = crop_pixmap(pixmap, crop)?;
    let mut backdrop = composited.clone();
    let Some(clip) = group.clip_path() else {
        return Ok(());
    };
    render_children(
        group,
        crop_transform,
        &mut composited.as_mut(),
        authored_clip_info,
    )?;
    let mask = clip_mask(
        clip,
        crop_transform,
        composited.width(),
        composited.height(),
    )?;
    composite_with_mask(&mut backdrop, &composited, &mask);
    write_crop(pixmap, crop, &backdrop);
    Ok(())
}

fn clip_mask(
    clip: &usvg::ClipPath,
    transform: Transform,
    width: u32,
    height: u32,
) -> Result<Mask, String> {
    let mut clip_pixels = Pixmap::new(width, height)
        .ok_or_else(|| "failed to allocate cropped SVG clip pixmap".to_string())?;
    render_clip_path(clip, transform, &mut clip_pixels)?;
    Ok(Mask::from_pixmap(clip_pixels.as_ref(), MaskType::Alpha))
}

fn composite_with_mask(pixmap: &mut Pixmap, source: &Pixmap, mask: &Mask) {
    let mut inverse_mask = mask.clone();
    inverse_mask.invert();
    pixmap.apply_mask(&inverse_mask);
    let paint = PixmapPaint {
        blend_mode: BlendMode::Plus,
        ..PixmapPaint::default()
    };
    pixmap.draw_pixmap(
        0,
        0,
        source.as_ref(),
        &paint,
        Transform::identity(),
        Some(mask),
    );
}

fn write_crop(pixmap: &mut tiny_skia::PixmapMut<'_>, crop: IntRect, source: &Pixmap) {
    let paint = PixmapPaint {
        blend_mode: BlendMode::Source,
        ..PixmapPaint::default()
    };
    pixmap.draw_pixmap(
        crop.x(),
        crop.y(),
        source.as_ref(),
        &paint,
        Transform::identity(),
        None,
    );
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
    let canvas = IntRect::from_xywh(0, 0, pixmap.width(), pixmap.height())
        .ok_or_else(|| "invalid SVG canvas bounds".to_string())?;
    Ok(layer.intersect(&canvas))
}

fn crop_pixmap(pixmap: &tiny_skia::PixmapMut<'_>, crop: IntRect) -> Result<Pixmap, String> {
    let mut cropped = Pixmap::new(crop.width(), crop.height())
        .ok_or_else(|| "failed to allocate cropped SVG compositing pixmap".to_string())?;
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
    Ok(cropped)
}

fn render_clip_path(
    clip: &usvg::ClipPath,
    transform: Transform,
    pixmap: &mut Pixmap,
) -> Result<(), String> {
    let node = usvg::Node::Group(Box::new(clip.root().clone()));
    render_existing_node(
        &node,
        transform.pre_concat(clip.transform()),
        &mut pixmap.as_mut(),
    );

    if let Some(nested) = clip.clip_path() {
        let mut nested_pixels = Pixmap::new(pixmap.width(), pixmap.height())
            .ok_or_else(|| "failed to allocate nested SVG clip pixmap".to_string())?;
        render_clip_path(nested, transform, &mut nested_pixels)?;
        let nested_mask = Mask::from_pixmap(nested_pixels.as_ref(), MaskType::Alpha);
        pixmap.apply_mask(&nested_mask);
    }
    Ok(())
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
