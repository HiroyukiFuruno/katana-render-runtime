use super::super::super::html_browser::HtmlBrowserViewport;
use super::super::layout::HtmlLayoutRenderer;
use super::super::types::{ElementPositioningContext, OverflowClip};
use super::{Point, PolygonScratch, RECT_CORNER_COUNT, axis_aligned, plane, polygon};
use std::collections::HashMap;

const NON_FINITE_GEOMETRY_ERROR: &str = "scrollable overflow contains non-finite geometry";
const OWNER_VIEWPORT_WIDTH: u32 = 10;
const OWNER_VIEWPORT_HEIGHT: u32 = 8;

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
fn plane_clipping_rejects_nonfinite_distances_and_overflowing_intersections() {
    let cases: &[&[Point]] = &[
        &[(0.0, f64::NAN), (1.0, 1.0)],
        &[(0.0, 1.0), (1.0, f64::INFINITY)],
        &[(f64::MAX, 1.0), (-f64::MAX, -1.0)],
    ];
    for input in cases {
        let mut output = Vec::new();
        assert_eq!(
            plane::clip_plane(input, &mut output, (0.0, 0.0), (0.0, 1.0), 0.0, true)
                .err()
                .as_deref(),
            Some(NON_FINITE_GEOMETRY_ERROR)
        );
    }
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
fn disjoint_axis_aligned_clip_has_no_visible_extent() -> Result<(), String> {
    let mut renderer = owner_renderer();
    renderer.element_boxes[0].transformed_corners =
        [(0.0, 0.0), (10.0, 0.0), (10.0, 8.0), (0.0, 8.0)];
    renderer.element_boxes[0].overflow_clip_axes = (true, true);
    let subject = [(20.0, 30.0), (25.0, 30.0), (25.0, 37.0), (20.0, 37.0)];
    let indices = HashMap::from([(1, 0)]);

    assert_eq!(
        super::visible_extent(
            &subject,
            &[clip(10.0, 8.0)],
            &renderer.element_boxes,
            &indices,
            &mut PolygonScratch::default(),
        ),
        Ok(None)
    );
    Ok(())
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
fn viewport_clipping_respects_each_active_axis() -> Result<(), String> {
    let input = viewport(-5.0, -7.0, 20.0, 20.0);
    let cases = [
        ((false, false), (-5.0, -7.0, 15.0, 13.0)),
        ((true, false), (0.0, -7.0, 10.0, 13.0)),
        ((false, true), (-5.0, 0.0, 15.0, 8.0)),
        ((true, true), (0.0, 0.0, 10.0, 8.0)),
    ];
    for (axes, (left, top, right, bottom)) in cases {
        let actual =
            polygon::clip_viewport_polygon(&clip(10.0, 8.0).with_active_axes(axes), &input)?;
        assert_eq!(actual.len(), RECT_CORNER_COUNT, "axes: {axes:?}");
        for corner in [(left, top), (right, top), (right, bottom), (left, bottom)] {
            assert!(
                actual.contains(&corner),
                "axes: {axes:?}, corner: {corner:?}"
            );
        }
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

fn owner_renderer() -> HtmlLayoutRenderer {
    let viewport = HtmlBrowserViewport {
        width: OWNER_VIEWPORT_WIDTH,
        height: OWNER_VIEWPORT_HEIGHT,
        device_scale_factor: 1.0,
    };
    let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
    renderer.start_element_box(1, ElementPositioningContext::InFlow, true, None);
    renderer
}

#[test]
fn owner_polygon_keeps_inactive_axes_and_rejects_a_collapsed_active_axis() {
    let points = viewport(1.0, 2.0, 3.0, 4.0).to_vec();
    let indices = HashMap::from([(1, 0)]);
    let mut renderer = owner_renderer();
    for (axes, expected) in [((false, false), true), ((true, false), false)] {
        renderer.element_boxes[0].overflow_clip_axes = axes;
        let mut scratch = PolygonScratch {
            first: points.clone(),
            second: Vec::new(),
        };
        let mut first = true;
        let result = polygon::clip_owner_polygon(
            &clip(10.0, 8.0),
            &renderer.element_boxes,
            &indices,
            &mut scratch,
            &mut first,
        );
        assert_eq!(result, Ok(expected), "axes: {axes:?}");
        assert!(first);
        assert_eq!(scratch.first, points);
        assert!(scratch.second.is_empty());
        if expected {
            assert_eq!(
                axis_aligned::polygon_right_bottom(scratch.current(first)),
                Ok(Some((4.0, 6.0)))
            );
        }
    }
}

#[path = "layout_scroll_extent_owner_index_tests.rs"]
mod owner_index_tests;
