use super::{RenderContext, clip_mask, render_children_with_context};
use resvg::usvg;
use tiny_skia::{PixmapMut, Transform};

#[cfg(test)]
#[path = "svg_rasterize_composite_full_canvas_clip_tests.rs"]
mod tests;

const MAX_CANDIDATE_SEGMENTS: usize = 5;
const FULL_COVERAGE: u8 = u8::MAX;

pub(super) fn render_if_covered(
    group: &usvg::Group,
    transform: Transform,
    pixmap: &mut PixmapMut<'_>,
    context: &RenderContext<'_, '_>,
) -> Result<bool, String> {
    let Some(clip) = group.clip_path() else {
        return Ok(false);
    };
    if !is_candidate(clip, transform, pixmap.width(), pixmap.height()) {
        return Ok(false);
    }
    let mask = clip_mask(clip, transform, pixmap.width(), pixmap.height())?;
    if !mask
        .data()
        .iter()
        .all(|coverage| *coverage == FULL_COVERAGE)
    {
        return Ok(false);
    }
    render_children_with_context(group, transform, pixmap, context)?;
    Ok(true)
}

fn is_candidate(clip: &usvg::ClipPath, transform: Transform, width: u32, height: u32) -> bool {
    let root = clip.root();
    let [usvg::Node::Path(path)] = root.children() else {
        return false;
    };
    if path.data().len() > MAX_CANDIDATE_SEGMENTS {
        return false;
    }
    let matrix = transform
        .pre_concat(clip.transform())
        .pre_concat(root.transform());
    let Some(bounds) = path.bounding_box().transform(matrix) else {
        return false;
    };
    bounds.x() <= 0.0
        && bounds.y() <= 0.0
        && bounds.right() >= width as f32
        && bounds.bottom() >= height as f32
}
