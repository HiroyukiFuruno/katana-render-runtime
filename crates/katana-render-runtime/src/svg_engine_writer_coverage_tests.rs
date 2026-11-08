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
    let first = assert_pixels_equal(&original, &parsed, "writer-style-coverage")?;
    assert!(
        first
            .data()
            .chunks_exact(RGBA_CHANNELS)
            .any(|pixel| pixel[ALPHA_CHANNEL] != 0)
    );
    Ok(())
}

#[test]
fn clip_fill_mask_and_stroke_boundaries_survive_writer_roundtrip() -> Result<(), String> {
    let input = r##"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="40">
      <defs>
        <clipPath id="inner"><path d="M2 2 H30 V30 H2 Z" clip-rule="evenodd"/></clipPath>
        <mask id="alpha" mask-type="alpha" maskUnits="userSpaceOnUse" x="0" y="0" width="40" height="40">
          <rect width="40" height="40" fill="white"/>
        </mask>
        <mask id="outer" mask="url(#alpha)" maskUnits="userSpaceOnUse" x="0" y="0" width="40" height="40">
          <rect width="40" height="40" fill="white"/>
        </mask>
      </defs>
      <path d="M2 2 H30 V30 H2 Z M8 8 V24 H24 V8 Z" fill-rule="evenodd"
        clip-path="url(#inner)" mask="url(#outer)" stroke="black"
        stroke-miterlimit="7" stroke-linejoin="miter-clip"/>
    </svg>"##;
    let original = parse_writer_svg(input, "writer-clip-mask-stroke-boundaries")?;
    let options = usvg::WriteOptions::default();
    let serialized = original.to_string(&options);
    assert_boundary_attributes(&serialized)?;
    let parsed = parse_writer_svg(&serialized, "writer-clip-mask-stroke-boundaries")?;
    assert_boundary_attributes(&parsed.to_string(&options))?;
    assert_pixels_equal(&original, &parsed, "clip-mask-stroke boundaries")?;
    Ok(())
}

fn assert_boundary_attributes(svg: &str) -> Result<(), String> {
    for (element, name, value) in [
        ("path", "fill-rule", "evenodd"),
        ("path", "clip-rule", "evenodd"),
        ("mask", "mask-type", "alpha"),
        ("mask", "mask", "url(#alpha)"),
        ("path", "stroke-miterlimit", "7"),
        ("path", "stroke-linejoin", "miter-clip"),
    ] {
        assert_attribute(svg, element, name, value)?;
    }
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

pub(super) fn assert_pixels_equal(
    first: &usvg::Tree,
    second: &usvg::Tree,
    case: &str,
) -> Result<tiny_skia::Pixmap, String> {
    let (before, after) = render_pair(first, second)?;
    assert_pixmaps_equal(&before, &after, case);
    Ok(before)
}

pub(super) fn render_pair(
    first: &usvg::Tree,
    second: &usvg::Tree,
) -> Result<(tiny_skia::Pixmap, tiny_skia::Pixmap), String> {
    Ok((render_tree(first)?, render_tree(second)?))
}

pub(super) fn assert_pixmaps_equal(
    before: &tiny_skia::Pixmap,
    after: &tiny_skia::Pixmap,
    case: &str,
) {
    let first_difference = before
        .data()
        .chunks_exact(RGBA_CHANNELS)
        .zip(after.data().chunks_exact(RGBA_CHANNELS))
        .position(|(left, right)| left != right);
    assert!(
        before.width() == after.width()
            && before.height() == after.height()
            && before.data() == after.data(),
        "{case}: roundtrip {}x{} vs {}x{}; first differing pixel index {first_difference:?}",
        before.width(),
        before.height(),
        after.width(),
        after.height()
    );
}

pub(super) fn assert_attribute(
    svg: &str,
    element: &str,
    attribute: &str,
    value: &str,
) -> Result<(), String> {
    let document = usvg::roxmltree::Document::parse(svg).map_err(|error| error.to_string())?;
    assert!(
        document
            .descendants()
            .any(|node| node.has_tag_name(element) && node.attribute(attribute) == Some(value)),
        "{element} must retain {attribute}={value}"
    );
    Ok(())
}

#[test]
fn nested_clip_reference_survives_writer_roundtrip() -> Result<(), String> {
    let input = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20">
      <defs>
        <clipPath id="inner"><rect x="2" y="2" width="12" height="12"/></clipPath>
        <clipPath id="outer" clip-path="url(#inner)"><rect width="18" height="18"/></clipPath>
      </defs>
      <rect width="20" height="20" fill="red" clip-path="url(#outer)"/>
    </svg>"##;
    let original = parse_writer_svg(input, "writer-nested-clip")?;
    let serialized = original.to_string(&usvg::WriteOptions::default());
    assert_attribute(&serialized, "clipPath", "clip-path", "url(#inner)")?;
    let reparsed = parse_writer_svg(&serialized, "writer-nested-clip-roundtrip")?;
    assert_pixels_equal(&original, &reparsed, "nested clip")?;
    Ok(())
}

#[rstest]
#[case::blend(
    "mix-blend-mode:multiply",
    false,
    "mix-blend-mode:multiply;isolation:auto"
)]
#[case::isolation("isolation:isolate", true, "mix-blend-mode:normal;isolation:isolate")]
fn group_blending_survives_writer_roundtrip(
    #[case] style: &str,
    #[case] isolate: bool,
    #[case] expected: &str,
) -> Result<(), String> {
    let input = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="white"/><g id="holder" style="{style}"><rect width="8" height="8" fill="red"/></g></svg>"#
    );
    let original = parse_writer_svg(&input, "writer-group-blending")?;
    let serialized = original.to_string(&usvg::WriteOptions::default());
    assert_attribute(&serialized, "g", "style", expected)?;
    let reparsed = parse_writer_svg(&serialized, "writer-group-blending-roundtrip")?;
    let Some(usvg::Node::Group(group)) = reparsed.node_by_id("holder") else {
        return Err("blending group missing".to_owned());
    };
    assert_eq!(group.isolate(), isolate);
    assert_pixels_equal(&original, &reparsed, "group blending")?;
    Ok(())
}
