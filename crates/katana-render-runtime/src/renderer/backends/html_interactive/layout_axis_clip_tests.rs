use super::{axis_x, axis_y, polygon_path};

#[test]
fn finalize_missing_paint_clip_owner_returns_specific_error() {
    use super::super::layout::{DeferredAxisClip, HtmlLayoutRenderer};
    use super::super::types::OverflowClip;
    use super::finalize;

    let mut renderer = HtmlLayoutRenderer::default();
    renderer.deferred_axis_clips.push(DeferredAxisClip {
        id: 17,
        owner_node_id: 91,
        source_owner: (0.0, 0.0, 10.0, 10.0),
        clip: OverflowClip::new(91, 0.0, 0.0, 4.0, 4.0, 0.0, 0.0),
    });

    assert_eq!(
        finalize(&mut renderer).err().as_deref(),
        Some("missing paint clip owner 91")
    );
}

#[test]
fn finalize_with_empty_pending_clips_preserves_svg() {
    use super::super::layout::HtmlLayoutRenderer;
    use super::finalize;

    let mut renderer = HtmlLayoutRenderer {
        svg: "<svg><path d=\"M 1 2\"/></svg>".to_string(),
        ..HtmlLayoutRenderer::default()
    };

    assert!(finalize(&mut renderer).is_ok());

    assert_eq!(renderer.svg, "<svg><path d=\"M 1 2\"/></svg>");
    assert!(renderer.deferred_axis_clips.is_empty());
}

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

#[test]
fn zero_source_dimensions_reject_both_undefined_owner_axes() {
    const COLLAPSED: [(f32, f32); 4] = [(0.0, 0.0); 4];
    for result in [
        axis_x(0.0, 0.0, (0.0, 0.0), &COLLAPSED),
        axis_y(0.0, 0.0, (0.0, 0.0), &COLLAPSED),
    ] {
        assert_eq!(
            result,
            Err("zero-size owner cannot define paint clip transform".to_owned())
        );
    }
}

#[test]
fn disjoint_paint_clip_and_non_finite_inverse_have_distinct_results() {
    use super::super::layout::HtmlLayoutRenderer;
    use super::super::types::OverflowClip;
    use super::{local_polygon, transformed_clip_path};
    use tiny_skia::Transform;

    const OUTSIDE_VIEWPORT: f32 = 10_000.0;
    let renderer = HtmlLayoutRenderer::default();
    let clip = OverflowClip::new(1, OUTSIDE_VIEWPORT, OUTSIDE_VIEWPORT, 1.0, 1.0, 0.0, 0.0);
    assert_eq!(
        transformed_clip_path(&renderer, &clip, Transform::identity()),
        Ok("M 0 0 Z".to_owned())
    );
    assert_eq!(
        local_polygon(vec![(f64::MAX, 0.0)], Transform::identity(), &renderer),
        Err("non-finite inverse owner transform point".to_owned())
    );
}

#[test]
fn finite_owner_geometry_that_overflows_transform_is_rejected() {
    let corners = [(0.0, 0.0), (f32::MAX, 0.0), (f32::MAX, 1.0), (0.0, 1.0)];
    assert_eq!(
        super::owner_transform((0.0, 0.0, 0.5, 1.0), &corners)
            .err()
            .as_deref(),
        Some("non-finite owner transform for paint clip")
    );
}

#[test]
fn noninvertible_owner_transforms_hide_paint_clip() {
    use super::super::layout::HtmlLayoutRenderer;
    use super::super::types::OverflowClip;
    use crate::HtmlBrowserViewport;
    use std::collections::HashMap;
    use tiny_skia::Transform;

    let viewport = HtmlBrowserViewport {
        width: 100,
        height: 100,
        device_scale_factor: 1.0,
    };
    let renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
    let clip = OverflowClip::new(1, 0.0, 0.0, 20.0, 20.0, 0.0, 0.0);
    for transform in [
        Transform::from_scale(0.0, 1.0),
        Transform::from_row(1.0, 1.0, 1.0, 1.0, 0.0, 0.0),
    ] {
        assert_eq!(
            super::transformed_clip_path(&renderer, &clip, transform),
            Ok("M 0 0 Z".to_string())
        );
    }
}
