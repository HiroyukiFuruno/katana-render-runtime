use super::layers::{allocate_pixmap, render_existing_node};
use resvg::usvg;
use tiny_skia::{IntRect, Mask, MaskType, Pixmap, PixmapMut, Transform};

const RGBA_CHANNELS: usize = 4;
const MASK_MAX: u32 = 255;
const ROUNDING_BIAS: u32 = 127;

pub(super) fn composite_crop_with_mask(
    pixmap: &mut PixmapMut<'_>,
    crop: IntRect,
    source: &Pixmap,
    mask: &Mask,
) -> Result<(), String> {
    validate_crop(pixmap, crop, source, mask)?;
    let crop_width = crop.width() as usize;
    let crop_height = crop.height() as usize;
    let canvas_width = pixmap.width() as usize;
    let x = crop.x() as usize;
    let y = crop.y() as usize;
    let destination = pixmap.data_mut();
    for row in 0..crop_height {
        let destination_start = ((y + row) * canvas_width + x) * RGBA_CHANNELS;
        let destination_end = destination_start + crop_width * RGBA_CHANNELS;
        let destination_row = &mut destination[destination_start..destination_end];
        let source_row_start = row * crop_width * RGBA_CHANNELS;
        let source_row_end = source_row_start + crop_width * RGBA_CHANNELS;
        blend_row(
            destination_row,
            &source.data()[source_row_start..source_row_end],
            &mask.data()[row * crop_width..(row + 1) * crop_width],
        );
    }
    Ok(())
}

fn validate_crop(
    pixmap: &PixmapMut<'_>,
    crop: IntRect,
    source: &Pixmap,
    mask: &Mask,
) -> Result<(), String> {
    if crop.x() < 0
        || crop.y() < 0
        || crop.right() > pixmap.width() as i32
        || crop.bottom() > pixmap.height() as i32
        || source.width() != crop.width()
        || source.height() != crop.height()
        || mask.width() != crop.width()
        || mask.height() != crop.height()
    {
        return Err("SVG clip composite dimensions differ".to_string());
    }
    Ok(())
}

fn blend_row(destination: &mut [u8], source: &[u8], mask: &[u8]) {
    for ((destination, source), coverage) in destination
        .chunks_exact_mut(RGBA_CHANNELS)
        .zip(source.chunks_exact(RGBA_CHANNELS))
        .zip(mask)
    {
        blend_pixel(destination, source, *coverage);
    }
}

fn blend_pixel(destination: &mut [u8], source: &[u8], coverage: u8) {
    if coverage == 0 || destination == source {
        return;
    }
    if coverage == MASK_MAX as u8 {
        destination.copy_from_slice(source);
        return;
    }
    let coverage = u32::from(coverage);
    for (destination, source) in destination.iter_mut().zip(source) {
        let value = u32::from(*destination) * (MASK_MAX - coverage) + u32::from(*source) * coverage;
        *destination = ((value + ROUNDING_BIAS) / MASK_MAX) as u8;
    }
}

pub(super) fn clip_mask(
    clip: &usvg::ClipPath,
    transform: Transform,
    width: u32,
    height: u32,
) -> Result<Mask, String> {
    let mut clip_pixels =
        allocate_pixmap(width, height, "failed to allocate cropped SVG clip pixmap")?;
    render_clip_path(clip, transform, &mut clip_pixels)?;
    Ok(Mask::from_pixmap(clip_pixels.as_ref(), MaskType::Alpha))
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
        let mut nested_pixels = allocate_pixmap(
            pixmap.width(),
            pixmap.height(),
            "failed to allocate nested SVG clip pixmap",
        )?;
        render_clip_path(nested, transform, &mut nested_pixels)?;
        let nested_mask = Mask::from_pixmap(nested_pixels.as_ref(), MaskType::Alpha);
        pixmap.apply_mask(&nested_mask);
    }
    Ok(())
}
