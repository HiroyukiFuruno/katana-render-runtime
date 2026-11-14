use super::axes::{owner_axes, polygon_axes, projection_bounds};
use super::plane::clip_plane;
use super::{
    ElementBox, HashMap, OverflowClip, OwnerAxes, Point, PolygonScratch, RECT_CORNER_COUNT,
    clip_owner, non_finite_geometry,
};

pub(super) fn clip_owner_polygon(
    clip: &OverflowClip,
    boxes: &[ElementBox],
    indices: &HashMap<u64, usize>,
    scratch: &mut PolygonScratch,
    current_first: &mut bool,
) -> Result<bool, String> {
    let owner = clip_owner(clip, boxes, indices)?;
    let active_axes = owner.overflow_clip_axes;
    if !active_axes.0 && !active_axes.1 {
        return Ok(true);
    }
    let axes = owner_axes(owner, active_axes)?;
    let clip_polygon = clip_polygon(clip)?;
    clip_active_axis_planes(active_axes, &axes, &clip_polygon, scratch, current_first)
}

fn clip_active_axis_planes(
    active_axes: (bool, bool),
    axes: &OwnerAxes,
    polygon: &[Point; RECT_CORNER_COUNT],
    scratch: &mut PolygonScratch,
    current_first: &mut bool,
) -> Result<bool, String> {
    for (active, axis) in [(active_axes.0, axes.local_x), (active_axes.1, axes.local_y)] {
        if !active {
            continue;
        }
        let Some(axis) = axis else {
            return Ok(false);
        };
        if !clip_axis_range(scratch, current_first, axes.origin, axis, polygon)? {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(in super::super) fn clip_viewport_polygon(
    clip: &OverflowClip,
    viewport: &[(f64, f64); RECT_CORNER_COUNT],
) -> Result<Vec<(f64, f64)>, String> {
    if !viewport.iter().all(|(x, y)| x.is_finite() && y.is_finite()) {
        return Err(non_finite_geometry());
    }
    let (polygon, axes) = clip_polygon_axes(clip)?;
    let mut scratch = PolygonScratch {
        first: viewport.to_vec(),
        second: Vec::new(),
    };
    let mut first = true;
    if !clip_active_axis_planes(clip.active_axes, &axes, &polygon, &mut scratch, &mut first)? {
        return Ok(Vec::new());
    }
    Ok(if first { scratch.first } else { scratch.second })
}

pub(super) fn clip_polygon(clip: &OverflowClip) -> Result<[Point; RECT_CORNER_COUNT], String> {
    let polygon = clip_corners(clip);
    if !polygon.iter().all(|(x, y)| x.is_finite() && y.is_finite()) {
        return Err(non_finite_geometry());
    }
    Ok(polygon)
}

pub(super) fn clip_polygon_axes(
    clip: &OverflowClip,
) -> Result<([Point; RECT_CORNER_COUNT], OwnerAxes), String> {
    let polygon = clip_corners(clip);
    let axes = polygon_axes(&polygon, clip.active_axes)?;
    Ok((polygon, axes))
}

fn clip_corners(clip: &OverflowClip) -> [Point; RECT_CORNER_COUNT] {
    clip.transformed_corners
        .map(|(x, y)| (f64::from(x), f64::from(y)))
}

fn clip_axis_range(
    scratch: &mut PolygonScratch,
    current_first: &mut bool,
    origin: Point,
    axis: Point,
    clip_polygon: &[Point; RECT_CORNER_COUNT],
) -> Result<bool, String> {
    let (minimum, maximum) = projection_bounds(clip_polygon, origin, axis)?;
    if maximum <= minimum {
        return Ok(false);
    }
    clip_current_plane(scratch, current_first, origin, axis, minimum, true)?;
    if scratch.current(*current_first).is_empty() {
        return Ok(false);
    }
    clip_current_plane(scratch, current_first, origin, axis, maximum, false)?;
    Ok(!scratch.current(*current_first).is_empty())
}

fn clip_current_plane(
    scratch: &mut PolygonScratch,
    current_first: &mut bool,
    origin: Point,
    axis: Point,
    boundary: f64,
    keep_greater: bool,
) -> Result<(), String> {
    if *current_first {
        clip_plane(
            &scratch.first,
            &mut scratch.second,
            origin,
            axis,
            boundary,
            keep_greater,
        )?;
    } else {
        clip_plane(
            &scratch.second,
            &mut scratch.first,
            origin,
            axis,
            boundary,
            keep_greater,
        )?;
    }
    *current_first = !*current_first;
    Ok(())
}
