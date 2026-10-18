use super::{
    BOTTOM_LEFT_CORNER, ElementBox, OwnerAxes, Point, RECT_CORNER_COUNT, non_finite_geometry,
};

pub(super) fn owner_axes(
    owner: &ElementBox,
    active_axes: (bool, bool),
) -> Result<Option<OwnerAxes>, String> {
    let corners: [Point; RECT_CORNER_COUNT] = owner
        .transformed_corners
        .map(|(x, y)| (f64::from(x), f64::from(y)));
    polygon_axes(&corners, active_axes)
}

pub(super) fn polygon_axes(
    corners: &[Point; RECT_CORNER_COUNT],
    active_axes: (bool, bool),
) -> Result<Option<OwnerAxes>, String> {
    if !corners.iter().all(|(x, y)| x.is_finite() && y.is_finite()) {
        return Err(non_finite_geometry());
    }
    let local_x_edge = (corners[1].0 - corners[0].0, corners[1].1 - corners[0].1);
    let local_y_edge = (
        corners[BOTTOM_LEFT_CORNER].0 - corners[0].0,
        corners[BOTTOM_LEFT_CORNER].1 - corners[0].1,
    );
    let local_x = active_axis_normal(active_axes.0, local_y_edge, local_x_edge)?;
    let local_y = active_axis_normal(active_axes.1, local_x_edge, local_y_edge)?;
    Ok(Some(OwnerAxes {
        origin: corners[0],
        local_x,
        local_y,
    }))
}

fn active_axis_normal(
    active: bool,
    perpendicular_edge: Point,
    edge: Point,
) -> Result<Option<Point>, String> {
    if !active {
        return Ok(None);
    }
    let Some(normal) = perpendicular_unit(perpendicular_edge).or_else(|| unit_vector(edge)) else {
        return Ok(None);
    };
    Ok(Some(orient_normal(normal, edge)))
}

fn perpendicular_unit(vector: Point) -> Option<Point> {
    let length = vector.0.hypot(vector.1);
    if !length.is_finite() || length == 0.0 {
        return None;
    }
    Some((vector.1 / length, -vector.0 / length))
}

fn unit_vector(vector: Point) -> Option<Point> {
    let length = vector.0.hypot(vector.1);
    if !length.is_finite() || length == 0.0 {
        return None;
    }
    Some((vector.0 / length, vector.1 / length))
}

fn orient_normal(mut normal: Point, edge: Point) -> Point {
    if normal.0 * edge.0 + normal.1 * edge.1 < 0.0 {
        normal = (-normal.0, -normal.1);
    }
    normal
}

pub(super) fn projection_bounds(
    corners: &[Point; RECT_CORNER_COUNT],
    origin: Point,
    axis: Point,
) -> Result<(f64, f64), String> {
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for &(x, y) in corners {
        let projection = (x - origin.0) * axis.0 + (y - origin.1) * axis.1;
        if !projection.is_finite() {
            return Err(non_finite_geometry());
        }
        minimum = minimum.min(projection);
        maximum = maximum.max(projection);
    }
    Ok((minimum, maximum))
}
