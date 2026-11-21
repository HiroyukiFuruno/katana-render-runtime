use tiny_skia::Point;

const COORDINATE_EPSILON: f32 = 0.001;

pub(super) fn assert_marker_basis(
    actual_anchor: Point,
    actual_basis: Point,
    expected_anchor: (f32, f32),
    expected_basis: (f32, f32),
) {
    assert_point_near(
        actual_anchor,
        Point::from_xy(expected_anchor.0, expected_anchor.1),
        "anchor",
        expected_anchor,
    );
    assert_point_near(
        actual_basis,
        Point::from_xy(expected_basis.0, expected_basis.1),
        "basis",
        expected_anchor,
    );
}

pub(super) fn assert_anchor_points(mut actual: Vec<Point>, expected: &[(f32, f32)]) {
    assert_eq!(actual.len(), expected.len());
    actual.sort_by(|left, right| left.x.total_cmp(&right.x).then(left.y.total_cmp(&right.y)));
    for (actual, expected) in actual.iter().zip(expected) {
        assert_point_near(
            *actual,
            Point::from_xy(expected.0, expected.1),
            "anchor",
            *expected,
        );
    }
}

pub(super) fn assert_point_near(actual: Point, expected: Point, stage: &str, anchor: (f32, f32)) {
    assert!(
        (actual.x - expected.x).abs() <= COORDINATE_EPSILON
            && (actual.y - expected.y).abs() <= COORDINATE_EPSILON,
        "marker {stage} mismatch at anchor {anchor:?}: actual=({}, {}), expected=({}, {})",
        actual.x,
        actual.y,
        expected.x,
        expected.y
    );
}
