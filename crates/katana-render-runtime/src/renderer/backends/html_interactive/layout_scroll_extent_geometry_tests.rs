use super::super::types::OverflowClip;
use super::{Point, RECT_CORNER_COUNT, axis_aligned, plane, polygon};

const NON_FINITE_GEOMETRY_ERROR: &str = "scrollable overflow contains non-finite geometry";

fn clip(width: f32, height: f32) -> OverflowClip {
    OverflowClip::new(1, 0.0, 0.0, width, height, 0.0, 0.0)
}

fn viewport(left: f64, top: f64, width: f64, height: f64) -> [Point; RECT_CORNER_COUNT] {
    [
        (left, top),
        (left + width, top),
        (left + width, top + height),
        (left, top + height),
    ]
}

#[test]
fn viewport_clipping_returns_empty_for_collapsed_and_disjoint_geometry() {
    let viewport = viewport(20.0, 30.0, 5.0, 7.0);
    for width in [0.0, 10.0] {
        assert_eq!(
            polygon::clip_viewport_polygon(&clip(width, 10.0), &viewport)
                .map(|polygon| polygon.is_empty()),
            Ok(true)
        );
    }
}

#[test]
fn viewport_and_clip_non_finite_geometry_return_exact_error() {
    let valid_clip = clip(10.0, 8.0);
    let mut invalid_viewport = viewport(0.0, 0.0, 4.0, 4.0);
    invalid_viewport[2].0 = f64::INFINITY;
    assert_eq!(
        polygon::clip_viewport_polygon(&valid_clip, &invalid_viewport)
            .err()
            .as_deref(),
        Some(NON_FINITE_GEOMETRY_ERROR)
    );

    let mut invalid_clip = valid_clip;
    invalid_clip.transformed_corners[1].1 = f32::NAN;
    assert_eq!(
        polygon::clip_polygon(&invalid_clip).err().as_deref(),
        Some(NON_FINITE_GEOMETRY_ERROR)
    );
}

#[test]
fn plane_clipping_clears_empty_output_and_rejects_non_finite_distance() {
    let mut output = vec![(2.0, 3.0)];
    assert_eq!(
        plane::clip_plane(&[], &mut output, (0.0, 0.0), (1.0, 0.0), 0.0, true),
        Ok(())
    );
    assert!(output.is_empty());

    let input = [(1.0, 1.0)];
    assert_eq!(
        plane::clip_plane(
            &input,
            &mut output,
            (0.0, 0.0),
            (1.0, 0.0),
            f64::INFINITY,
            true
        )
        .err()
        .as_deref(),
        Some(NON_FINITE_GEOMETRY_ERROR)
    );
}

#[test]
fn axis_aligned_extent_accepts_finite_degenerate_and_rejects_empty_or_invalid_bounds() {
    const OVERFLOW_COORDINATE: f64 = f64::MAX;
    const DEGENERATE_SINGLETON: [(f64, f64); 1] = [(7.0, 11.0)];
    assert_eq!(
        axis_aligned::polygon_right_bottom(&DEGENERATE_SINGLETON),
        Ok(None)
    );
    let invalid_polygons: [(&str, &[Point]); 2] = [
        ("empty", &[]),
        ("overflow", &[(OVERFLOW_COORDINATE, 1.0), (1.0, 2.0)]),
    ];
    for (case, points) in invalid_polygons {
        assert_eq!(
            axis_aligned::polygon_right_bottom(points).err().as_deref(),
            Some(NON_FINITE_GEOMETRY_ERROR),
            "{case} polygon"
        );
    }

    let invalid_corners = [(0.0, 0.0), (f32::NAN, 0.0), (1.0, 1.0), (0.0, 1.0)];
    assert_eq!(
        axis_aligned::bounds(&invalid_corners).err().as_deref(),
        Some(NON_FINITE_GEOMETRY_ERROR)
    );
}

#[test]
fn polygon_right_bottom_returns_finite_extrema_for_unordered_points() {
    let cases = [
        (
            "positive rectangle",
            &[(1.25, 4.5), (8.75, 4.5), (8.75, 11.25), (1.25, 11.25)][..],
            Some((8.75, 11.25)),
        ),
        (
            "negative rectangle",
            &[(-12.5, -8.25), (-4.75, -1.5), (-12.5, -1.5), (-4.75, -8.25)][..],
            Some((-4.75, -1.5)),
        ),
    ];

    for (name, polygon, expected) in cases {
        assert_eq!(
            axis_aligned::polygon_right_bottom(polygon),
            Ok(expected),
            "{name} extrema"
        );
    }
}

#[test]
fn viewport_polygon_intersection_keeps_all_four_clip_boundaries() -> Result<(), String> {
    let polygon =
        polygon::clip_viewport_polygon(&clip(10.0, 8.0), &viewport(-1.0, -1.0, 12.0, 10.0))?;
    assert_eq!(polygon.len(), RECT_CORNER_COUNT);
    for corner in [(0.0, 0.0), (10.0, 0.0), (10.0, 8.0), (0.0, 8.0)] {
        assert!(
            polygon.contains(&corner),
            "missing clipped corner {corner:?}"
        );
    }
    Ok(())
}

#[test]
fn collapsed_polygon_with_no_active_normal_cannot_expose_viewport_pixels() {
    let mut collapsed = clip(10.0, 8.0);
    collapsed.transformed_corners = [(0.0, 0.0); RECT_CORNER_COUNT];
    assert_eq!(
        polygon::clip_viewport_polygon(&collapsed, &viewport(0.0, 0.0, 10.0, 8.0)),
        Ok(Vec::new())
    );
}
