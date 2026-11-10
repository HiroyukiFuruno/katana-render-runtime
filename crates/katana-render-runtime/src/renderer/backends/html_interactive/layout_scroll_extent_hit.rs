use super::axes::{polygon_axes, projection_bounds};
use super::polygon::clip_polygon;
use super::{OverflowClip, Point, RECT_CORNER_COUNT, non_finite_geometry};

pub(in super::super) fn clip_contains_point(clip: &OverflowClip, x: f32, y: f32) -> bool {
    if !x.is_finite() || !y.is_finite() {
        return false;
    }
    let Ok(Some((position, size))) = clip_point_position(clip, (f64::from(x), f64::from(y))) else {
        return false;
    };
    clip.active_axes != (true, true) || rounded_point_is_inside(clip, position, size)
}

fn clip_point_position(
    clip: &OverflowClip,
    point: Point,
) -> Result<Option<(Point, Point)>, String> {
    let polygon = clip_polygon(clip)?;
    let axes = polygon_axes(&polygon, clip.active_axes)?;
    let mut values = [(0.0, 0.0); 2];
    let active = [clip.active_axes.0, clip.active_axes.1];
    for (index, axis) in [axes.local_x, axes.local_y].into_iter().enumerate() {
        if !active[index] {
            continue;
        }
        let Some(axis) = axis else { return Ok(None) };
        let Some(value) = point_axis_position(point, axes.origin, axis, &polygon)? else {
            return Ok(None);
        };
        values[index] = value;
    }
    Ok(Some((
        (values[0].0, values[1].0),
        (values[0].1, values[1].1),
    )))
}

fn point_axis_position(
    point: Point,
    origin: Point,
    axis: Point,
    polygon: &[Point; RECT_CORNER_COUNT],
) -> Result<Option<Point>, String> {
    let (minimum, maximum) = projection_bounds(polygon, origin, axis)?;
    let position = (point.0 - origin.0) * axis.0 + (point.1 - origin.1) * axis.1;
    if !position.is_finite() {
        return Err(non_finite_geometry());
    }
    Ok(
        (maximum > minimum && position >= minimum && position <= maximum)
            .then_some((position - minimum, maximum - minimum)),
    )
}

fn rounded_point_is_inside(clip: &OverflowClip, point: Point, size: Point) -> bool {
    clip.corner_radii
        .iter()
        .enumerate()
        .all(|(index, &(x, y))| {
            if !x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0 {
                return false;
            }
            let radius = (f64::from(x), f64::from(y));
            let corner_point = (
                if matches!(index, 1 | 2) {
                    size.0 - point.0
                } else {
                    point.0
                },
                if index >= 2 {
                    size.1 - point.1
                } else {
                    point.1
                },
            );
            rounded_corner_contains(corner_point, radius)
        })
}

fn rounded_corner_contains(point: Point, radius: Point) -> bool {
    if radius.0 == 0.0 || radius.1 == 0.0 || point.0 >= radius.0 || point.1 >= radius.1 {
        return true;
    }
    ((point.0 - radius.0) / radius.0).powi(2) + ((point.1 - radius.1) / radius.1).powi(2) <= 1.0
}

#[cfg(test)]
#[path = "layout_scroll_extent_hit_clip_hit_tests.rs"]
mod clip_hit_tests;
