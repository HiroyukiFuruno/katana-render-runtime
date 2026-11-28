use super::{
    SvgCompositeRenderer, VIEWPORT_SVG, parse_svg_pixmap, render_existing_svg, render_svg,
};
use resvg::usvg::{BlendMode, Group, Node};
use tiny_skia::Transform;

const RGBA_CHANNELS: usize = 4;
const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><defs><clipPath id="window" transform="translate(10 10)"><rect width="20" height="20"/></clipPath></defs><rect width="40" height="40" fill="#202020"/><g id="clipped" clip-path="url(#window)"><rect width="40" height="40" fill="#e0e0e0" style="mix-blend-mode:multiply"/></g><rect x="32" y="2" width="5" height="5" fill="#ff0000"/></svg>"##;

#[test]
fn transformed_authored_clip_keeps_isolation_and_sibling() -> Result<(), String> {
    let (tree, _) = parse_svg_pixmap(SVG, 40, 40)?;
    let Some(Node::Group(group)) = tree.node_by_id("clipped") else {
        return Err("clipped group missing".to_owned());
    };
    let clip = group.clip_path().ok_or("clip missing")?;
    assert_eq!(clip.transform(), Transform::from_translate(10.0, 10.0));
    assert!(!group.isolate() && group.opacity().get() == 1.0);
    assert_eq!(group.blend_mode(), BlendMode::Normal);
    assert!(group.filters().is_empty() && group.mask().is_none());
    assert!(has_nonisolating_multiply(group));
    assert!(!SvgCompositeRenderer::needs_authored_clip_ids(&tree));
    let (viewport, _) = parse_svg_pixmap(VIEWPORT_SVG, 40, 40)?;
    assert!(SvgCompositeRenderer::needs_authored_clip_ids(&viewport));
    let actual = render_svg(SVG, 40, 40, Transform::identity())?;
    let expected = render_existing_svg(SVG, 40, 40, Transform::identity())?;
    assert_eq!(actual.data(), expected.data());
    assert_pixel(&actual, 5, 5, [32, 32, 32, 255]);
    assert_pixel(&actual, 15, 15, [224, 224, 224, 255]);
    assert_pixel(&actual, 34, 4, [255, 0, 0, 255]);
    Ok(())
}

fn has_nonisolating_multiply(group: &Group) -> bool {
    group.children().iter().any(|node| match node {
        Node::Group(child) => {
            (child.blend_mode() == BlendMode::Multiply
                && !child.isolate()
                && child.opacity().get() == 1.0
                && child.filters().is_empty()
                && child.mask().is_none()
                && child.clip_path().is_none())
                || has_nonisolating_multiply(child)
        }
        _ => false,
    })
}

fn assert_pixel(pixmap: &tiny_skia::Pixmap, x: u32, y: u32, expected: [u8; RGBA_CHANNELS]) {
    let actual = pixmap
        .pixel(x, y)
        .map(|pixel| [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()]);
    assert_eq!(actual, Some(expected));
}
