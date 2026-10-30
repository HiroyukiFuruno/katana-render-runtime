use super::{assert_rgba_pixel, parse_writer_svg, render_tree, usvg, writer_options};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::ImageEncoder;

const IMAGE_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/svg-engine/upstream/resvg/tests/tests/structure/image/embedded-svg.svg"
);
const RGBA_PNG: [u8; 16] = [
    255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
];
type PngPixelSample = ((u32, u32), (u8, u8, u8, u8));

const PNG_SAMPLES: [PngPixelSample; 4] = [
    ((1, 1), (255, 0, 0, 255)),
    ((2, 1), (0, 255, 0, 255)),
    ((1, 2), (0, 0, 255, 255)),
    ((2, 2), (255, 255, 0, 255)),
];

#[test]
fn embedded_svg_image_preserves_kind_size_and_pixel_through_writer() -> Result<(), String> {
    let input = std::fs::read_to_string(IMAGE_FIXTURE).map_err(|error| error.to_string())?;
    let original = parse_writer_svg(&input, "embedded-svg-image")?;
    assert_embedded_svg(&original)?;
    let output = original.to_string(&writer_options("embedded-svg-image"));
    assert!(output.contains("data:image/svg+xml;base64"));
    let reparsed = parse_writer_svg(&output, "embedded-svg-image-roundtrip")?;
    assert_embedded_svg(&reparsed)?;
    assert_roundtrip_pixels(&original, &reparsed, (60, 11), (255, 153, 0, 255))
}

#[test]
fn inline_rgba_png_decodes_places_and_roundtrips() -> Result<(), String> {
    let input = inline_png_svg()?;
    let original = parse_writer_svg(&input, "inline-rgba-png")?;
    assert_png_image(&original)?;
    let output = original.to_string(&writer_options("inline-rgba-png"));
    assert!(output.contains("data:image/png;base64"));
    let reparsed = parse_writer_svg(&output, "inline-rgba-png-roundtrip")?;
    assert_png_image(&reparsed)?;
    let before = render_tree(&original)?;
    let after = render_tree(&reparsed)?;
    assert_eq!(before.data(), after.data(), "roundtrip pixels");
    for pixmap in [&before, &after] {
        for (point, expected) in PNG_SAMPLES {
            assert_rgba_pixel(pixmap, point, expected)?;
        }
        assert_rgba_pixel(pixmap, (0, 0), (0, 0, 0, 0))?;
    }
    Ok(())
}

fn inline_png_svg() -> Result<String, String> {
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(&RGBA_PNG, 2, 2, image::ExtendedColorType::Rgba8)
        .map_err(|error| error.to_string())?;
    let data = STANDARD.encode(bytes);
    Ok(format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4">
          <image id="rgba" x="1" y="1" width="2" height="2"
            href="data:image/png;base64,{data}"/>
        </svg>"#
    ))
}

fn assert_embedded_svg(tree: &usvg::Tree) -> Result<(), String> {
    let image = image_node(tree, "image1")?;
    assert_image_size(image, (100.0, 100.0));
    assert!(matches!(image.kind(), usvg::ImageKind::SVG(_)));
    Ok(())
}

fn assert_png_image(tree: &usvg::Tree) -> Result<(), String> {
    let image = image_node(tree, "rgba")?;
    assert_image_size(image, (2.0, 2.0));
    assert!(matches!(image.kind(), usvg::ImageKind::PNG(_)));
    Ok(())
}

fn image_node<'a>(tree: &'a usvg::Tree, id: &str) -> Result<&'a usvg::Image, String> {
    let node = tree
        .node_by_id(id)
        .ok_or_else(|| format!("image element ID {id} missing"))?;
    assert_eq!(node.id(), id, "image element ID");
    match node {
        usvg::Node::Image(image) => Ok(image.as_ref()),
        usvg::Node::Group(group) => {
            find_image_node(group).ok_or_else(|| format!("image under element {id} missing"))
        }
        _ => Err(format!("element {id} does not contain an image")),
    }
}

fn find_image_node(group: &usvg::Group) -> Option<&usvg::Image> {
    group.children().iter().find_map(|node| match node {
        usvg::Node::Image(image) => Some(image.as_ref()),
        usvg::Node::Group(group) => find_image_node(group),
        _ => None,
    })
}

fn assert_image_size(image: &usvg::Image, size: (f32, f32)) {
    assert_eq!((image.size().width(), image.size().height()), size);
}

fn assert_roundtrip_pixels(
    original: &usvg::Tree,
    reparsed: &usvg::Tree,
    point: (u32, u32),
    expected: (u8, u8, u8, u8),
) -> Result<(), String> {
    let before = render_tree(original)?;
    let after = render_tree(reparsed)?;
    assert_eq!(before.data(), after.data(), "roundtrip pixels");
    assert_rgba_pixel(&before, point, expected)?;
    assert_rgba_pixel(&after, point, expected)
}
