use std::path::Path;

use rstest::rstest;

use super::{ALPHA_CHANNEL, RGBA_CHANNELS, parse_writer_svg, render_tree, usvg};

const FILTER_FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/svg-engine/upstream/resvg/tests/tests/filters"
);
const STYLE_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="40">
<defs>
  <linearGradient id="reflect" spreadMethod="reflect"><stop stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient>
  <linearGradient id="repeat" spreadMethod="repeat"><stop stop-color="green"/><stop offset="1" stop-color="white"/></linearGradient>
</defs>
<path d="M4 4 H35 V35 H4 Z" fill="url(#reflect)" fill-opacity="0.5" stroke="black" stroke-opacity="0.5" stroke-width="3" stroke-linecap="round" stroke-linejoin="bevel" stroke-dasharray="4 2" stroke-dashoffset="1" paint-order="stroke fill"/>
<path d="M45 4 H75 V35 H45 Z" fill="url(#repeat)" stroke="black" stroke-width="2" stroke-linecap="square" stroke-linejoin="round"/>
</svg>"##;

#[test]
fn styled_paths_and_gradient_spread_survive_writer_roundtrip() -> Result<(), String> {
    let original = parse_writer_svg(STYLE_SVG, "writer-style-coverage")?;
    let options = usvg::WriteOptions::default();
    let serialized = original.to_string(&options);
    assert_style_attributes(&serialized)?;
    let parsed = parse_writer_svg(&serialized, "writer-style-coverage")?;
    assert_style_attributes(&parsed.to_string(&options))?;
    let first = render_tree(&original)?;
    let second = render_tree(&parsed)?;
    assert_eq!(first.data(), second.data());
    assert!(
        first
            .data()
            .chunks_exact(RGBA_CHANNELS)
            .any(|pixel| pixel[ALPHA_CHANNEL] != 0)
    );
    Ok(())
}

fn assert_style_attributes(svg: &str) -> Result<(), String> {
    for (element, name, value) in [
        ("linearGradient", "spreadMethod", "reflect"),
        ("linearGradient", "spreadMethod", "repeat"),
        ("path", "fill-opacity", "0.5"),
        ("path", "stroke-opacity", "0.5"),
        ("path", "stroke-linecap", "round"),
        ("path", "stroke-linecap", "square"),
        ("path", "stroke-linejoin", "bevel"),
        ("path", "stroke-linejoin", "round"),
        ("path", "stroke-dashoffset", "1"),
        ("path", "paint-order", "stroke"),
    ] {
        assert_attribute(svg, element, name, value)?;
    }
    Ok(())
}

#[rstest]
#[case("dominant-baseline", "middle")]
#[case("dominant-baseline", "central")]
#[case("dominant-baseline", "hanging")]
#[case("dominant-baseline", "mathematical")]
#[case("alignment-baseline", "baseline")]
#[case("alignment-baseline", "before-edge")]
#[case("alignment-baseline", "after-edge")]
#[case("alignment-baseline", "ideographic")]
#[case("alignment-baseline", "alphabetic")]
fn preserved_text_baselines_survive_writer_roundtrip(
    #[case] attribute: &str,
    #[case] value: &str,
) -> Result<(), String> {
    let input = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="60"><text x="12" y="32" font-family="Noto Sans" font-size="18" {attribute}="{value}">Baseline</text></svg>"#
    );
    let original = parse_writer_svg(&input, "writer-text-baseline")?;
    let options = usvg::WriteOptions {
        preserve_text: true,
        ..usvg::WriteOptions::default()
    };
    let serialized = original.to_string(&options);
    assert_attribute(&serialized, "tspan", attribute, value)?;
    let parsed = parse_writer_svg(&serialized, "writer-text-baseline")?;
    assert_pixels_equal(&original, &parsed, "text first write")?;
    let again = parsed.to_string(&options);
    assert_attribute(&again, "tspan", attribute, value)?;
    let reparsed = parse_writer_svg(&again, "writer-text-baseline")?;
    assert_pixels_equal(&parsed, &reparsed, "text second write")?;
    Ok(())
}

#[rstest]
#[case("feComposite/operator=over", "feComposite", "operator", "over")]
#[case("feComposite/operator=in", "feComposite", "operator", "in")]
#[case("feComposite/operator=out", "feComposite", "operator", "out")]
#[case("feComposite/operator=atop", "feComposite", "operator", "atop")]
#[case("feComposite/operator=xor", "feComposite", "operator", "xor")]
#[case("feComposite/operator=lighter", "feComposite", "operator", "lighter")]
#[case("feColorMatrix/type=matrix", "feColorMatrix", "type", "matrix")]
#[case("feColorMatrix/type=saturate", "feColorMatrix", "type", "saturate")]
#[case("feColorMatrix/type=hueRotate", "feColorMatrix", "type", "hueRotate")]
#[case(
    "feConvolveMatrix/edgeMode=none",
    "feConvolveMatrix",
    "edgeMode",
    "none"
)]
#[case(
    "feConvolveMatrix/edgeMode=wrap",
    "feConvolveMatrix",
    "edgeMode",
    "wrap"
)]
#[case(
    "feConvolveMatrix/preserveAlpha=true",
    "feConvolveMatrix",
    "preserveAlpha",
    "true"
)]
#[case(
    "feTurbulence/type=fractalNoise",
    "feTurbulence",
    "type",
    "fractalNoise"
)]
#[case(
    "feTurbulence/stitchTiles=stitch",
    "feTurbulence",
    "stitchTiles",
    "stitch"
)]
fn upstream_filter_variants_preserve_attributes_and_pixels(
    #[case] fixture: &str,
    #[case] element: &str,
    #[case] attribute: &str,
    #[case] value: &str,
) -> Result<(), String> {
    let source = std::fs::read_to_string(Path::new(FILTER_FIXTURES).join(format!("{fixture}.svg")))
        .map_err(|error| error.to_string())?;
    let tree = parse_writer_svg(&source, fixture)?;
    assert!(!tree.filters().is_empty(), "{fixture}: missing filter");
    let options = usvg::WriteOptions::default();
    let serialized = tree.to_string(&options);
    assert_attribute(&serialized, element, attribute, value)?;
    let parsed = parse_writer_svg(&serialized, fixture)?;
    assert_pixels_equal(&tree, &parsed, fixture)?;
    let serialized_again = parsed.to_string(&options);
    assert_attribute(&serialized_again, element, attribute, value)?;
    let reparsed = parse_writer_svg(&serialized_again, fixture)?;
    assert_eq!(
        parsed.filters().len(),
        reparsed.filters().len(),
        "{fixture}"
    );
    assert_pixels_equal(&parsed, &reparsed, fixture)?;
    Ok(())
}

fn assert_pixels_equal(first: &usvg::Tree, second: &usvg::Tree, case: &str) -> Result<(), String> {
    let before = render_tree(first)?;
    let after = render_tree(second)?;
    assert_eq!(before.data(), after.data(), "{case}: roundtrip pixels");
    Ok(())
}

fn assert_attribute(svg: &str, element: &str, attribute: &str, value: &str) -> Result<(), String> {
    let document = usvg::roxmltree::Document::parse(svg).map_err(|error| error.to_string())?;
    assert!(
        document
            .descendants()
            .any(|node| node.has_tag_name(element) && node.attribute(attribute) == Some(value)),
        "{element} must retain {attribute}={value}"
    );
    Ok(())
}
