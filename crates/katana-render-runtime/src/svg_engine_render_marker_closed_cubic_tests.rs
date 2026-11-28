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
fn closed_subpath_preserves_mid_markers_before_and_after_the_seam() -> Result<(), String> {
    let source = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{VIEWBOX_SIZE}' height='{VIEWBOX_SIZE}'>{MARKER_DEFINITION}<path id='source' d='M20 20 L60 20 L60 60 Z M20 80 L80 80' fill='none' stroke='black' marker-mid='url(#m)'/></svg>"
    );
    let options = usvg::Options {
        fontdb: upstream_writer_font_database()?,
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_str(&source, &options).map_err(|error| error.to_string())?;
    let transforms = marker_path_transforms(&tree);
    let [first, second, after_close] = transforms.as_slice() else {
        return Err(format!(
            "expected three mid markers, got {}",
            transforms.len()
        ));
    };
    assert_marker_basis(*first, (60.0, 20.0), (0.707_106_77, 0.707_106_77));
    assert_marker_basis(*second, (60.0, 60.0), (-0.923_879_5, 0.382_683_43));
    assert_marker_basis(*after_close, (20.0, 80.0), (1.0, 0.0));
    Ok(())
}

#[test]
fn closed_cubic_mid_and_end_markers_use_curve_and_closing_edge_tangents() -> Result<(), String> {
    let tree = parse_closed_cubic_svg(CUBIC_CONTROL_2)?;
    let transforms = marker_path_transforms(&tree);
    let direction = normal_closed_cubic_bisector();
    assert_eq!(transforms.len(), EXPECTED_MARKER_COUNT);
    assert_marker_basis(transforms[MID_MARKER_INDEX], CUBIC_END, direction);
    assert_marker_basis(transforms[END_MARKER_INDEX], PATH_START, direction);
    Ok(())
}

#[test]
fn closed_cubic_with_end_control_at_endpoint_uses_distinct_mid_and_seam_tangents()
-> Result<(), String> {
    let tree = parse_closed_cubic_svg(CUBIC_END)?;
    let transforms = marker_path_transforms(&tree);
    assert_eq!(transforms.len(), EXPECTED_MARKER_COUNT);
    assert_marker_basis(
        transforms[MID_MARKER_INDEX],
        CUBIC_END,
        degenerate_mid_bisector(),
    );
    assert_marker_basis(
        transforms[END_MARKER_INDEX],
        PATH_START,
        subpath_seam_bisector(),
    );
    Ok(())
}

#[test]
fn closed_cubic_with_both_controls_at_endpoint_matches_line_mid_marker() -> Result<(), String> {
    let svg = |path: &str| {
        format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='{VIEWBOX_SIZE}' height='{VIEWBOX_SIZE}'>{MARKER_DEFINITION}<path id='source' d='{path}' fill='none' stroke='black' stroke-opacity='0' marker-mid='url(#m)'/></svg>"
        )
    };
    let cubic = usvg::Tree::from_str(
        &svg("M20 40 C80 80 80 80 80 80 Z"),
        &usvg::Options::default(),
    )
    .map_err(|error| error.to_string())?;
    let line = usvg::Tree::from_str(&svg("M20 40 L80 80 Z"), &usvg::Options::default())
        .map_err(|error| error.to_string())?;
    let cubic_markers = marker_path_transforms(&cubic);
    assert_eq!(cubic_markers.len(), 1);
    assert_eq!(cubic_markers, marker_path_transforms(&line));
    let cubic_image = super::super::render(&cubic, super::super::Mode::Normal)?;
    let line_image = super::super::render(&line, super::super::Mode::Normal)?;
    assert!(line_image.pixels().iter().any(|pixel| pixel.alpha() > 0));
    assert_eq!(cubic_image.data(), line_image.data());
    Ok(())
}

fn parse_closed_cubic_svg(control_2: (f32, f32)) -> Result<usvg::Tree, String> {
    let source = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{VIEWBOX_SIZE}' height='{VIEWBOX_SIZE}' viewBox='0 0 {VIEWBOX_SIZE} {VIEWBOX_SIZE}'>{MARKER_DEFINITION}<path id='source' d='M{} {} C{} {} {} {} {} {} Z' fill='none' stroke='black' marker-mid='url(#m)' marker-end='url(#m)'/></svg>",
        PATH_START.0,
        PATH_START.1,
        CUBIC_CONTROL_1.0,
        CUBIC_CONTROL_1.1,
        control_2.0,
        control_2.1,
        CUBIC_END.0,
        CUBIC_END.1,
    );
    let options = usvg::Options {
        fontdb: upstream_writer_font_database()?,
        ..usvg::Options::default()
    };
    usvg::Tree::from_str(&source, &options).map_err(|error| error.to_string())
}

fn normal_closed_cubic_bisector() -> (f32, f32) {
    let incoming = point_from_xy((
        CUBIC_END.0 - CUBIC_CONTROL_2.0,
        CUBIC_END.1 - CUBIC_CONTROL_2.1,
    ));
    let closing = point_from_xy((PATH_START.0 - CUBIC_END.0, PATH_START.1 - CUBIC_END.1));
    unit_vector_bisector(incoming, closing)
}

fn degenerate_mid_bisector() -> (f32, f32) {
    let incoming = point_from_xy((
        CUBIC_END.0 - CUBIC_CONTROL_1.0,
        CUBIC_END.1 - CUBIC_CONTROL_1.1,
    ));
    let closing = point_from_xy((PATH_START.0 - CUBIC_END.0, PATH_START.1 - CUBIC_END.1));
    unit_vector_bisector(incoming, closing)
}

fn subpath_seam_bisector() -> (f32, f32) {
    let closing = point_from_xy((PATH_START.0 - CUBIC_END.0, PATH_START.1 - CUBIC_END.1));
    let first_curve = point_from_xy((
        CUBIC_CONTROL_1.0 - PATH_START.0,
        CUBIC_CONTROL_1.1 - PATH_START.1,
    ));
    unit_vector_bisector(closing, first_curve)
}
