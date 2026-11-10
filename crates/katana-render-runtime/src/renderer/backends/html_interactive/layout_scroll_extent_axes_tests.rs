use super::axes::{polygon_axes, projection_bounds};
use super::{Point, RECT_CORNER_COUNT};

const NON_FINITE_GEOMETRY_ERROR: &str = "scrollable overflow contains non-finite geometry";
const FALLBACK_RECTANGLE: [Point; RECT_CORNER_COUNT] =
    [(10.0, 20.0), (20.0, 20.0), (20.0, 20.0), (10.0, 20.0)];
const COLLAPSED_RECTANGLE: [Point; RECT_CORNER_COUNT] = [(4.0, 7.0); RECT_CORNER_COUNT];
const PROJECTION_RECTANGLE: [Point; RECT_CORNER_COUNT] =
    [(1.0, 2.0), (4.0, 2.0), (4.0, 6.0), (1.0, 6.0)];

#[test]
fn owner_axes_use_finite_edge_fallback_and_reject_invalid_corners() {
    let fallback =
        polygon_axes(&FALLBACK_RECTANGLE, (true, false)).map(|axes| (axes.local_x, axes.local_y));
    assert_eq!(fallback, Ok((Some((1.0, 0.0)), None)));

    let collapsed =
        polygon_axes(&COLLAPSED_RECTANGLE, (true, false)).map(|axes| (axes.local_x, axes.local_y));
    assert_eq!(collapsed, Ok((None, None)));

    let mut invalid = FALLBACK_RECTANGLE;
    invalid[2].0 = f64::NAN;
    assert_eq!(
        polygon_axes(&invalid, (true, false)).err().as_deref(),
        Some(NON_FINITE_GEOMETRY_ERROR)
    );
}

#[test]
fn projection_bounds_preserve_valid_range_and_reject_finite_overflow() {
    assert_eq!(
        projection_bounds(&PROJECTION_RECTANGLE, (1.0, 2.0), (0.0, 1.0)),
        Ok((0.0, 4.0))
    );

    const EXTREME: f64 = f64::MAX;
    let overflow = [
        (-EXTREME, 0.0),
        (EXTREME, 0.0),
        (EXTREME, 1.0),
        (-EXTREME, 1.0),
    ];
    assert_eq!(
        projection_bounds(&overflow, (EXTREME, 0.0), (1.0, 0.0))
            .err()
            .as_deref(),
        Some(NON_FINITE_GEOMETRY_ERROR)
    );
}
