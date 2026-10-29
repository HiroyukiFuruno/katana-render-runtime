use super::{parse_writer_svg, render_tree, writer_options};
use rstest::rstest;
use std::path::Path;

const FILTER_FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/svg-engine/upstream/resvg/tests/tests/filters"
);
const INTERIOR_START: u32 = 30;
const INTERIOR_END: u32 = 170;
const LINKED_IMAGE_CENTER: u32 = 20;
const OPAQUE_CHANNEL: u8 = 255;

#[rstest]
#[case(
    "feComponentTransfer/mixed-types",
    "feComponentTransfer",
    "type=\"gamma\""
)]
#[case(
    "feColorMatrix/type=luminanceToAlpha",
    "feColorMatrix",
    "type=\"luminanceToAlpha\""
)]
#[case(
    "feComposite/operator=arithmetic-on-sRGB",
    "feComposite",
    "operator=\"arithmetic\""
)]
#[case("feConvolveMatrix/bias=-0.5", "feConvolveMatrix", "bias=\"-0.5\"")]
#[case("feMorphology/operator=dilate", "feMorphology", "operator=\"dilate\"")]
#[case(
    "feTurbulence/baseFrequency=0.01",
    "feTurbulence",
    "baseFrequency=\"0.01 0.01\""
)]
#[case("feTile/simple-case", "feTile", "feOffset")]
#[case("feImage/embedded-png", "feImage", "data:image/png;base64")]
#[case(
    "feImage/link-on-an-element-with-transform",
    "feImage",
    "xlink:href=\"#"
)]
#[case(
    "feSpecularLighting/with-fePointLight",
    "feSpecularLighting",
    "fePointLight"
)]
#[case(
    "feSpecularLighting/with-feSpotLight-and-specular-and-exponent",
    "feSpecularLighting",
    "feSpotLight"
)]
fn filter_writer_preserves_renderable_semantics(
    #[case] fixture: &str,
    #[case] primitive: &str,
    #[case] attribute: &str,
) -> Result<(), String> {
    let input = std::fs::read_to_string(Path::new(FILTER_FIXTURES).join(format!("{fixture}.svg")))
        .map_err(|error| error.to_string())?;
    let original = parse_writer_svg(&input, fixture)?;
    assert!(!original.filters().is_empty(), "{fixture}: no input filter");
    let options = writer_options(fixture);
    let serialized = original.to_string(&options);
    assert_serialized_filter(&serialized, primitive, attribute, fixture);
    let parsed = parse_writer_svg(&serialized, fixture)?;
    assert_filter_structure(&original, &parsed, fixture);
    let serialized_again = parsed.to_string(&options);
    assert_serialized_filter(&serialized_again, primitive, attribute, fixture);
    let reparsed = parse_writer_svg(&serialized_again, fixture)?;
    assert_filter_structure(&parsed, &reparsed, fixture);
    let serialized_third = reparsed.to_string(&options);
    assert_serialized_filter(&serialized_third, primitive, attribute, fixture);
    if primitive == "feImage" {
        assert_image_reference(&original, &serialized)?;
        assert_image_reference(&parsed, &serialized_again)?;
        assert_image_reference(&reparsed, &serialized_third)?;
    }
    if fixture == "feImage/embedded-png" {
        assert_rendered_pixels(&original, &reparsed, fixture)?;
    }
    assert_rendered_pixels(&parsed, &reparsed, fixture)
}

fn assert_serialized_filter(svg: &str, primitive: &str, attribute: &str, fixture: &str) {
    assert!(svg.contains(&format!("<{primitive}")), "{fixture}");
    assert!(svg.contains(attribute), "{fixture}");
    assert!(svg.contains("url(#"), "{fixture}: missing reference");
}

#[test]
fn linked_image_preserves_group_and_child_ids() -> Result<(), String> {
    let input = r##"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="80">
      <defs><g id="parent"><rect id="child" width="40" height="40" fill="#00ff00"/></g>
      <filter id="filter"><feImage href="#parent"/></filter></defs>
      <rect width="40" height="40" filter="url(#filter)"/></svg>"##;
    let original = parse_writer_svg(input, "linked-ids")?;
    let options = writer_options("linked-ids");
    let output = original.to_string(&options);
    assert_image_reference(&original, &output)?;
    assert!(output.contains("id=\"parent\""));
    assert!(output.contains("id=\"child\""));
    let parsed = parse_writer_svg(&output, "linked-ids")?;
    let serialized_again = parsed.to_string(&options);
    assert_image_reference(&parsed, &serialized_again)?;
    assert!(serialized_again.contains("id=\"parent\""));
    assert!(serialized_again.contains("id=\"child\""));
    let before = render_tree(&original)?;
    let after = render_tree(&parsed)?;
    assert_eq!(before.data(), after.data());
    let pixel = after
        .pixel(LINKED_IMAGE_CENTER, LINKED_IMAGE_CENTER)
        .ok_or("missing interior pixel")?;
    assert_eq!(
        (pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()),
        (0, OPAQUE_CHANNEL, 0, OPAQUE_CHANNEL)
    );
    Ok(())
}

fn assert_image_reference(tree: &super::usvg::Tree, serialized: &str) -> Result<(), String> {
    for primitive in tree.filters().iter().flat_map(|filter| filter.primitives()) {
        if let super::usvg::filter::Kind::Image(image) = primitive.kind() {
            let child = image
                .root()
                .children()
                .first()
                .ok_or("empty feImage root")?;
            let id = child.id();
            assert!(!id.is_empty(), "feImage target lacks an ID");
            assert!(serialized.contains(&format!("xlink:href=\"#{id}\"")));
            assert!(serialized.contains(&format!("id=\"{id}\"")));
            return Ok(());
        }
    }
    Err("feImage primitive missing".into())
}

fn assert_filter_structure(
    original: &super::usvg::Tree,
    parsed: &super::usvg::Tree,
    fixture: &str,
) {
    assert_eq!(original.size(), parsed.size(), "{fixture}: viewport");
    assert_eq!(
        original.filters().len(),
        parsed.filters().len(),
        "{fixture}: filters"
    );
    for (before, after) in original.filters().iter().zip(parsed.filters()) {
        assert_eq!(
            before.primitives().len(),
            after.primitives().len(),
            "{fixture}: primitives"
        );
    }
}

fn assert_rendered_pixels(
    parsed: &super::usvg::Tree,
    reparsed: &super::usvg::Tree,
    fixture: &str,
) -> Result<(), String> {
    let first = render_tree(parsed)?;
    let second = render_tree(reparsed)?;
    assert_eq!(first.data(), second.data(), "{fixture}: roundtrip pixels");
    assert!(
        (INTERIOR_START..INTERIOR_END).any(|y| (INTERIOR_START..INTERIOR_END)
            .any(|x| first.pixel(x, y).is_some_and(|pixel| pixel.alpha() != 0))),
        "{fixture}: empty interior"
    );
    Ok(())
}
