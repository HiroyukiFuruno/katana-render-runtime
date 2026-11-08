use super::{assert_image_reference, assert_rendered_pixels, parse_writer_svg, writer_options};
use crate::svg_engine::usvg;

#[test]
fn linked_image_inside_nested_mask_preserves_xlink_and_rendering() -> Result<(), String> {
    let input = r##"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="80">
      <defs><g id="target"><rect width="40" height="40" fill="lime"/></g>
        <filter id="linked"><feImage href="#target"/></filter>
        <mask id="inner" mask-type="alpha" maskUnits="userSpaceOnUse"
          x="0" y="0" width="40" height="40">
          <g filter="url(#linked)"><rect width="40" height="40" fill="white"/></g>
        </mask>
        <mask id="outer" mask="url(#inner)" maskUnits="userSpaceOnUse"
          x="0" y="0" width="40" height="40">
          <rect width="40" height="40" fill="white"/>
        </mask>
      </defs><rect width="40" height="40" fill="red" mask="url(#outer)"/></svg>"##;
    let original = parse_writer_svg(input, "nested-mask-linked-image")?;
    assert_nested_mask_chain(&original)?;
    let options = writer_options("nested-mask-linked-image");
    let serialized = original.to_string(&options);
    assert!(serialized.contains("xmlns:xlink="));
    assert_image_reference(&original, &serialized)?;

    let parsed = parse_writer_svg(&serialized, "nested-mask-linked-image-roundtrip")?;
    assert_nested_mask_chain(&parsed)?;
    assert_second_roundtrip(&original, &parsed, &options)
}

fn assert_second_roundtrip(
    original: &usvg::Tree,
    parsed: &usvg::Tree,
    options: &usvg::WriteOptions,
) -> Result<(), String> {
    let serialized_again = parsed.to_string(options);
    assert!(serialized_again.contains("xmlns:xlink="));
    assert_image_reference(parsed, &serialized_again)?;
    let reparsed = parse_writer_svg(
        &serialized_again,
        "nested-mask-linked-image-second-roundtrip",
    )?;
    assert_nested_mask_chain(&reparsed)?;
    assert_rendered_pixels(original, &reparsed, "nested-mask-linked-image")
}

fn assert_nested_mask_chain(tree: &usvg::Tree) -> Result<(), String> {
    if tree.masks().iter().any(|mask| mask.mask().is_some()) {
        return Ok(());
    }
    Err("nested mask relationship missing".into())
}
