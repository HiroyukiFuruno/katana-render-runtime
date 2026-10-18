use super::{axis_x, axis_y, polygon_path};

#[test]
fn zero_area_polygon_is_empty_clip_path() {
    assert_eq!(polygon_path(&[]), "M 0 0 Z");
}

#[test]
fn zero_source_axis_recovers_rotated_basis() {
    let origin = (10.0, 20.0);
    let corners = [(10.0, 20.0), (10.0, 20.0), (0.0, 20.0), (0.0, 20.0)];
    assert_eq!(axis_x(0.0, 10.0, origin, &corners).ok(), Some((0.0, 1.0)));
    let corners = [(10.0, 20.0), (10.0, 30.0), (10.0, 30.0), (10.0, 20.0)];
    assert_eq!(axis_y(10.0, 0.0, origin, &corners).ok(), Some((-1.0, 0.0)));
}
