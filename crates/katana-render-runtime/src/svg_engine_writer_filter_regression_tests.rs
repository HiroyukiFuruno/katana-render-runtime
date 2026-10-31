use super::{
    parse_writer_svg, render_tree,
    writer_coverage_tests::{assert_attribute, assert_pixels_equal},
    writer_options,
};
use rstest::rstest;
use std::path::Path;

#[path = "svg_engine_writer_lighting_input_tests.rs"]
mod lighting_input_tests;

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
#[case("feDropShadow/with-offset", "feDropShadow", "stdDeviation=")]
#[case("feGaussianBlur/simple-case", "feGaussianBlur", "stdDeviation=")]
#[case("feOffset/simple-case", "feOffset", "dx=\"20\"")]
#[case("feBlend/mode=multiply", "feBlend", "mode=\"multiply\"")]
#[case("feFlood/seagreen", "feFlood", "flood-color=")]
#[case("feMerge/color-interpolation-filters=sRGB", "feMerge", "feMergeNode")]
#[case(
    "feDiffuseLighting/multiple-light-sources",
    "feDiffuseLighting",
    "feDistantLight"
)]
#[case("feConvolveMatrix/bias=-0.5", "feConvolveMatrix", "bias=\"-0.5\"")]
#[case("feMorphology/operator=dilate", "feMorphology", "operator=\"dilate\"")]
#[case("feMorphology/fractional-radius", "feMorphology", "operator=\"erode\"")]
#[case(
    "feComponentTransfer/type=identity-on-all",
    "feComponentTransfer",
    "type=\"identity\""
)]
#[case(
    "feMerge/color-interpolation-filters=linearRGB",
    "feMerge",
    "color-interpolation-filters=\"linearRGB\""
)]
#[case(
    "feSpecularLighting/with-feSpotLight",
    "feSpecularLighting",
    "in=\"SourceAlpha\""
)]
#[case(
    "feSpotLight/limitingConeAngle=30",
    "feSpotLight",
    "limitingConeAngle=\"30\""
)]
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
    assert_fixture_filter_input(&original, fixture)?;
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

fn assert_fixture_filter_input(tree: &super::usvg::Tree, fixture: &str) -> Result<(), String> {
    assert!(!tree.filters().is_empty(), "{fixture}: no input filter");
    if fixture == "feSpecularLighting/with-feSpotLight" {
        assert_filter_has_input(tree, &super::usvg::filter::Input::SourceAlpha, fixture)?;
    }
    Ok(())
}

fn assert_filter_has_input(
    tree: &super::usvg::Tree,
    input: &super::usvg::filter::Input,
    fixture: &str,
) -> Result<(), String> {
    if tree
        .filters()
        .iter()
        .flat_map(|filter| filter.primitives())
        .any(|primitive| primitive.kind().has_input(input))
    {
        return Ok(());
    }
    Err(format!("{fixture}: parsed filter input missing"))
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

#[test]
fn repeated_fe_image_target_is_defined_once_and_keeps_both_references() -> Result<(), String> {
    let input = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200">
      <defs><g id="target"><rect width="100" height="100" fill="lime"/></g>
        <filter id="twice"><feImage href="#target"/><feImage href="#target"/></filter>
      </defs><rect width="140" height="140" filter="url(#twice)"/></svg>"##;
    let original = parse_writer_svg(input, "repeated-fe-image-target")?;
    let options = writer_options("repeated-fe-image-target");
    let serialized = original.to_string(&options);
    assert_eq!(serialized.matches("id=\"target\"").count(), 1);
    assert_eq!(serialized.matches("xlink:href=\"#target\"").count(), 2);
    let reparsed = parse_writer_svg(&serialized, "repeated-fe-image-target")?;
    assert_eq!(reparsed.filters().len(), original.filters().len());
    assert_rendered_pixels(&original, &reparsed, "repeated-fe-image-target")
}

#[test]
fn primitive_subregion_and_source_alpha_survive_writer_roundtrip() -> Result<(), String> {
    let input = r##"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="40">
      <defs><filter id="subregion" x="0" y="0" width="80" height="40">
        <feOffset in="SourceAlpha" dx="3" dy="4" x="11" y="12" width="13" height="14"/>
      </filter></defs>
      <rect width="30" height="30" fill="red" filter="url(#subregion)"/>
    </svg>"##;
    let original = parse_writer_svg(input, "filter-primitive-subregion")?;
    let options = writer_options("filter-primitive-subregion");
    let serialized = original.to_string(&options);
    assert_filter_subregion(&serialized)?;
    let reparsed = parse_writer_svg(&serialized, "filter-primitive-subregion")?;
    assert_filter_subregion(&reparsed.to_string(&options))?;
    assert_pixels_equal(&original, &reparsed, "filter-primitive-subregion")?;
    Ok(())
}

fn assert_filter_subregion(svg: &str) -> Result<(), String> {
    for (attribute, value) in [
        ("in", "SourceAlpha"),
        ("x", "11"),
        ("y", "12"),
        ("width", "13"),
        ("height", "14"),
    ] {
        assert_attribute(svg, "feOffset", attribute, value)?;
    }
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
