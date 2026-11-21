use super::{assert_image_reference, assert_rendered_pixels, parse_writer_svg, writer_options};
use crate::svg_engine::usvg;

const REPEATED_IMAGE_REFERENCE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="80">
  <defs><g id="target"><rect width="40" height="40" fill="lime"/></g>
    <filter id="linked" x="0" y="0" width="40" height="40" filterUnits="userSpaceOnUse">
      <feImage href="#target" result="first"/>
      <feImage href="#target" result="second"/>
      <feBlend in="first" in2="second" mode="multiply"/>
    </filter>
  </defs><rect width="40" height="40" fill="red" filter="url(#linked)"/>
</svg>"##;

#[test]
fn repeated_filter_image_writes_one_definition_and_preserves_both_references() -> Result<(), String>
{
    let case = "repeated filter image";
    let original = parse_writer_svg(REPEATED_IMAGE_REFERENCE, case)?;
    let serialized = original.to_string(&writer_options(case));
    assert_repeated_reference(&serialized)?;
    let reparsed = parse_writer_svg(&serialized, case)?;
    assert_rendered_pixels(&original, &reparsed, case)
}

#[test]
fn invalid_filter_images_roundtrip_without_dangling_references() -> Result<(), String> {
    for image in ["<feImage/>", "<feImage href='#empty'/>"] {
        let input = format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='80' height='80'><defs><g id='empty'/><filter id='f'>{image}</filter></defs><rect width='80' height='80' fill='blue'/><rect width='40' height='40' fill='red' filter='url(#f)'/></svg>"
        );
        let original = parse_writer_svg(&input, "invalid feImage")?;
        let serialized = original.to_string(&writer_options("invalid feImage"));
        assert_filter_image_targets_resolve(&serialized)?;
        let reparsed = parse_writer_svg(&serialized, "invalid feImage roundtrip")?;
        assert_rendered_pixels(&original, &reparsed, "invalid feImage")?;
    }
    Ok(())
}

fn assert_filter_image_targets_resolve(serialized: &str) -> Result<(), String> {
    let document =
        usvg::roxmltree::Document::parse(serialized).map_err(|error| error.to_string())?;
    for image in document
        .descendants()
        .filter(|node| node.has_tag_name("feImage"))
    {
        let target = image
            .attribute(("http://www.w3.org/1999/xlink", "href"))
            .and_then(|href| href.strip_prefix('#'))
            .filter(|target| !target.is_empty())
            .ok_or("serialized feImage has no local target")?;
        assert_eq!(
            document
                .descendants()
                .filter(|node| node.attribute("id") == Some(target))
                .count(),
            1,
            "serialized feImage target {target:?} must resolve to one node"
        );
    }
    Ok(())
}

fn assert_repeated_reference(serialized: &str) -> Result<(), String> {
    let document =
        usvg::roxmltree::Document::parse(serialized).map_err(|error| error.to_string())?;
    let mut references = document
        .descendants()
        .filter(|node| node.has_tag_name("feImage"))
        .map(|node| node.attribute(("http://www.w3.org/1999/xlink", "href")));
    let first = references
        .next()
        .flatten()
        .ok_or("first feImage href missing")?;
    assert_eq!(references.next(), Some(Some(first)));
    assert_eq!(references.next(), None);
    let target = first
        .strip_prefix('#')
        .ok_or("feImage target must be local")?;
    let definitions = document
        .descendants()
        .filter(|node| node.attribute("id") == Some(target));
    assert_eq!(definitions.count(), 1);
    Ok(())
}

#[test]
fn mask_reference_to_a_non_mask_element_discards_the_masked_element() -> Result<(), String> {
    assert_invalid_mask_discards_only_target(
        r#"<rect id="not-a-mask" width="10" height="10"/>"#,
        "not-a-mask",
    )
}

#[test]
fn invalid_linked_mask_discards_only_the_masked_element() -> Result<(), String> {
    assert_invalid_mask_discards_only_target(
        r##"<mask id="invalid" width="0" height="10"><rect width="10" height="10"/></mask>
        <mask id="outer" mask="url(#invalid)" maskUnits="userSpaceOnUse"
          width="10" height="10"><rect width="10" height="10" fill="white"/></mask>"##,
        "outer",
    )
}

fn assert_invalid_mask_discards_only_target(definitions: &str, mask: &str) -> Result<(), String> {
    let input = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40">
        <defs>{definitions}</defs>
        <rect id="unmasked" x="20" width="10" height="10"/>
        <rect id="masked" width="10" height="10" mask="url(#{mask})"/></svg>"##
    );
    let tree = parse_writer_svg(&input, mask)?;
    assert!(tree.node_by_id("unmasked").is_some());
    assert!(tree.node_by_id("masked").is_none());
    Ok(())
}

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
