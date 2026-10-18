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
    let Some(axes) = polygon_axes(&polygon, clip.active_axes)? else {
        return Ok(None);
    };
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
mod clip_hit_tests {
    use super::{OverflowClip, clip_contains_point};

    fn clip(axes: (bool, bool), width: f32, height: f32) -> OverflowClip {
        OverflowClip::new(1, 0.0, 0.0, width, height, 10.0, 10.0).with_active_axes(axes)
    }

    #[test]
    fn only_active_axes_restrict_hits_and_rounding_requires_both() {
        for (axes, point, expected) in [
            ((true, false), (20.0, 500.0), true),
            ((true, false), (120.0, 20.0), false),
            ((false, true), (500.0, 20.0), true),
            ((false, true), (20.0, 120.0), false),
            ((true, true), (1.0, 1.0), false),
            ((true, false), (1.0, 1.0), true),
            ((true, true), (10.0, 1.0), true),
        ] {
            assert_eq!(
                clip_contains_point(&clip(axes, 100.0, 100.0), point.0, point.1),
                expected
            );
        }
    }

    #[test]
    fn rotated_axis_clips_in_its_local_direction() {
        let mut clip = clip((true, false), 100.0, 40.0);
        clip.transformed_corners = [(0.0, 0.0), (0.0, 100.0), (-40.0, 100.0), (-40.0, 0.0)];
        assert!(clip_contains_point(&clip, -500.0, 50.0));
        assert!(!clip_contains_point(&clip, -20.0, 150.0));
    }

    #[test]
    fn zero_inactive_dimension_remains_visible_and_zero_active_is_empty() {
        for (axes, width, height, point, expected) in [
            ((true, false), 100.0, 0.0, (20.0, 500.0), true),
            ((false, true), 0.0, 100.0, (500.0, 20.0), true),
            ((true, false), 0.0, 100.0, (0.0, 20.0), false),
            ((false, true), 100.0, 0.0, (20.0, 0.0), false),
        ] {
            assert_eq!(
                clip_contains_point(&clip(axes, width, height), point.0, point.1),
                expected
            );
        }
    }

    #[test]
    fn non_finite_geometry_and_input_do_not_admit_hits() {
        let mut clip = clip((true, false), 100.0, 100.0);
        assert!(!clip_contains_point(&clip, f32::NAN, 1.0));
        clip.transformed_corners[0].0 = f32::INFINITY;
        assert!(!clip_contains_point(&clip, 1.0, 1.0));
    }

    #[test]
    fn axis_metadata_preserves_the_previous_clip_storage_bound() {
        const PREVIOUS_CLIP_BYTES: usize = 96;
        assert!(std::mem::size_of::<OverflowClip>() <= PREVIOUS_CLIP_BYTES);
    }
}
