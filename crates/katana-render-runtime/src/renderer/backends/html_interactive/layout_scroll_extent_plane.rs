use super::{Point, non_finite_geometry};

pub(super) fn clip_plane(
    input: &[Point],
    output: &mut Vec<Point>,
    origin: Point,
    axis: Point,
    boundary: f64,
    keep_greater: bool,
) -> Result<(), String> {
    output.clear();
    let Some(&previous_point) = input.last() else {
        return Ok(());
    };
    let signed_distance =
        |point: Point| (point.0 - origin.0) * axis.0 + (point.1 - origin.1) * axis.1 - boundary;
    clip_plane_edges(input, output, previous_point, signed_distance, keep_greater)
}

fn clip_plane_edges(
    input: &[Point],
    output: &mut Vec<Point>,
    previous_point: Point,
    signed_distance: impl Fn(Point) -> f64,
    keep_greater: bool,
) -> Result<(), String> {
    let mut previous = previous_point;
    let mut previous_distance = plane_distance(signed_distance(previous))?;
    for &current in input {
        let current_distance = plane_distance(signed_distance(current))?;
        clip_edge(
            output,
            previous,
            current,
            previous_distance,
            current_distance,
            keep_greater,
        )?;
        previous = current;
        previous_distance = current_distance;
    }
    Ok(())
}

fn plane_distance(distance: f64) -> Result<f64, String> {
    if distance.is_finite() {
        Ok(distance)
    } else {
        Err(non_finite_geometry())
    }
}

fn clip_edge(
    output: &mut Vec<Point>,
    previous: Point,
    current: Point,
    previous_distance: f64,
    current_distance: f64,
    keep_greater: bool,
) -> Result<(), String> {
    let previous_inside = inside_plane(previous_distance, keep_greater);
    let current_inside = inside_plane(current_distance, keep_greater);
    if previous_inside != current_inside {
        let denominator = previous_distance - current_distance;
        if denominator == 0.0 {
            return Err(non_finite_geometry());
        }
        let amount = previous_distance / denominator;
        let intersection = (
            previous.0 + (current.0 - previous.0) * amount,
            previous.1 + (current.1 - previous.1) * amount,
        );
        if !intersection.0.is_finite() || !intersection.1.is_finite() {
            return Err(non_finite_geometry());
        }
        output.push(intersection);
    }
    if current_inside {
        output.push(current);
    }
    Ok(())
}

fn inside_plane(distance: f64, keep_greater: bool) -> bool {
    if keep_greater {
        distance >= 0.0
    } else {
        distance <= 0.0
    }
}
