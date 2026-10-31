use super::{VIEWPORT, parse_with_languages, sample_path, usvg};
use rstest::rstest;

const OPAQUE_ALPHA: f32 = 1.0;
const GRADIENT_FALLBACK_COLOR: (u8, u8, u8) = (18, 52, 86);
const PATTERN_FALLBACK_COLOR: (u8, u8, u8) = (171, 205, 239);
const VALID_STOP_COLOR: (u8, u8, u8) = (18, 52, 86);
const INVALID_PAINT_SERVER_REFERENCES_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="8" height="8"><defs><linearGradient id="valid-gradient"><stop stop-color="#000"/><stop offset="1" stop-color="#fff"/></linearGradient><pattern id="valid-pattern" width="2" height="2"><rect width="2" height="2"/></pattern><linearGradient id="bad-gradient" xlink:href="#valid-pattern"/><pattern id="bad-pattern" width="2" height="2" xlink:href="#valid-gradient"/></defs><rect id="gradient-user" width="4" height="8" fill="url(#bad-gradient) #123456"/><rect id="pattern-user" x="4" width="4" height="8" fill="url(#bad-pattern) #abcdef"/></svg>"##;
const EMPTY_PATTERN_WITH_VIEWBOX_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><defs><pattern id="empty" width="4" height="4" patternUnits="userSpaceOnUse" patternContentUnits="userSpaceOnUse" viewBox="0 0 4 4"><g display="none"><rect width="4" height="4"/></g></pattern></defs><rect width="8" height="8" fill="url(#empty)"/></svg>"#;
const EMPTY_PATTERN_WITHOUT_VIEWBOX_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><defs><pattern id="empty" width="4" height="4" patternUnits="userSpaceOnUse" patternContentUnits="userSpaceOnUse"><g display="none"><rect width="4" height="4"/></g></pattern></defs><rect width="8" height="8" fill="url(#empty)"/></svg>"#;
const INVALID_GRADIENT_STOP_COLOR_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><defs><linearGradient id="gradient"><stop offset="0" stop-color="not-a-color"/><stop offset="1" stop-color="#123456"/></linearGradient></defs><rect id="sample" width="8" height="8" fill="url(#gradient)"/></svg>"##;

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
