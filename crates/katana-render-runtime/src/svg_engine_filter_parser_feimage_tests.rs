use super::{parse_svg_with_writer_fonts, usvg};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use rstest::rstest;

const EMBEDDED_SVG_IMAGE_MARKUP: &str = "<image id='source' width='1' height='1' href='data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIxIiBoZWlnaHQ9IjEiPjxyZWN0IHdpZHRoPSIxIiBoZWlnaHQ9IjEiIGZpbGw9InJlZCIvPjwvc3ZnPg=='/>";
const PNG: &[u8] =
    include_bytes!("../../../tests/fixtures/svg-engine/upstream/resvg/tests/resources/image.png");
const EXPECTED_OPACITY: f32 = 0.5;

#[rstest]
#[case::path("<rect id='source' opacity='0.5' width='1' height='1'/>", "path")]
#[case::image(EMBEDDED_SVG_IMAGE_MARKUP, "image")]
#[case::text(
    "<text id='source' opacity='0.5' x='0' y='8' font-family='Noto Sans' font-size='8'>A</text>",
    "text"
)]
#[case::group(
    "<g id='source' opacity='0.5'><g opacity='0.5'><rect width='1' height='1'/></g></g>",
    "group"
)]
fn linked_feimage_moves_supported_child_ids_to_its_wrapper(
    #[case] child_markup: &str,
    #[case] child_kind: &str,
) -> Result<(), String> {
    let source = linked_feimage_svg(child_markup);
    let tree = parse_svg_with_writer_fonts(&source)?;
    assert_feimage_child(&tree, child_kind)
}

#[test]
fn linked_png_preserves_bytes_opacity_and_unique_id() -> Result<(), String> {
    let encoded = STANDARD.encode(PNG);
    let markup = format!(
        "<image id='source' opacity='0.5' width='1' height='1' href='data:image/png;base64,{encoded}'/>"
    );
    let tree = parse_svg_with_writer_fonts(&linked_feimage_svg(&markup))?;
    let root = filter_image_root(&tree)?;
    let wrapper = first_group(root)?;
    assert_eq!(wrapper.id(), "source");
    assert_eq!(wrapper.opacity().get(), EXPECTED_OPACITY);
    let inner = first_group(wrapper)?;
    assert_eq!(inner.id(), "");
    assert_png_child(inner)?;
    assert_eq!(count_id(root, "source"), 1);
    Ok(())
}

fn linked_feimage_svg(target_markup: &str) -> String {
    format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='16' height='16'><defs><filter id='f' filterUnits='userSpaceOnUse' x='0' y='0' width='16' height='16'><feImage href='#source'/></filter></defs>{target_markup}<rect id='sample' x='1' y='1' width='8' height='8' filter='url(#f)'/></svg>"
    )
}

fn assert_feimage_child(tree: &usvg::Tree, child_kind: &str) -> Result<(), String> {
    let wrapper = first_group(filter_image_root(tree)?)?;
    assert_eq!(wrapper.id(), "source");
    let child = wrapper.children().first().ok_or("wrapped child missing")?;
    assert_eq!(child.id(), "");
    assert_image_child_kind(child, child_kind)
}

fn filter_image_root(tree: &usvg::Tree) -> Result<&usvg::Group, String> {
    let primitive = tree
        .filters()
        .first()
        .and_then(|filter| filter.primitives().first())
        .ok_or("feImage primitive missing")?;
    let usvg::filter::Kind::Image(image) = primitive.kind() else {
        return Err("feImage image data missing".to_owned());
    };
    Ok(image.root())
}

fn first_group(group: &usvg::Group) -> Result<&usvg::Group, String> {
    match group.children().first() {
        Some(usvg::Node::Group(child)) => Ok(child),
        _ => Err("feImage wrapper group missing".to_owned()),
    }
}

fn assert_image_child_kind(child: &usvg::Node, expected: &str) -> Result<(), String> {
    let matches = matches!(
        (expected, child),
        ("path", usvg::Node::Path(_))
            | ("image", usvg::Node::Image(_))
            | ("text", usvg::Node::Text(_))
            | ("group", usvg::Node::Group(_))
    );
    if !matches {
        return Err(format!("feImage child kind mismatch: {expected}"));
    }
    Ok(())
}

fn assert_png_child(group: &usvg::Group) -> Result<(), String> {
    let Some(usvg::Node::Image(image)) = group.children().first() else {
        return Err("PNG image child missing".to_owned());
    };
    assert_eq!(image.id(), "");
    let usvg::ImageKind::PNG(bytes) = image.kind() else {
        return Err("image is not PNG".to_owned());
    };
    assert_eq!(bytes.as_slice(), PNG);
    Ok(())
}

fn count_id(group: &usvg::Group, target: &str) -> usize {
    usize::from(group.id() == target)
        + group
            .children()
            .iter()
            .map(|child| match child {
                usvg::Node::Group(nested) => count_id(nested, target),
                _ => usize::from(child.id() == target),
            })
            .sum::<usize>()
}
