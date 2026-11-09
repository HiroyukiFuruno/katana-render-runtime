use super::usvg;
use rstest::rstest;
use tiny_skia::{Pixmap, Transform};

const VIEWPORT: u32 = 8;
const CENTER: u32 = VIEWPORT / 2;
const CHANNEL_COUNT: usize = 4;

#[path = "svg_engine_parser_paint_boundary_tests.rs"]
mod paint_boundary_tests;

#[path = "svg_engine_parser_shape_boundary_tests.rs"]
mod shape_boundary_tests;

#[derive(Clone, Copy)]
enum PublicInput {
    Bytes(&'static [u8]),
    Text(&'static str),
}

#[derive(Clone, Copy)]
enum PublicErrorKind {
    InvalidUtf8,
    MalformedGzip,
    ParsingFailed,
    InvalidSize,
}

fn parse_with_languages(source: &str, languages: &[&str]) -> Result<usvg::Tree, String> {
    super::init_test_logger();
    let options = usvg::Options {
        languages: languages
            .iter()
            .map(|language| (*language).to_owned())
            .collect(),
        ..usvg::Options::default()
    };
    usvg::Tree::from_str(source, &options).map_err(|error| error.to_string())
}

fn center_pixel(source: &str, languages: &[&str]) -> Result<[u8; CHANNEL_COUNT], String> {
    let tree = parse_with_languages(source, languages)?;
    let mut pixmap = Pixmap::new(VIEWPORT, VIEWPORT).ok_or("pixmap allocation failed")?;
    super::resvg::render(&tree, Transform::identity(), &mut pixmap.as_mut());
    let start = (CENTER * VIEWPORT + CENTER) as usize * CHANNEL_COUNT;
    pixmap
        .data()
        .get(start..start + CHANNEL_COUNT)
        .ok_or("center pixel is outside the rendered image")?
        .try_into()
        .map_err(|_| "center pixel has an invalid channel count".to_owned())
}

fn sample_path(tree: &usvg::Tree) -> Result<&usvg::Path, String> {
    tree.root()
        .children()
        .iter()
        .find(|node| node.id() == "sample")
        .and_then(|node| match node {
            usvg::Node::Path(path) => Some(path.as_ref()),
            _ => None,
        })
        .ok_or_else(|| "sample SVG path missing".to_owned())
}

#[test]
fn svg_parser_ignores_foreign_namespace_elements_and_prefers_unprefixed_use_href()
-> Result<(), String> {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg"
      xmlns:xlink="http://www.w3.org/1999/xlink" xmlns:foreign="urn:foreign"
      width="8" height="8"><foreign:rect width="8" height="8" fill="#ff0000"/>
      <defs><rect id="legacy" width="8" height="8" fill="#ff0000"/>
      <rect id="modern" width="8" height="8" fill="#0000ff"/></defs>
      <use xlink:href="#legacy" href="#modern"/></svg>"##;
    assert_eq!(center_pixel(source, &[])?, [0, 0, 255, 255]);
    Ok(())
}

#[test]
fn switch_skips_unsupported_features_and_selects_a_supported_child() -> Result<(), String> {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">
      <switch><rect width="8" height="8" requiredFeatures="urn:unsupported" fill="#ff0000"/>
      <rect width="8" height="8" requiredFeatures="http://www.w3.org/TR/SVG11/feature#SVG-static" fill="#00ff00"/>
      <rect width="8" height="8" fill="#0000ff"/></switch></svg>"##;
    assert_eq!(center_pixel(source, &[])?, [0, 255, 0, 255]);
    Ok(())
}

#[test]
fn switch_skips_required_extensions_and_uses_the_next_supported_child() -> Result<(), String> {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">
      <switch><rect width="8" height="8" requiredExtensions="urn:extension" fill="#ff0000"/>
      <rect width="8" height="8" fill="#0000ff"/></switch></svg>"##;
    assert_eq!(center_pixel(source, &[])?, [0, 0, 255, 255]);
    Ok(())
}

#[test]
fn switch_matches_system_language_prefix_and_uses_fallback_when_unmatched() -> Result<(), String> {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">
      <switch><rect width="8" height="8" systemLanguage="en-US" fill="#00ff00"/>
      <rect width="8" height="8" fill="#0000ff"/></switch></svg>"##;
    assert_eq!(center_pixel(source, &["en"])?, [0, 255, 0, 255]);
    assert_eq!(center_pixel(source, &["en-US"])?, [0, 255, 0, 255]);
    assert_eq!(center_pixel(source, &["fr"])?, [0, 0, 255, 255]);
    Ok(())
}

#[rstest]
#[case::rx_sets_ry("rx='4'")]
#[case::ry_sets_rx("ry='4'")]
fn rounded_rectangle_uses_single_authored_radius_on_both_axes(
    #[case] radius: &str,
) -> Result<(), String> {
    let square = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">
      <rect x="4" y="4" width="8" height="8" fill="#ff0000"/></svg>"##;
    let rounded = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='8' height='8'><rect x='4' y='4' width='8' height='8' {radius} fill='#ff0000'/></svg>"
    );

    assert_eq!(center_pixel(square, &[])?, [255, 0, 0, 255]);
    assert_eq!(center_pixel(&rounded, &[])?, [0, 0, 0, 0]);
    Ok(())
}

#[test]
fn shape_conversion_normalizes_radii_and_skips_invalid_geometry() -> Result<(), String> {
    let cases = [
        (
            "<rect id='sample' x='1' y='1' width='6' height='4' rx='99' ry='99' fill='red'/>",
            Some("<rect id='sample' x='1' y='1' width='6' height='4' rx='3' ry='2' fill='red'/>"),
        ),
        ("<rect id='sample' width='0' height='4' fill='red'/>", None),
        ("<rect id='sample' width='4' height='0' fill='red'/>", None),
        ("<polyline id='sample' points='1,1' fill='red'/>", None),
        ("<polygon id='sample' points='bad' fill='red'/>", None),
        ("<circle id='sample' r='0' fill='red'/>", None),
    ];
    let wrap = |shape: &str| {
        format!("<svg xmlns='http://www.w3.org/2000/svg' width='8' height='8'>{shape}</svg>")
    };
    for (shape, expected_shape) in cases {
        let actual = parse_with_languages(&wrap(shape), &[])?;
        if let Some(expected_shape) = expected_shape {
            let expected = parse_with_languages(&wrap(expected_shape), &[])?;
            let actual_segments = sample_path(&actual)?.data().segments();
            let expected_segments = sample_path(&expected)?.data().segments();
            assert!(actual_segments.eq(expected_segments), "{shape}");
        } else {
            assert!(actual.node_by_id("sample").is_none(), "{shape}");
        }
    }
    Ok(())
}

#[test]
fn recursive_use_is_skipped_without_affecting_following_rendered_content() -> Result<(), String> {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">
      <use id="recursive" href="#recursive" fill="#ff0000"/>
      <rect width="8" height="8" fill="#00ff00"/></svg>"##;

    assert_eq!(center_pixel(source, &[])?, [0, 255, 0, 255]);
    Ok(())
}

#[test]
fn flood_filter_replaces_source_color_in_the_filtered_shape() -> Result<(), String> {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">
      <defs><filter id="green" x="0" y="0" width="8" height="8">
        <feFlood flood-color="#00ff00"/></filter></defs>
      <rect width="8" height="8" fill="#ff0000" filter="url(#green)"/></svg>"##;

    assert_eq!(center_pixel(source, &[])?, [0, 255, 0, 255]);
    Ok(())
}

#[test]
fn linked_linear_gradient_inherits_stops_coordinates_and_spread_method() -> Result<(), String> {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg"
      xmlns:xlink="http://www.w3.org/1999/xlink" width="8" height="8">
      <defs><linearGradient id="base" x1="10%" x2="80%" spreadMethod="reflect">
      <stop offset="0" stop-color="#123456"/><stop offset="1" stop-color="#abcdef"/>
      </linearGradient><linearGradient id="alias" xlink:href="#base"/></defs>
      <rect id="sample" width="8" height="8" fill="url(#alias)"/></svg>"##;
    let tree = parse_with_languages(source, &[])?;
    let fill = sample_path(&tree)?
        .fill()
        .ok_or("sample path has no fill")?;
    let gradient = match fill.paint() {
        usvg::Paint::LinearGradient(gradient) => gradient,
        _ => return Err("linked fill is not a linear gradient".to_owned()),
    };
    assert!((gradient.x1() - 0.1).abs() < f32::EPSILON);
    assert!((gradient.x2() - 0.8).abs() < f32::EPSILON);
    assert_eq!(gradient.spread_method(), usvg::SpreadMethod::Reflect);
    assert_eq!(gradient.stops().len(), 2);
    Ok(())
}

#[test]
fn single_stop_current_color_gradient_resolves_to_its_stop_color_and_opacity() -> Result<(), String>
{
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">
      <defs><linearGradient id="single" color="#123456">
      <stop stop-color="currentColor" stop-opacity="0.5"/></linearGradient></defs>
      <rect id="sample" width="8" height="8" fill="url(#single)"/></svg>"##;
    let tree = parse_with_languages(source, &[])?;
    let fill = sample_path(&tree)?
        .fill()
        .ok_or("sample path has no fill")?;
    match fill.paint() {
        usvg::Paint::Color(color) => assert_eq!((color.red, color.green, color.blue), (18, 52, 86)),
        _ => return Err("single-stop gradient was not reduced to a color".to_owned()),
    }
    assert!((fill.opacity().get() - 0.5).abs() < f32::EPSILON);
    Ok(())
}

#[test]
fn linked_pattern_keeps_referenced_children_through_public_tree_parse() -> Result<(), String> {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg"
      xmlns:xlink="http://www.w3.org/1999/xlink" width="8" height="8">
      <defs><pattern id="base" width="4" height="4" patternUnits="userSpaceOnUse">
      <rect width="4" height="4" fill="#123456"/></pattern>
      <pattern id="alias" xlink:href="#base"/></defs>
      <rect id="sample" width="8" height="8" fill="url(#alias)"/></svg>"##;
    let tree = parse_with_languages(source, &[])?;
    let fill = sample_path(&tree)?
        .fill()
        .ok_or("sample path has no fill")?;
    match fill.paint() {
        usvg::Paint::Pattern(pattern) => assert!(pattern.root().has_children()),
        _ => return Err("linked fill is not a pattern".to_owned()),
    }
    Ok(())
}

#[rstest]
#[case::invalid_utf8(
    "invalid UTF-8 bytes",
    PublicInput::Bytes(&[0xff]),
    PublicErrorKind::InvalidUtf8
)]
#[case::malformed_gzip(
    "truncated gzip header",
    PublicInput::Bytes(&[0x1f, 0x8b, 0x08, 0x00]),
    PublicErrorKind::MalformedGzip
)]
#[case::malformed_xml(
    "mismatched XML tags",
    PublicInput::Text("<svg><g></svg>"),
    PublicErrorKind::ParsingFailed
)]
#[case::non_svg_root(
    "well-formed non-SVG root",
    PublicInput::Text("<g xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"8\"/>"),
    PublicErrorKind::ParsingFailed
)]
#[case::foreign_namespace_root(
    "foreign namespace SVG root",
    PublicInput::Text("<svg xmlns=\"urn:foreign\" width=\"8\" height=\"8\"/>"),
    PublicErrorKind::ParsingFailed
)]
#[case::zero_width(
    "zero SVG width",
    PublicInput::Text("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"0\" height=\"8\"/>"),
    PublicErrorKind::InvalidSize
)]
fn public_parser_returns_expected_error_for_invalid_inputs(
    #[case] label: &str,
    #[case] input: PublicInput,
    #[case] expected: PublicErrorKind,
) -> Result<(), String> {
    let error = match input {
        PublicInput::Bytes(bytes) => usvg::Tree::from_data(bytes, &usvg::Options::default()),
        PublicInput::Text(text) => usvg::Tree::from_str(text, &usvg::Options::default()),
    }
    .err()
    .ok_or_else(|| format!("{label}: invalid input was accepted"))?;
    error_kind_matches(expected, &error)
        .then_some(())
        .ok_or_else(|| format!("{label}: unexpected parser error: {error}"))
}

fn error_kind_matches(expected: PublicErrorKind, error: &usvg::Error) -> bool {
    match expected {
        PublicErrorKind::InvalidUtf8 => matches!(error, usvg::Error::NotAnUtf8Str),
        PublicErrorKind::MalformedGzip => {
            matches!(error, usvg::Error::MalformedGZip)
                && error.to_string() == "provided data has a malformed GZip content"
        }
        PublicErrorKind::ParsingFailed => matches!(error, usvg::Error::ParsingFailed(_)),
        PublicErrorKind::InvalidSize => {
            matches!(error, usvg::Error::InvalidSize)
                && error.to_string() == "SVG has an invalid size"
        }
    }
}
