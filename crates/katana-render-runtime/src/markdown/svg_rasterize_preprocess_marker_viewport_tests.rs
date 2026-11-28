use super::{RGBA_CHANNEL_COUNT, rendered_pixels};
use resvg::usvg::{Node, Options, ShapeRendering, Tree};

#[test]
fn marker_viewport_clip_is_crisp_without_changing_user_or_marker_shapes() -> Result<(), String> {
    let svg = marker_clip_svg();
    let tree = Tree::from_str(&svg, &Options::default()).map_err(|error| error.to_string())?;
    assert_clip_rendering_modes(&tree)?;
    assert_marker_paths_keep_antialiasing(&tree);
    Ok(())
}

#[test]
fn marker_child_pixels_outside_viewport_are_clipped() -> Result<(), String> {
    let image = rendered_pixels(&marker_clip_svg()).ok_or("marker SVG did not rasterize")?;
    assert_eq!((image.0, image.1), (24, 16));
    assert_eq!(pixel(&image.2, image.0, 10, 8)?, [255, 0, 0, 255]);
    assert_eq!(pixel(&image.2, image.0, 16, 8)?[3], 0);
    Ok(())
}

#[test]
fn recursive_marker_preserves_the_first_marker_and_connector() -> Result<(), String> {
    let control = marker_clip_svg();
    let recursive = control.replace(
        r##"fill="#ff0000""##,
        r##"fill="#ff0000" marker-end="url(#arrow)""##,
    );
    let expected = rendered_pixels(&control).ok_or("control marker did not rasterize")?;
    let actual = rendered_pixels(&recursive).ok_or("recursive marker did not rasterize")?;
    assert_eq!((actual.0, actual.1), (expected.0, expected.1));
    assert!(
        actual.2 == expected.2,
        "recursive reference changed marker pixels"
    );
    Ok(())
}

fn marker_clip_svg() -> String {
    String::from(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="16"><defs><clipPath id="user-clip"><path d="M0 0H24V16H0Z"/></clipPath><marker id="arrow" markerWidth="4" markerHeight="4" refX="0" refY="2" markerUnits="userSpaceOnUse" orient="0"><path d="M0 0H12V4H0Z" fill="#ff0000"/></marker></defs><path d="M4 8H8" fill="none" stroke="#000000" stroke-width="1" clip-path="url(#user-clip)" marker-end="url(#arrow)"/></svg>"##,
    )
}

fn assert_clip_rendering_modes(tree: &Tree) -> Result<(), String> {
    let user_clip = tree
        .clip_paths()
        .iter()
        .find(|clip| clip.id() == "user-clip")
        .ok_or("missing authored user clip")?;
    let generated_clip = tree
        .clip_paths()
        .iter()
        .find(|clip| clip.id() != "user-clip")
        .ok_or("missing generated marker clip")?;
    assert_eq!(
        clip_path_mode(user_clip)?,
        ShapeRendering::GeometricPrecision
    );
    assert_eq!(clip_path_mode(generated_clip)?, ShapeRendering::CrispEdges);
    Ok(())
}

fn assert_marker_paths_keep_antialiasing(tree: &Tree) {
    let mut root_modes = Vec::new();
    collect_path_modes(tree.root(), &mut root_modes);
    assert!(root_modes.len() >= 2, "expected connector and marker paths");
    assert!(
        root_modes
            .iter()
            .filter(|mode| **mode == ShapeRendering::GeometricPrecision)
            .count()
            >= 2,
        "marker child path must retain anti-aliasing"
    );
}

fn clip_path_mode(clip: &resvg::usvg::ClipPath) -> Result<ShapeRendering, String> {
    match clip.root().children().first() {
        Some(Node::Path(path)) => Ok(path.rendering_mode()),
        _ => Err("clip path root must contain one path".to_owned()),
    }
}

fn collect_path_modes(group: &resvg::usvg::Group, modes: &mut Vec<ShapeRendering>) {
    for node in group.children() {
        match node {
            Node::Group(group) => collect_path_modes(group, modes),
            Node::Path(path) => modes.push(path.rendering_mode()),
            Node::Image(_) | Node::Text(_) => {}
        }
    }
}

fn pixel(rgba: &[u8], width: u32, x: u32, y: u32) -> Result<[u8; RGBA_CHANNEL_COUNT], String> {
    let offset = ((y * width + x) * RGBA_CHANNEL_COUNT as u32) as usize;
    let channels = rgba
        .get(offset..offset + RGBA_CHANNEL_COUNT)
        .ok_or_else(|| "pixel is outside raster bounds".to_owned())?;
    channels
        .try_into()
        .map_err(|_| "pixel must contain four channels".to_owned())
}
