use super::{fixture, upstream_writer_font_database, usvg};
use tiny_skia::{Point, Transform};

const MARKER_REFERENCE_X: f32 = 10.0;
const MARKER_REFERENCE_Y: f32 = 10.0;
const MARKER_X_BASIS_STEP: f32 = 1.0;
const MARKER_FIXTURE_VIEWBOX_WIDTH: f32 = 200.0;
const MARKER_FIXTURE_VIEWBOX_HEIGHT: f32 = 200.0;

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
