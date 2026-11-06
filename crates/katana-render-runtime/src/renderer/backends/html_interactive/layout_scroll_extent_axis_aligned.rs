use super::AXIS_ALIGNMENT_EPSILON;
use super::{
    AxisAlignedExtent, BOTTOM_LEFT_CORNER, ElementBox, HashMap, OverflowClip, Point,
    RECT_CORNER_COUNT, clip_owner, non_finite_geometry,
};

pub(super) fn axis_aligned_extent(
    (left, top, right, bottom): (f32, f32, f32, f32),
    subject: &[(f32, f32); RECT_CORNER_COUNT],
    clips: &[OverflowClip],
    boxes: &[ElementBox],
    indices: &HashMap<u64, usize>,
) -> Result<AxisAlignedExtent, String> {
    if !is_axis_aligned(subject) {
        return Ok(AxisAlignedExtent::NotApplicable);
    }
    let mut bounds = (left, top, right, bottom);
    for clip in clips {
        if !intersect_axis_aligned_clip(&mut bounds, clip, boxes, indices)? {
            return Ok(AxisAlignedExtent::NotApplicable);
        }
    }
    let (left, top, right, bottom) = bounds;
    /* WHY: 片軸のクリップが空でも、反対軸のスクロール範囲は残る。 */
    let horizontal = positive_axis_extent(left, right);
    let vertical = positive_axis_extent(top, bottom);
    Ok(if horizontal > 0.0 || vertical > 0.0 {
        AxisAlignedExtent::Visible((horizontal, vertical))
    } else {
        AxisAlignedExtent::Empty
    })
}

fn positive_axis_extent(start: f32, end: f32) -> f32 {
    if end > start { end.max(0.0) } else { 0.0 }
}

fn intersect_axis_aligned_clip(
    subject: &mut (f32, f32, f32, f32),
    clip: &OverflowClip,
    boxes: &[ElementBox],
    indices: &HashMap<u64, usize>,
) -> Result<bool, String> {
    let owner = clip_owner(clip, boxes, indices)?;
    if !is_axis_aligned(&owner.transformed_corners) || !is_axis_aligned(&clip.transformed_corners) {
        return Ok(false);
    }
    let Some(axes) = local_axes_world_axis(&owner.transformed_corners) else {
        return Ok(false);
    };
    let clip_bounds = bounds(&clip.transformed_corners)?;
    if owner.overflow_clip_axes.0 {
        intersect_world_axis(subject, clip_bounds, axes.0);
    }
    if owner.overflow_clip_axes.1 {
        intersect_world_axis(subject, clip_bounds, axes.1);
    }
    Ok(true)
}

fn intersect_world_axis(
    subject: &mut (f32, f32, f32, f32),
    clip: (f32, f32, f32, f32),
    axis: usize,
) {
    if axis == 0 {
        subject.0 = subject.0.max(clip.0);
        subject.2 = subject.2.min(clip.2);
    } else {
        subject.1 = subject.1.max(clip.1);
        subject.3 = subject.3.min(clip.3);
    }
}

fn local_axes_world_axis(corners: &[(f32, f32); RECT_CORNER_COUNT]) -> Option<(usize, usize)> {
    let local_x = (corners[1].0 - corners[0].0, corners[1].1 - corners[0].1);
    let local_y = (
        corners[BOTTOM_LEFT_CORNER].0 - corners[0].0,
        corners[BOTTOM_LEFT_CORNER].1 - corners[0].1,
    );
    if local_x.0.abs() > local_x.1.abs() && local_y.1.abs() > local_y.0.abs() {
        Some((0, 1))
    } else if local_x.1.abs() > local_x.0.abs() && local_y.0.abs() > local_y.1.abs() {
        Some((1, 0))
    } else {
        None
    }
}

fn is_axis_aligned(corners: &[(f32, f32); RECT_CORNER_COUNT]) -> bool {
    corners.iter().all(|(x, y)| x.is_finite() && y.is_finite())
        && (0..RECT_CORNER_COUNT).all(|index| {
            let current = corners[index];
            let next = corners[(index + 1) % RECT_CORNER_COUNT];
            let dx = (next.0 - current.0).abs();
            let dy = (next.1 - current.1).abs();
            dx <= AXIS_ALIGNMENT_EPSILON || dy <= AXIS_ALIGNMENT_EPSILON
        })
}

pub(super) fn polygon_right_bottom(polygon: &[Point]) -> Result<Option<(f32, f32)>, String> {
    let left = polygon
        .iter()
        .map(|point| point.0)
        .fold(f64::INFINITY, f64::min);
    let top = polygon
        .iter()
        .map(|point| point.1)
        .fold(f64::INFINITY, f64::min);
    let right = polygon
        .iter()
        .map(|point| point.0)
        .fold(f64::NEG_INFINITY, f64::max);
    let bottom = polygon
        .iter()
        .map(|point| point.1)
        .fold(f64::NEG_INFINITY, f64::max);
    if ![left, top, right, bottom].into_iter().all(f64::is_finite) {
        return Err(non_finite_geometry());
    }
    if right <= left || bottom <= top {
        return Ok(None);
    }
    let (right, bottom) = (right as f32, bottom as f32);
    if !right.is_finite() || !bottom.is_finite() {
        return Err(non_finite_geometry());
    }
    Ok(Some((right, bottom)))
}

pub(super) fn bounds(
    corners: &[(f32, f32); RECT_CORNER_COUNT],
) -> Result<(f32, f32, f32, f32), String> {
    if !corners.iter().all(|(x, y)| x.is_finite() && y.is_finite()) {
        return Err(non_finite_geometry());
    }
    let left = corners
        .iter()
        .map(|(x, _)| *x)
        .fold(f32::INFINITY, f32::min);
    let right = corners
        .iter()
        .map(|(x, _)| *x)
        .fold(f32::NEG_INFINITY, f32::max);
    let top = corners
        .iter()
        .map(|(_, y)| *y)
        .fold(f32::INFINITY, f32::min);
    let bottom = corners
        .iter()
        .map(|(_, y)| *y)
        .fold(f32::NEG_INFINITY, f32::max);
    Ok((left, top, right, bottom))
}

pub(super) fn visible_right_bottom(
    (left, top, right, bottom): (f32, f32, f32, f32),
) -> Option<(f32, f32)> {
    (right > left && bottom > top && right > 0.0 && bottom > 0.0).then_some((right, bottom))
}
