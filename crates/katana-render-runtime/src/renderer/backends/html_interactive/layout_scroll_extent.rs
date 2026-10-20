#[path = "layout_scroll_extent_axes.rs"]
mod axes;
#[path = "layout_scroll_extent_axis_aligned.rs"]
mod axis_aligned;
#[path = "layout_scroll_extent_hit.rs"]
mod hit;
#[path = "layout_scroll_extent_plane.rs"]
mod plane;
#[path = "layout_scroll_extent_polygon.rs"]
mod polygon;

use axis_aligned::{axis_aligned_extent, bounds, polygon_right_bottom, visible_right_bottom};
pub(super) use hit::clip_contains_point;
use polygon::clip_owner_polygon;
pub(super) use polygon::clip_viewport_polygon;

use super::types::{ElementBox, ElementPositioningContext, OverflowClip};
use std::collections::HashMap;

type Point = (f64, f64);
type Extent = (f32, f32);

struct OwnerAxes {
    origin: Point,
    local_x: Option<Point>,
    local_y: Option<Point>,
}

enum AxisAlignedExtent {
    NotApplicable,
    Empty,
    Visible(Extent),
}

#[derive(Default)]
struct PolygonScratch {
    first: Vec<Point>,
    second: Vec<Point>,
}

impl PolygonScratch {
    fn current(&self, current_first: bool) -> &[Point] {
        if current_first {
            &self.first
        } else {
            &self.second
        }
    }
}

pub(super) const RECT_CORNER_COUNT: usize = 4;
pub(super) const BOTTOM_LEFT_CORNER: usize = 3;
const AXIS_ALIGNMENT_EPSILON: f32 = 0.000_01;

pub(super) fn scrollable_extent(
    element_boxes: &[ElementBox],
    indices: &HashMap<u64, usize>,
    viewport_width: f32,
    viewport_height: f32,
    flow_height: f32,
) -> Result<(f32, f32), String> {
    let mut extent = (viewport_width, viewport_height.max(flow_height));
    let mut scratch = PolygonScratch::default();
    for element in element_boxes {
        update_element_extent(&mut extent, element, element_boxes, indices, &mut scratch)?;
    }
    Ok(extent)
}

fn update_element_extent(
    extent: &mut Extent,
    element: &ElementBox,
    boxes: &[ElementBox],
    indices: &HashMap<u64, usize>,
    scratch: &mut PolygonScratch,
) -> Result<(), String> {
    if element.positioning_context == ElementPositioningContext::FixedViewport {
        return Ok(());
    }
    update_extent(
        extent,
        &element.transformed_corners,
        &element.overflow_clips,
        boxes,
        indices,
        scratch,
    )?;
    update_inline_extent(extent, element, boxes, indices, scratch)
}

fn update_inline_extent(
    extent: &mut Extent,
    element: &ElementBox,
    boxes: &[ElementBox],
    indices: &HashMap<u64, usize>,
    scratch: &mut PolygonScratch,
) -> Result<(), String> {
    for fragment in &element.inline_fragments {
        update_extent(
            extent,
            &fragment.transformed_corners,
            &element.overflow_clips,
            boxes,
            indices,
            scratch,
        )?;
    }
    Ok(())
}

fn update_extent(
    extent: &mut Extent,
    corners: &[(f32, f32); RECT_CORNER_COUNT],
    clips: &[OverflowClip],
    boxes: &[ElementBox],
    indices: &HashMap<u64, usize>,
    scratch: &mut PolygonScratch,
) -> Result<(), String> {
    if let Some((right, bottom)) = visible_extent(corners, clips, boxes, indices, scratch)? {
        extent.0 = extent.0.max(right);
        extent.1 = extent.1.max(bottom);
    }
    Ok(())
}

fn visible_extent(
    corners: &[(f32, f32); RECT_CORNER_COUNT],
    clips: &[OverflowClip],
    boxes: &[ElementBox],
    indices: &HashMap<u64, usize>,
    scratch: &mut PolygonScratch,
) -> Result<Option<(f32, f32)>, String> {
    let bounds = bounds(corners)?;
    if clips.is_empty() {
        return Ok(visible_right_bottom(bounds));
    }
    match axis_aligned_extent(bounds, corners, clips, boxes, indices)? {
        AxisAlignedExtent::Visible(extent) => return Ok(Some(extent)),
        AxisAlignedExtent::Empty => return Ok(None),
        AxisAlignedExtent::NotApplicable => {}
    }
    rotated_visible_extent(corners, clips, boxes, indices, scratch)
}

fn rotated_visible_extent(
    corners: &[(f32, f32); RECT_CORNER_COUNT],
    clips: &[OverflowClip],
    boxes: &[ElementBox],
    indices: &HashMap<u64, usize>,
    scratch: &mut PolygonScratch,
) -> Result<Option<Extent>, String> {
    scratch.first.clear();
    scratch
        .first
        .extend(corners.iter().map(|(x, y)| (f64::from(*x), f64::from(*y))));
    let mut current_first = true;
    for clip in clips {
        if !clip_owner_polygon(clip, boxes, indices, scratch, &mut current_first)? {
            return Ok(None);
        }
    }
    let Some((right, bottom)) = polygon_right_bottom(scratch.current(current_first))? else {
        return Ok(None);
    };
    Ok((right > 0.0 && bottom > 0.0).then_some((right, bottom)))
}

fn clip_owner<'a>(
    clip: &OverflowClip,
    boxes: &'a [ElementBox],
    indices: &HashMap<u64, usize>,
) -> Result<&'a ElementBox, String> {
    indices
        .get(&clip.owner_node_id)
        .and_then(|index| boxes.get(*index))
        .filter(|owner| owner.node_id == clip.owner_node_id)
        .ok_or_else(|| "scrollable overflow clip owner is missing".to_owned())
}
fn non_finite_geometry() -> String {
    "scrollable overflow contains non-finite geometry".to_owned()
}

#[cfg(test)]
#[path = "layout_scroll_extent_geometry_tests.rs"]
mod geometry_tests;
