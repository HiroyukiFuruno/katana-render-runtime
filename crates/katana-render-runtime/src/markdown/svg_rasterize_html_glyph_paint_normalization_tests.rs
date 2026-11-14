use super::super::{is_identity_linear, normalize_font_linear, scale_translation};
use tiny_skia::Transform;

const DECIMAL_UNITS_PER_EM: f32 = 1_000.0;
const BINARY_UNITS_PER_EM: f32 = 2_048.0;
const OBSERVED_FONT_SIZE: f32 = 14.0;
const OTHER_FONT_SIZES: [f32; 3] = [10.5, 12.0, 16.0];
const SOURCE_X: f32 = 3.0;
const SOURCE_Y: f32 = 10.0;
const BASELINE_SHIFT: f32 = 2.5;
const DPR: f32 = 2.0;
const EXPECTED_DEVICE_X: f32 = 6.0;
const EXPECTED_DEVICE_Y: f32 = 25.0;
const SKEW_X: f32 = -0.5;
const SKEW_Y: f32 = 0.25;
const NONUNIFORM_SCALE_X: f32 = 2.0;
const NONUNIFORM_SCALE_Y: f32 = 0.5;
const NEAR_IDENTITY_SCALE: f32 = 1.0001;

#[test]
fn canonical_decimal_scale_restores_exact_identity_and_batching() {
    let source = source_transform(
        Transform::identity(),
        OBSERVED_FONT_SIZE,
        DECIMAL_UNITS_PER_EM,
    );
    let previous = source.pre_scale(
        DECIMAL_UNITS_PER_EM / OBSERVED_FONT_SIZE,
        DECIMAL_UNITS_PER_EM / OBSERVED_FONT_SIZE,
    );
    assert!(!is_identity_linear(previous));
    assert!(previous.sx > 1.0 && previous.sy > 1.0);
    let normalized = normalize_font_linear(source, OBSERVED_FONT_SIZE, DECIMAL_UNITS_PER_EM);
    assert_eq!(normalized, Transform::identity());
    assert!(is_identity_linear(normalized));
}

#[test]
fn canonical_binary_and_other_decimal_sizes_remain_identity() {
    for upem in [DECIMAL_UNITS_PER_EM, BINARY_UNITS_PER_EM] {
        for size in OTHER_FONT_SIZES.into_iter().chain([OBSERVED_FONT_SIZE]) {
            let source = source_transform(Transform::identity(), size, upem);
            let normalized = normalize_font_linear(source, size, upem);
            assert_eq!(normalized, Transform::identity());
            assert!(is_identity_linear(normalized));
        }
    }
}

#[test]
fn genuine_rotation_skew_and_nonuniform_scale_remain_transformed() {
    for expected in [
        Transform::from_row(0.0, 1.0, -1.0, 0.0, 0.0, 0.0),
        Transform::from_row(1.0, SKEW_Y, SKEW_X, 1.0, 0.0, 0.0),
        Transform::from_row(NONUNIFORM_SCALE_X, 0.0, 0.0, NONUNIFORM_SCALE_Y, 0.0, 0.0),
    ] {
        let source = source_transform(expected, OBSERVED_FONT_SIZE, DECIMAL_UNITS_PER_EM);
        let normalized = normalize_font_linear(source, OBSERVED_FONT_SIZE, DECIMAL_UNITS_PER_EM);
        assert_eq!(normalized, expected);
        assert!(!is_identity_linear(normalized));
    }
}

#[test]
fn genuine_near_identity_scale_is_not_snapped() {
    let expected = Transform::from_scale(NEAR_IDENTITY_SCALE, NEAR_IDENTITY_SCALE);
    let source = source_transform(expected, OBSERVED_FONT_SIZE, DECIMAL_UNITS_PER_EM);
    let normalized = normalize_font_linear(source, OBSERVED_FONT_SIZE, DECIMAL_UNITS_PER_EM);
    assert_eq!(normalized, expected);
    assert!(!is_identity_linear(normalized));
}

#[test]
fn one_ulp_above_canonical_source_scale_remains_transformed() {
    let canonical = OBSERVED_FONT_SIZE / DECIMAL_UNITS_PER_EM;
    let changed = canonical.next_up();
    let source = Transform::from_scale(changed, canonical);
    let normalized = normalize_font_linear(source, OBSERVED_FONT_SIZE, DECIMAL_UNITS_PER_EM);
    assert!(normalized.sx > 1.0);
    assert_eq!(normalized.sy, 1.0);
    assert!(!is_identity_linear(normalized));
}

#[test]
fn normalization_preserves_translation_before_baseline_and_dpr() {
    let expected = Transform::from_translate(SOURCE_X, SOURCE_Y);
    let source = source_transform(expected, OBSERVED_FONT_SIZE, DECIMAL_UNITS_PER_EM);
    let normalized = normalize_font_linear(source, OBSERVED_FONT_SIZE, DECIMAL_UNITS_PER_EM);
    assert_eq!(normalized, expected);
    let shifted = normalized.post_translate(0.0, BASELINE_SHIFT);
    let device = scale_translation(shifted, DPR);
    assert!(is_identity_linear(device));
    assert_eq!(device.tx, EXPECTED_DEVICE_X);
    assert_eq!(device.ty, EXPECTED_DEVICE_Y);
}

fn source_transform(transform: Transform, font_size: f32, upem: f32) -> Transform {
    let scale = font_size / upem;
    transform.pre_scale(scale, scale)
}
