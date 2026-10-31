use std::sync::Arc;

use super::super::{Image, ImageKind, ImageRendering, Node, NonZeroRect, Size, Transform};
use super::{MODEL_ORIGIN, parse_model_svg, text_node};

const EMPTY_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"/>"#;
const UNIT_IMAGE_SIZE: (f32, f32) = (1.0, 1.0);

#[test]
fn image_kind_debug_hides_payloads_for_each_variant() -> Result<(), String> {
    let nested_svg = parse_model_svg(EMPTY_SVG)?;
    let payload = Arc::new(b"embedded-image-payload".to_vec());
    let cases = [
        (ImageKind::JPEG(Arc::clone(&payload)), "ImageKind::JPEG(..)"),
        (ImageKind::PNG(Arc::clone(&payload)), "ImageKind::PNG(..)"),
        (ImageKind::GIF(Arc::clone(&payload)), "ImageKind::GIF(..)"),
        (ImageKind::WEBP(Arc::clone(&payload)), "ImageKind::WEBP(..)"),
        (ImageKind::SVG(nested_svg), "ImageKind::SVG(..)"),
    ];
    for (kind, expected) in cases {
        assert_eq!(format!("{kind:?}"), expected);
    }
    Ok(())
}

#[test]
fn tree_text_presence_descends_through_an_embedded_svg_image() -> Result<(), String> {
    let mut outer = parse_model_svg(EMPTY_SVG)?;
    assert!(!outer.has_text_nodes());
    let mut nested = parse_model_svg(EMPTY_SVG)?;
    nested
        .root
        .children
        .push(text_node("nested-text", Transform::default())?);
    assert!(nested.has_text_nodes());
    let image = svg_image_node("embedded", nested)?;
    super::assert_node_identity_and_transform(&image, "embedded", MODEL_ORIGIN);
    outer.root.children.push(image);
    assert!(outer.has_text_nodes());
    Ok(())
}

fn svg_image_node(id: &str, tree: super::super::super::Tree) -> Result<Node, String> {
    let size = Size::from_wh(UNIT_IMAGE_SIZE.0, UNIT_IMAGE_SIZE.1).ok_or("image size invalid")?;
    let bounds = NonZeroRect::from_xywh(
        MODEL_ORIGIN.0,
        MODEL_ORIGIN.1,
        UNIT_IMAGE_SIZE.0,
        UNIT_IMAGE_SIZE.1,
    )
    .ok_or("image bounds invalid")?;
    Ok(Node::Image(Box::new(Image {
        id: id.to_owned(),
        visible: true,
        size,
        rendering_mode: ImageRendering::OptimizeQuality,
        kind: ImageKind::SVG(tree),
        abs_transform: Transform::default(),
        abs_bounding_box: bounds,
    })))
}
