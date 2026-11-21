use super::{OverflowClip, clip_contains_point};

const CORNER_RADIUS: f32 = 10.0;

fn clip(axes: (bool, bool), width: f32, height: f32) -> OverflowClip {
    OverflowClip::new(1, 0.0, 0.0, width, height, CORNER_RADIUS, CORNER_RADIUS)
        .with_active_axes(axes)
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
    assert!(!clip_contains_point(&clip, f32::MAX, -f32::MAX));
    assert!(!clip_contains_point(&clip, f32::NAN, 1.0));
    clip.transformed_corners[0].0 = f32::INFINITY;
    assert!(!clip_contains_point(&clip, 1.0, 1.0));
}

#[test]
fn invalid_corner_radii_never_admit_hits() {
    let original = clip((true, true), 100.0, 100.0);
    assert!(clip_contains_point(&original, 50.0, 50.0));
    for radius in [(-1.0, 2.0), (2.0, -1.0), (f32::NAN, 2.0), (2.0, f32::NAN)] {
        let mut invalid = original;
        invalid.corner_radii[0] = radius;
        assert!(!clip_contains_point(&invalid, 50.0, 50.0), "{radius:?}");
    }
}

#[test]
fn axis_metadata_preserves_the_previous_clip_storage_bound() {
    const PREVIOUS_CLIP_BYTES: usize = 96;
    assert!(std::mem::size_of::<OverflowClip>() <= PREVIOUS_CLIP_BYTES);
}
