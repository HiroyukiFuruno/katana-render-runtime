use super::{fixture, upstream_writer_font_database, usvg};
use tiny_skia::{Point, Transform};

const MARKER_REFERENCE_X: f32 = 10.0;
const MARKER_REFERENCE_Y: f32 = 10.0;
const MARKER_X_BASIS_STEP: f32 = 1.0;
const MARKER_FIXTURE_VIEWBOX_WIDTH: f32 = 200.0;
const MARKER_FIXTURE_VIEWBOX_HEIGHT: f32 = 200.0;
const CURVE_LINE_MARKER_COUNT: usize = 3;
const CURVE_LINE_MID_ANCHOR: (f32, f32) = (100.0, 60.0);
const CUBIC_TO_LINE_INCOMING: (f32, f32) = (0.0, -50.0);
const CUBIC_TO_LINE_OUTGOING: (f32, f32) = (70.0, 80.0);
const LINE_TO_CUBIC_INCOMING: (f32, f32) = (70.0, -80.0);
const LINE_TO_CUBIC_OUTGOING: (f32, f32) = (0.0, 50.0);
const SUBPATH_MARKER_COUNT: usize = 4;
const CURVE_START_MARKER_INDEX: usize = 0;
const CURVE_MID_MARKER_INDEX: usize = 1;
const CURVE_END_MARKER_INDEX: usize = 2;
const FIRST_SUBPATH_MID_INDEX: usize = 1;
const SECOND_SUBPATH_MID_INDEX: usize = 2;
const FIRST_SUBPATH_MID_ANCHOR: (f32, f32) = (80.0, 100.0);
const SECOND_SUBPATH_MID_ANCHOR: (f32, f32) = (120.0, 100.0);
const LINE_TO_MOVE_BASIS: (f32, f32) = (1.0, 0.0);
const DIAGONAL_UP_BASIS: (f32, f32) = (0.70710677, -0.70710677);
const DEGENERATE_CURVE_START: (f32, f32) = (40.0, 155.0);
const DEGENERATE_CURVE_START_VECTOR: (f32, f32) = (60.0, -115.0);
const DEGENERATE_CURVE_END: (f32, f32) = (160.0, 160.0);
const DEGENERATE_CURVE_END_VECTOR: (f32, f32) = (60.0, 120.0);
const DEGENERATE_CURVE_MID: (f32, f32) = (100.0, 40.0);
const DEGENERATE_INCOMING_VECTOR: (f32, f32) = (60.0, -115.0);
const DEGENERATE_INCOMING_NEXT_VECTOR: (f32, f32) = (60.0, 0.0);
const DEGENERATE_OUTGOING_PREVIOUS_VECTOR: (f32, f32) = (60.0, 0.0);
const DEGENERATE_OUTGOING_VECTOR: (f32, f32) = (60.0, 120.0);

#[test]
fn cubic_auto_markers_follow_independent_tangents() -> Result<(), String> {
    let tree = parse_fixture_at_viewbox_size("orient=auto-on-M-C-C-1")?;
    let transforms = marker_path_transforms(&tree);
    assert_eq!(transforms.len(), 3);
    assert_marker_basis(transforms[0], (40.0, 155.0), (0.0, -1.0));
    assert_marker_basis(transforms[1], (100.0, 40.0), (1.0, 0.0));
    assert_marker_basis(transforms[2], (160.0, 160.0), (0.0, 1.0));
    Ok(())
}

#[test]
fn cubic_to_line_marker_uses_the_incoming_and_outgoing_tangent_bisector() -> Result<(), String> {
    assert_curve_line_mid_tangent(
        "orient=auto-on-M-C-L",
        point_from_xy(CUBIC_TO_LINE_INCOMING),
        point_from_xy(CUBIC_TO_LINE_OUTGOING),
    )
}

#[test]
fn line_to_cubic_marker_uses_the_incoming_and_outgoing_tangent_bisector() -> Result<(), String> {
    assert_curve_line_mid_tangent(
        "orient=auto-on-M-L-C",
        point_from_xy(LINE_TO_CUBIC_INCOMING),
        point_from_xy(LINE_TO_CUBIC_OUTGOING),
    )
}

#[test]
fn line_to_move_and_move_to_cubic_markers_use_the_adjacent_subpath_tangents() -> Result<(), String>
{
    assert_subpath_mid_tangents(
        "orient=auto-on-M-L-M-C",
        LINE_TO_MOVE_BASIS,
        DIAGONAL_UP_BASIS,
    )
}

#[test]
fn cubic_to_move_and_move_to_line_markers_use_the_adjacent_subpath_tangents() -> Result<(), String>
{
    assert_subpath_mid_tangents(
        "orient=auto-on-M-C-M-L",
        DIAGONAL_UP_BASIS,
        LINE_TO_MOVE_BASIS,
    )
}

fn assert_subpath_mid_tangents(
    fixture_name: &str,
    first_mid_basis: (f32, f32),
    second_mid_basis: (f32, f32),
) -> Result<(), String> {
    let tree = parse_fixture_at_viewbox_size(fixture_name)?;
    let transforms = marker_path_transforms(&tree);
    assert_eq!(transforms.len(), SUBPATH_MARKER_COUNT, "{fixture_name}");
    assert_marker_basis(
        transforms[FIRST_SUBPATH_MID_INDEX],
        FIRST_SUBPATH_MID_ANCHOR,
        first_mid_basis,
    );
    assert_marker_basis(
        transforms[SECOND_SUBPATH_MID_INDEX],
        SECOND_SUBPATH_MID_ANCHOR,
        second_mid_basis,
    );
    Ok(())
}

#[test]
fn degenerate_cubic_start_and_incoming_midpoint_use_fallback_tangents() -> Result<(), String> {
    let tree = parse_fixture_at_viewbox_size("orient=auto-on-M-C-C-6")?;
    let transforms = marker_path_transforms(&tree);
    assert_eq!(transforms.len(), CURVE_LINE_MARKER_COUNT);
    assert_marker_basis(
        transforms[CURVE_START_MARKER_INDEX],
        DEGENERATE_CURVE_START,
        unit_direction(DEGENERATE_CURVE_START_VECTOR),
    );
    assert_degenerate_midpoint(
        transforms[CURVE_MID_MARKER_INDEX],
        DEGENERATE_CURVE_MID,
        DEGENERATE_INCOMING_VECTOR,
        DEGENERATE_INCOMING_NEXT_VECTOR,
    );
    Ok(())
}

#[test]
fn degenerate_cubic_end_and_outgoing_midpoint_use_fallback_tangents() -> Result<(), String> {
    let tree = parse_fixture_at_viewbox_size("orient=auto-on-M-C-C-7")?;
    let transforms = marker_path_transforms(&tree);
    assert_eq!(transforms.len(), CURVE_LINE_MARKER_COUNT);
    assert_marker_basis(
        transforms[CURVE_END_MARKER_INDEX],
        DEGENERATE_CURVE_END,
        unit_direction(DEGENERATE_CURVE_END_VECTOR),
    );
    assert_degenerate_midpoint(
        transforms[CURVE_MID_MARKER_INDEX],
        DEGENERATE_CURVE_MID,
        DEGENERATE_OUTGOING_PREVIOUS_VECTOR,
        DEGENERATE_OUTGOING_VECTOR,
    );
    Ok(())
}

fn assert_degenerate_midpoint(
    transform: Transform,
    anchor: (f32, f32),
    incoming: (f32, f32),
    outgoing: (f32, f32),
) {
    assert_marker_basis(
        transform,
        anchor,
        unit_vector_bisector(point_from_xy(incoming), point_from_xy(outgoing)),
    );
}

fn unit_direction(vector: (f32, f32)) -> (f32, f32) {
    let vector = unit_vector(point_from_xy(vector));
    (vector.x, vector.y)
}

fn assert_curve_line_mid_tangent(
    fixture_name: &str,
    incoming: Point,
    outgoing: Point,
) -> Result<(), String> {
    let tree = parse_fixture_at_viewbox_size(fixture_name)?;
    let transforms = marker_path_transforms(&tree);
    assert_eq!(transforms.len(), CURVE_LINE_MARKER_COUNT, "{fixture_name}");
    assert_marker_basis(
        transforms[1],
        CURVE_LINE_MID_ANCHOR,
        unit_vector_bisector(incoming, outgoing),
    );
    Ok(())
}

fn point_from_xy(coordinates: (f32, f32)) -> Point {
    Point::from_xy(coordinates.0, coordinates.1)
}

fn unit_vector_bisector(incoming: Point, outgoing: Point) -> (f32, f32) {
    let incoming = unit_vector(incoming);
    let outgoing = unit_vector(outgoing);
    let bisector = Point::from_xy(incoming.x + outgoing.x, incoming.y + outgoing.y);
    let bisector = unit_vector(bisector);
    (bisector.x, bisector.y)
}

fn unit_vector(vector: Point) -> Point {
    let length = vector.x.hypot(vector.y);
    Point::from_xy(vector.x / length, vector.y / length)
}

#[test]
fn subpath_markers_keep_each_authored_start() -> Result<(), String> {
    let tree = parse_fixture_at_viewbox_size("target-with-subpaths-1")?;
    assert_anchor_points(
        marker_path_transforms(&tree),
        &[
            (20.0, 85.0),
            (35.0, 135.0),
            (60.0, 55.0),
            (85.0, 135.0),
            (100.0, 85.0),
            (100.0, 85.0),
            (115.0, 135.0),
            (140.0, 55.0),
            (165.0, 135.0),
            (180.0, 85.0),
        ],
    );
    Ok(())
}

#[test]
fn closed_marker_end_returns_to_its_subpath_start() -> Result<(), String> {
    let tree = parse_fixture_at_viewbox_size("orient=auto-on-M-L-Z")?;
    assert_anchor_points(
        marker_path_transforms(&tree),
        &[(30.0, 100.0), (30.0, 100.0), (170.0, 100.0)],
    );
    Ok(())
}

fn assert_anchor_points(transforms: Vec<Transform>, expected: &[(f32, f32)]) {
    let mut anchors: Vec<_> = transforms
        .into_iter()
        .map(|transform| map_point(transform, MARKER_REFERENCE_X, MARKER_REFERENCE_Y))
        .collect();
    assert_eq!(anchors.len(), expected.len());
    anchors.sort_by(|left, right| left.x.total_cmp(&right.x).then(left.y.total_cmp(&right.y)));
    for (actual, expected) in anchors.iter().zip(expected) {
        assert_point_near(*actual, Point::from_xy(expected.0, expected.1));
    }
}

fn marker_path_transforms(tree: &usvg::Tree) -> Vec<Transform> {
    fn collect(group: &usvg::Group, transforms: &mut Vec<Transform>) {
        for node in group.children() {
            match node {
                usvg::Node::Group(child) => collect(child, transforms),
                usvg::Node::Path(path) if path.id().is_empty() => {
                    transforms.push(path.abs_transform());
                }
                _ => {}
            }
        }
    }

    let mut transforms = Vec::new();
    collect(tree.root(), &mut transforms);
    transforms
}

fn assert_marker_basis(transform: Transform, anchor: (f32, f32), direction: (f32, f32)) {
    let mapped_anchor = map_point(transform, MARKER_REFERENCE_X, MARKER_REFERENCE_Y);
    let mapped_x = map_point(
        transform,
        MARKER_REFERENCE_X + MARKER_X_BASIS_STEP,
        MARKER_REFERENCE_Y,
    );
    assert_point_near(mapped_anchor, Point::from_xy(anchor.0, anchor.1));
    assert_point_near(
        Point::from_xy(mapped_x.x - mapped_anchor.x, mapped_x.y - mapped_anchor.y),
        Point::from_xy(direction.0, direction.1),
    );
}

fn map_point(transform: Transform, x: f32, y: f32) -> Point {
    let mut point = Point::from_xy(x, y);
    transform.map_point(&mut point);
    point
}

fn assert_point_near(actual: Point, expected: Point) {
    const COORDINATE_EPSILON: f32 = 0.001;
    assert!((actual.x - expected.x).abs() <= COORDINATE_EPSILON);
    assert!((actual.y - expected.y).abs() <= COORDINATE_EPSILON);
}

fn parse_fixture_at_viewbox_size(name: &str) -> Result<usvg::Tree, String> {
    let source = fixture(name)?;
    let default_size =
        usvg::Size::from_wh(MARKER_FIXTURE_VIEWBOX_WIDTH, MARKER_FIXTURE_VIEWBOX_HEIGHT)
            .ok_or("invalid test viewport")?;
    let options = usvg::Options {
        fontdb: upstream_writer_font_database()?,
        default_size,
        ..usvg::Options::default()
    };
    usvg::Tree::from_str(&source, &options).map_err(|error| error.to_string())
}
