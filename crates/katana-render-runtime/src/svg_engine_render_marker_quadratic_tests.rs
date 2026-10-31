use super::{
    MARKER_DEFINITION, assert_marker_basis, marker_path_transforms, unit_vector_bisector, usvg,
};
use rstest::rstest;
use tiny_skia::Point;

const VIEWPORT: &str = "200";
const MARKER_ANCHOR: (f32, f32) = (100.0, 100.0);
const MARKER_COUNT: usize = 3;
const MID_MARKER_INDEX: usize = 1;
const QUADRATIC_TO_QUADRATIC: &str = "M20 100 Q60 20 100 100 Q130 50 160 100";
const QUADRATIC_TO_LINE: &str = "M20 100 Q60 20 100 100 L160 100";
const LINE_TO_QUADRATIC: &str = "M20 100 L100 100 Q140 20 180 100";
const QUADRATIC_TO_QUADRATIC_TANGENTS: ((f32, f32), (f32, f32)) = ((40.0, 80.0), (30.0, -50.0));
const QUADRATIC_TO_LINE_TANGENTS: ((f32, f32), (f32, f32)) = ((40.0, 80.0), (60.0, 0.0));
const LINE_TO_QUADRATIC_TANGENTS: ((f32, f32), (f32, f32)) = ((80.0, 0.0), (40.0, -80.0));

#[rstest]
#[case::quadratic_to_quadratic(QUADRATIC_TO_QUADRATIC, QUADRATIC_TO_QUADRATIC_TANGENTS)]
#[case::quadratic_to_line(QUADRATIC_TO_LINE, QUADRATIC_TO_LINE_TANGENTS)]
#[case::line_to_quadratic(LINE_TO_QUADRATIC, LINE_TO_QUADRATIC_TANGENTS)]
fn quadratic_marker_tangent_uses_bezier_derivatives(
    #[case] path_data: &str,
    #[case] tangents: ((f32, f32), (f32, f32)),
) -> Result<(), String> {
    let tree = parse_quadratic_path(path_data)?;
    let transforms = marker_path_transforms(&tree);
    assert_eq!(transforms.len(), MARKER_COUNT);
    assert_marker_basis(
        transforms[MID_MARKER_INDEX],
        MARKER_ANCHOR,
        tangent_bisector(tangents.0, tangents.1),
    );
    Ok(())
}

fn parse_quadratic_path(path_data: &str) -> Result<usvg::Tree, String> {
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{VIEWPORT}' height='{VIEWPORT}'>{MARKER_DEFINITION}<path id='source' d='{path_data}' fill='none' stroke='black' marker-start='url(#m)' marker-mid='url(#m)' marker-end='url(#m)'/></svg>"
    );
    let options = usvg::Options {
        fontdb: super::upstream_writer_font_database()?,
        ..usvg::Options::default()
    };
    usvg::Tree::from_str(&svg, &options).map_err(|error| error.to_string())
}

fn tangent_bisector(incoming: (f32, f32), outgoing: (f32, f32)) -> (f32, f32) {
    let point = |vector: (f32, f32)| Point::from_xy(vector.0, vector.1);
    unit_vector_bisector(point(incoming), point(outgoing))
}
