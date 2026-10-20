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

#[test]
fn non_finite_clip_geometry_returns_specific_errors() {
    use super::super::types::OverflowClip;
    use super::{owner_transform, transform_clip};
    use tiny_skia::Transform;

    const CLIP_SCALE: f32 = f32::MAX;
    const CLIP_COORDINATE: f32 = 2.0;
    let clip = OverflowClip::new(1, 0.0, 0.0, CLIP_COORDINATE, 1.0, 0.0, 0.0);
    let corners = clip.transformed_corners;
    assert_eq!(
        owner_transform((0.0, 0.0, f32::NAN, 1.0), &corners)
            .err()
            .as_deref(),
        Some("non-finite owner source geometry for paint clip")
    );
    assert_eq!(
        transform_clip(clip, Transform::from_scale(CLIP_SCALE, 1.0))
            .err()
            .as_deref(),
        Some("non-finite transformed paint clip point")
    );
}

#[test]
fn degenerate_zero_source_axes_return_specific_errors() {
    const ORIGIN: (f32, f32) = (4.0, 7.0);
    const COLLAPSED_CORNERS: [(f32, f32); 4] = [ORIGIN; 4];
    assert_eq!(
        axis_x(0.0, 3.0, ORIGIN, &COLLAPSED_CORNERS),
        Err("degenerate owner y axis for paint clip transform".to_string())
    );
    assert_eq!(
        axis_y(3.0, 0.0, ORIGIN, &COLLAPSED_CORNERS),
        Err("degenerate owner x axis for paint clip transform".to_string())
    );
}
