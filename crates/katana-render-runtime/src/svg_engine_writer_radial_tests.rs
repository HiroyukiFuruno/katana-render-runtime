use super::{render_tree, usvg};

const FOCAL_RADIUS: f32 = 8.0;
const NONZERO_FOCAL_RADIUS_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><defs><radialGradient id="radial" cx="32" cy="32" r="24" fx="24" fy="32" fr="8" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="#ff0000"/><stop offset="1" stop-color="#0000ff"/></radialGradient></defs><rect width="64" height="64" fill="url(#radial)"/></svg>"##;

#[test]
fn writer_preserves_nonzero_radial_focal_radius_and_pixels() -> Result<(), String> {
    let original = parse_svg(NONZERO_FOCAL_RADIUS_SVG)?;
    let serialized = original.to_string(&usvg::WriteOptions::default());
    assert!(
        serialized.contains("fr=\"8\""),
        "writer omitted focal radius"
    );
    let reparsed = parse_svg(&serialized)?;
    assert_eq!(focal_radius(&original)?, FOCAL_RADIUS);
    assert_eq!(focal_radius(&reparsed)?, FOCAL_RADIUS);
    let before = render_tree(&original)?;
    let after = render_tree(&reparsed)?;
    assert!(
        before.data() == after.data(),
        "radial pixels changed after writing"
    );
    Ok(())
}

#[test]
fn writer_omits_default_zero_radial_focal_radius() -> Result<(), String> {
    let input = NONZERO_FOCAL_RADIUS_SVG.replace(" fr=\"8\"", "");
    let tree = parse_svg(&input)?;
    assert_eq!(focal_radius(&tree)?, 0.0);
    let serialized = tree.to_string(&usvg::WriteOptions::default());
    assert!(
        !serialized.contains(" fr="),
        "writer emitted default focal radius"
    );
    Ok(())
}

fn focal_radius(tree: &usvg::Tree) -> Result<f32, String> {
    tree.radial_gradients()
        .first()
        .map(|gradient| gradient.fr().get())
        .ok_or_else(|| "radial fixture did not produce a gradient".to_owned())
}

fn parse_svg(input: &str) -> Result<usvg::Tree, String> {
    usvg::Tree::from_str(input, &usvg::Options::default())
        .map_err(|error| format!("failed parsing radial fixture: {error}"))
}
