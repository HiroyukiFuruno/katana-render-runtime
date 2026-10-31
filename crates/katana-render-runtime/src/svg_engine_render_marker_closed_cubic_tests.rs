use super::{
    MARKER_DEFINITION, assert_marker_basis, marker_path_transforms, point_from_xy,
    unit_vector_bisector, upstream_writer_font_database, usvg,
};

const VIEWBOX_SIZE: f32 = 100.0;
const PATH_START: (f32, f32) = (20.0, 40.0);
const CUBIC_CONTROL_1: (f32, f32) = (40.0, 40.0);
const CUBIC_CONTROL_2: (f32, f32) = (60.0, 80.0);
const CUBIC_END: (f32, f32) = (80.0, 80.0);
const EXPECTED_MARKER_COUNT: usize = 2;
const MID_MARKER_INDEX: usize = 0;
const END_MARKER_INDEX: usize = 1;

#[test]
fn closed_cubic_mid_and_end_markers_use_curve_and_closing_edge_tangents() -> Result<(), String> {
    let tree = parse_closed_cubic_svg()?;
    let transforms = marker_path_transforms(&tree);
    let direction = closed_vertex_bisector();
    assert_eq!(transforms.len(), EXPECTED_MARKER_COUNT);
    assert_marker_basis(transforms[MID_MARKER_INDEX], CUBIC_END, direction);
    assert_marker_basis(transforms[END_MARKER_INDEX], PATH_START, direction);
    Ok(())
}

fn parse_closed_cubic_svg() -> Result<usvg::Tree, String> {
    let source = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{VIEWBOX_SIZE}' height='{VIEWBOX_SIZE}' viewBox='0 0 {VIEWBOX_SIZE} {VIEWBOX_SIZE}'>{MARKER_DEFINITION}<path id='source' d='M{} {} C{} {} {} {} {} {} Z' fill='none' stroke='black' marker-mid='url(#m)' marker-end='url(#m)'/></svg>",
        PATH_START.0,
        PATH_START.1,
        CUBIC_CONTROL_1.0,
        CUBIC_CONTROL_1.1,
        CUBIC_CONTROL_2.0,
        CUBIC_CONTROL_2.1,
        CUBIC_END.0,
        CUBIC_END.1,
    );
    let options = usvg::Options {
        fontdb: upstream_writer_font_database()?,
        ..usvg::Options::default()
    };
    usvg::Tree::from_str(&source, &options).map_err(|error| error.to_string())
}

fn closed_vertex_bisector() -> (f32, f32) {
    let incoming = point_from_xy((
        CUBIC_END.0 - CUBIC_CONTROL_2.0,
        CUBIC_END.1 - CUBIC_CONTROL_2.1,
    ));
    let closing = point_from_xy((PATH_START.0 - CUBIC_END.0, PATH_START.1 - CUBIC_END.1));
    unit_vector_bisector(incoming, closing)
}
