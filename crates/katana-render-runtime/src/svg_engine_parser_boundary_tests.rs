use super::usvg;
use rstest::rstest;
use tiny_skia::{Pixmap, Transform};

const VIEWPORT: u32 = 8;
const CENTER: u32 = VIEWPORT / 2;
const CHANNEL_COUNT: usize = 4;

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
    let result = match input {
        PublicInput::Bytes(bytes) => usvg::Tree::from_data(bytes, &usvg::Options::default()),
        PublicInput::Text(text) => usvg::Tree::from_str(text, &usvg::Options::default()),
    };
    let error = match result {
        Ok(_) => return Err(format!("{label}: invalid input was accepted")),
        Err(error) => error,
    };
    assert_error_kind(label, expected, &error)
}

fn assert_error_kind(
    label: &str,
    expected: PublicErrorKind,
    error: &usvg::Error,
) -> Result<(), String> {
    let matches = match expected {
        PublicErrorKind::InvalidUtf8 => matches!(error, usvg::Error::NotAnUtf8Str),
        PublicErrorKind::MalformedGzip => matches!(error, usvg::Error::MalformedGZip),
        PublicErrorKind::ParsingFailed => matches!(error, usvg::Error::ParsingFailed(_)),
        PublicErrorKind::InvalidSize => matches!(error, usvg::Error::InvalidSize),
    };
    if !matches {
        return Err(format!("{label}: unexpected parser error: {error}"));
    }
    Ok(())
}
