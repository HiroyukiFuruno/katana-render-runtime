use super::{CHANNEL_COUNT, VIEWPORT, parse_with_languages, sample_path, usvg};
use rstest::rstest;

#[path = "svg_engine_parser_paint_additional_boundary_tests.rs"]
mod additional_boundaries;

const OPAQUE_ALPHA: f32 = 1.0;
const GRADIENT_FALLBACK_COLOR: (u8, u8, u8) = (18, 52, 86);
const PATTERN_FALLBACK_COLOR: (u8, u8, u8) = (171, 205, 239);
const EMPTY_RADIAL_FALLBACK_COLOR: (u8, u8, u8) = (171, 205, 239);
const VALID_STOP_COLOR: (u8, u8, u8) = (18, 52, 86);
const INVALID_PATTERN_SIZE_FALLBACK: (u8, u8, u8) = (34, 68, 102);
const LINEAR_START_SAMPLE_X: usize = 0;
const LINEAR_END_SAMPLE_X: usize = 1;
const RADIAL_CENTER_SAMPLE_X: usize = 3;
const RADIAL_EDGE_SAMPLE_X: usize = 2;
const PATTERN_FIRST_SAMPLE_X: usize = 5;
const PATTERN_NEXT_SAMPLE_X: usize = 6;
const PAINT_SAMPLE_Y: usize = 3;
const PATTERN_SAMPLE_Y: usize = 1;
const INVALID_PAINT_SERVER_REFERENCES_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="8" height="8"><defs><linearGradient id="valid-gradient"><stop stop-color="#000"/><stop offset="1" stop-color="#fff"/></linearGradient><pattern id="valid-pattern" width="2" height="2"><rect width="2" height="2"/></pattern><linearGradient id="bad-gradient" xlink:href="#valid-pattern"/><pattern id="bad-pattern" width="2" height="2" xlink:href="#valid-gradient"/></defs><rect id="gradient-user" width="4" height="8" fill="url(#bad-gradient) #123456"/><rect id="pattern-user" x="4" width="4" height="8" fill="url(#bad-pattern) #abcdef"/></svg>"##;
const EMPTY_PATTERN_WITH_VIEWBOX_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><defs><pattern id="empty" width="4" height="4" patternUnits="userSpaceOnUse" patternContentUnits="userSpaceOnUse" viewBox="0 0 4 4"><g display="none"><rect width="4" height="4"/></g></pattern></defs><rect width="8" height="8" fill="url(#empty)"/></svg>"#;
const EMPTY_PATTERN_WITHOUT_VIEWBOX_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><defs><pattern id="empty" width="4" height="4" patternUnits="userSpaceOnUse" patternContentUnits="userSpaceOnUse"><g display="none"><rect width="4" height="4"/></g></pattern></defs><rect width="8" height="8" fill="url(#empty)"/></svg>"#;
const INVALID_GRADIENT_STOP_COLOR_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><defs><linearGradient id="gradient"><stop offset="0" stop-color="not-a-color"/><stop offset="1" stop-color="#123456"/></linearGradient></defs><rect id="sample" width="8" height="8" fill="url(#gradient)"/></svg>"##;
const INVALID_PATTERN_SIZE_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><defs><pattern id="zero" width="0" height="4"><rect width="4" height="4"/></pattern></defs><rect id="sample" width="8" height="8" fill="url(#zero) #224466"/></svg>"##;
const PATTERN_WRONG_TYPE_LINK_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="8" height="8"><defs><linearGradient id="gradient"><stop stop-color="#00f"/><stop offset="1" stop-color="#0ff"/></linearGradient><pattern id="pattern" width="4" height="4" patternUnits="userSpaceOnUse" xlink:href="#gradient"><rect width="2" height="4" fill="#ff0000"/></pattern></defs><rect id="sample" width="8" height="8" fill="url(#pattern)"/></svg>"##;
const PATTERN_WITHOUT_LINK_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><defs><pattern id="pattern" width="4" height="4" patternUnits="userSpaceOnUse"><rect width="2" height="4" fill="#ff0000"/></pattern></defs><rect id="sample" width="8" height="8" fill="url(#pattern)"/></svg>"##;

const LINEAR_RADIAL_PATTERN_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">
      <defs>
        <linearGradient id="linear" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="2" y2="0">
          <stop stop-color="#ff0000"/><stop offset="1" stop-color="#0000ff"/>
        </linearGradient>
        <radialGradient id="radial" gradientUnits="userSpaceOnUse" cx="3.5" cy="3.5" r="1.5">
          <stop stop-color="#ff0000"/><stop offset="1" stop-color="#0000ff"/>
        </radialGradient>
        <pattern id="pattern" patternUnits="userSpaceOnUse" patternContentUnits="userSpaceOnUse" width="2" height="2">
          <rect width="1" height="2" fill="#00ff00"/>
        </pattern>
      </defs>
      <rect id="linear-shape" width="2" height="8" fill="url(#linear)"/>
      <rect id="radial-shape" x="2" width="3" height="8" fill="url(#radial)"/>
      <rect id="pattern-shape" x="5" width="3" height="8" fill="url(#pattern)"/>
    </svg>"##;

fn path_by_id<'a>(tree: &'a usvg::Tree, id: &str) -> Result<&'a usvg::Path, String> {
    tree.root()
        .children()
        .iter()
        .find(|node| node.id() == id)
        .and_then(|node| match node {
            usvg::Node::Path(path) => Some(path.as_ref()),
            _ => None,
        })
        .ok_or_else(|| format!("SVG path {id} missing"))
}

fn assert_solid_fill_color(
    tree: &usvg::Tree,
    id: &str,
    expected: (u8, u8, u8),
) -> Result<(), String> {
    let fill = path_by_id(tree, id)?
        .fill()
        .ok_or_else(|| format!("{id} has no fallback fill"))?;
    let usvg::Paint::Color(color) = fill.paint() else {
        return Err(format!("{id} fallback is not a solid color"));
    };
    assert_eq!((color.red, color.green, color.blue), expected);
    assert_eq!(fill.opacity().get(), OPAQUE_ALPHA);
    Ok(())
}

fn assert_all_pixels_transparent(source: &str) -> Result<(), String> {
    let tree = parse_with_languages(source, &[])?;
    let mut pixmap = tiny_skia::Pixmap::new(VIEWPORT, VIEWPORT).ok_or("pixmap unavailable")?;
    super::super::resvg::render(
        &tree,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    if pixmap.pixels().iter().any(|pixel| pixel.alpha() != 0) {
        return Err("empty pattern painted a pixel".to_owned());
    }
    Ok(())
}

#[test]
fn invalid_cross_type_paint_server_links_use_explicit_fallback_colors() -> Result<(), String> {
    let tree = parse_with_languages(INVALID_PAINT_SERVER_REFERENCES_SVG, &[])?;
    assert_solid_fill_color(&tree, "gradient-user", GRADIENT_FALLBACK_COLOR)?;
    assert_solid_fill_color(&tree, "pattern-user", PATTERN_FALLBACK_COLOR)
}

#[rstest]
#[case::with_viewbox(EMPTY_PATTERN_WITH_VIEWBOX_SVG)]
#[case::without_viewbox(EMPTY_PATTERN_WITHOUT_VIEWBOX_SVG)]
fn effectively_empty_pattern_paints_no_pixels(#[case] source: &str) -> Result<(), String> {
    assert_all_pixels_transparent(source)
}

#[test]
fn invalid_gradient_stop_color_uses_initial_black_without_losing_valid_stop() -> Result<(), String>
{
    let tree = parse_with_languages(INVALID_GRADIENT_STOP_COLOR_SVG, &[])?;
    let fill = sample_path(&tree)?.fill().ok_or("gradient fill missing")?;
    let usvg::Paint::LinearGradient(gradient) = fill.paint() else {
        return Err("invalid stop removed the gradient".to_owned());
    };
    let [invalid, valid] = gradient.stops() else {
        return Err("gradient stops were not preserved".to_owned());
    };
    assert_eq!(invalid.color(), usvg::Color::black());
    assert_eq!(
        (valid.color().red, valid.color().green, valid.color().blue),
        VALID_STOP_COLOR
    );
    Ok(())
}

#[test]
fn non_stop_gradient_child_does_not_change_valid_stops_or_paint() -> Result<(), String> {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">
      <defs><linearGradient id="gradient"><stop stop-color="red"/><rect width="8" height="8" fill="lime"/><stop offset="1" stop-color="blue"/></linearGradient></defs>
      <rect id="sample" width="8" height="8" fill="url(#gradient)"/></svg>"##;
    let control = source.replace("<rect width=\"8\" height=\"8\" fill=\"lime\"/>", "");
    assert_ne!(source, control);
    let actual = rendered_svg(source)?;
    let expected = rendered_svg(&control)?;
    assert!(expected.iter().any(|channel| *channel != 0));
    assert_eq!(actual, expected);
    Ok(())
}

#[test]
fn invalid_pattern_size_uses_the_authored_fallback_color() -> Result<(), String> {
    let tree = parse_with_languages(INVALID_PATTERN_SIZE_SVG, &[])?;
    assert_solid_fill_color(&tree, "sample", INVALID_PATTERN_SIZE_FALLBACK)
}

#[test]
fn pattern_with_wrong_type_href_keeps_its_own_children() -> Result<(), String> {
    let linked = rendered_svg(PATTERN_WRONG_TYPE_LINK_SVG)?;
    let control = rendered_svg(PATTERN_WITHOUT_LINK_SVG)?;
    assert!(control.iter().any(|channel| *channel != 0));
    assert_eq!(linked, control);
    Ok(())
}

#[test]
fn gradient_child_fallback_and_zero_bbox_paints_are_explicit() -> Result<(), String> {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">
      <defs>
        <linearGradient id="one"><title>metadata</title><stop stop-color="#123456"/><stop offset="1" stop-color="#654321"/></linearGradient>
        <linearGradient id="empty-linear"/><radialGradient id="empty-radial"/>
        <pattern id="pattern" width="0.5" height="0.5"><rect width="1" height="1" fill="red"/></pattern>
      </defs>
      <rect id="linear-fallback" width="2" height="8" fill="url(#empty-linear) #123456"/>
      <rect id="radial-fallback" x="2" width="2" height="8" fill="url(#empty-radial) #abcdef"/>
      <path id="gradient-line" d="M5 1L5 7" fill="url(#one)"/>
      <path id="pattern-line" d="M7 1L7 7" fill="url(#pattern)"/>
    </svg>"##;
    let tree = parse_with_languages(source, &[])?;
    assert_solid_fill_color(&tree, "linear-fallback", GRADIENT_FALLBACK_COLOR)?;
    assert_solid_fill_color(&tree, "radial-fallback", EMPTY_RADIAL_FALLBACK_COLOR)?;
    for id in ["gradient-line", "pattern-line"] {
        let Some(usvg::Node::Path(path)) = tree.node_by_id(id) else {
            return Err(format!("zero-width shape {id} missing"));
        };
        assert!(path.fill().is_none(), "paint on zero-width shape {id}");
    }
    Ok(())
}

#[test]
fn linear_radial_and_pattern_fills_keep_their_paint_kinds() -> Result<(), String> {
    let tree = parse_with_languages(LINEAR_RADIAL_PATTERN_SVG, &[])?;
    for (id, kind) in [
        ("linear-shape", "linear"),
        ("radial-shape", "radial"),
        ("pattern-shape", "pattern"),
    ] {
        assert_paint_kind(&tree, id, kind)?;
    }
    Ok(())
}

fn assert_paint_kind(tree: &usvg::Tree, id: &str, kind: &str) -> Result<(), String> {
    let Some(usvg::Node::Path(path)) = tree.node_by_id(id) else {
        return Err(format!("paint source shape {id} missing"));
    };
    let paint = path
        .fill()
        .ok_or_else(|| format!("fill missing for {id}"))?
        .paint();
    let matches = match kind {
        "linear" => matches!(paint, usvg::Paint::LinearGradient(_)),
        "radial" => matches!(paint, usvg::Paint::RadialGradient(_)),
        "pattern" => matches!(paint, usvg::Paint::Pattern(_)),
        _ => false,
    };
    assert!(matches, "unexpected paint kind for {id}");
    Ok(())
}

#[test]
fn linear_radial_and_pattern_fills_produce_expected_pixels() -> Result<(), String> {
    let pixels = rendered_svg(LINEAR_RADIAL_PATTERN_SVG)?;
    assert!(
        pixel(&pixels, LINEAR_START_SAMPLE_X, PAINT_SAMPLE_Y)?[0]
            > pixel(&pixels, LINEAR_START_SAMPLE_X, PAINT_SAMPLE_Y)?[2]
    );
    assert!(
        pixel(&pixels, LINEAR_END_SAMPLE_X, PAINT_SAMPLE_Y)?[2]
            > pixel(&pixels, LINEAR_END_SAMPLE_X, PAINT_SAMPLE_Y)?[0]
    );
    assert!(
        pixel(&pixels, RADIAL_CENTER_SAMPLE_X, PAINT_SAMPLE_Y)?[0]
            > pixel(&pixels, RADIAL_CENTER_SAMPLE_X, PAINT_SAMPLE_Y)?[2]
    );
    assert!(
        pixel(&pixels, RADIAL_EDGE_SAMPLE_X, PAINT_SAMPLE_Y)?[2]
            > pixel(&pixels, RADIAL_EDGE_SAMPLE_X, PAINT_SAMPLE_Y)?[0]
    );
    assert_pattern_pixels(&pixels)
}

fn assert_pattern_pixels(pixels: &[u8]) -> Result<(), String> {
    let first = pixel(pixels, PATTERN_FIRST_SAMPLE_X, PATTERN_SAMPLE_Y)?;
    let next = pixel(pixels, PATTERN_NEXT_SAMPLE_X, PATTERN_SAMPLE_Y)?;
    assert_ne!(first, next);
    assert!((first[1] > 0 && first[3] > 0) || (next[1] > 0 && next[3] > 0));
    Ok(())
}

fn pixel(bytes: &[u8], x: usize, y: usize) -> Result<[u8; CHANNEL_COUNT], String> {
    let start = (y * VIEWPORT as usize + x) * CHANNEL_COUNT;
    bytes
        .get(start..start + CHANNEL_COUNT)
        .ok_or_else(|| "rendered pixel missing".to_owned())?
        .try_into()
        .map_err(|_| "rendered pixel channel count changed".to_owned())
}

fn rendered_svg(source: &str) -> Result<Vec<u8>, String> {
    let tree = parse_with_languages(source, &[])?;
    let mut pixmap = tiny_skia::Pixmap::new(VIEWPORT, VIEWPORT).ok_or("pixmap unavailable")?;
    super::super::resvg::render(
        &tree,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    Ok(pixmap.data().to_vec())
}
