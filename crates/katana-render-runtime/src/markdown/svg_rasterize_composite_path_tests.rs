use super::super::super::html_glyph::HtmlGlyphRasterizer;
use super::super::{AuthoredClipInfo, SvgCompositeRenderer};
use resvg::usvg;
use tiny_skia::{Pixmap, Transform};

#[path = "svg_rasterize_composite_path_dash_tests.rs"]
mod dash;
#[path = "svg_rasterize_composite_fill_only_tests.rs"]
mod fill_only;
#[path = "svg_rasterize_composite_path_opacity_tests.rs"]
mod opacity;

const SIZE: u32 = 64;
const TRANSLATE_X: f32 = 20.0;
const TRANSLATE_Y: f32 = 30.0;
const EVEN_ODD_PATH: &str = "M4 4H60V60H4Z M20 20H44V44H20Z";
const SQUARE_PATH: &str = "M12 12H52V52H12Z";
const OUTER_FILL_SAMPLE: (u32, u32) = (10, 10);
const EVEN_ODD_HOLE_SAMPLE: (u32, u32) = (30, 30);

#[test]
fn opaque_fill_rule_is_preserved_with_and_without_stroke() -> Result<(), String> {
    for attributes in [
        "fill=\"#ff0000\" fill-rule=\"evenodd\" stroke=\"#0000ff\" stroke-width=\"2\"",
        "fill=\"#ff0000\" fill-rule=\"evenodd\"",
    ] {
        assert_fill_rule_pixels(attributes)?;
    }
    Ok(())
}

fn assert_fill_rule_pixels(attributes: &str) -> Result<(), String> {
    let rendered = render_path(EVEN_ODD_PATH, attributes, Transform::identity())?;
    let outer = rendered
        .pixel(OUTER_FILL_SAMPLE.0, OUTER_FILL_SAMPLE.1)
        .ok_or("missing filled path pixel")?;
    let hole = rendered
        .pixel(EVEN_ODD_HOLE_SAMPLE.0, EVEN_ODD_HOLE_SAMPLE.1)
        .ok_or("missing even-odd hole pixel")?;
    assert_eq!(outer.alpha(), 255);
    assert_eq!(outer.red(), 255);
    assert_eq!(outer.blue(), 0);
    assert_eq!(hole.alpha(), 0);
    Ok(())
}

#[test]
fn caller_transform_is_preserved_with_and_without_stroke() -> Result<(), String> {
    for attributes in [
        "fill=\"#00ff00\" stroke=\"#000000\" stroke-width=\"1\"",
        "fill=\"#00ff00\"",
    ] {
        assert_transformed_fill_pixels(attributes)?;
    }
    Ok(())
}

fn assert_transformed_fill_pixels(attributes: &str) -> Result<(), String> {
    let rendered = render_path(
        "M2 2H12V12H2Z",
        attributes,
        Transform::from_translate(TRANSLATE_X, TRANSLATE_Y),
    )?;
    assert!(is_visible(&rendered, 25, 35));
    assert!(!is_visible(&rendered, 5, 5));
    Ok(())
}

#[test]
fn stroke_then_fill_order_changes_the_overlap_pixel() -> Result<(), String> {
    let fill_then_stroke = render_path(
        SQUARE_PATH,
        "fill=\"#ff0000\" stroke=\"#0000ff\" stroke-width=\"12\" paint-order=\"fill stroke\"",
        Transform::identity(),
    )?;
    let stroke_then_fill = render_path(
        SQUARE_PATH,
        "fill=\"#ff0000\" stroke=\"#0000ff\" stroke-width=\"12\" paint-order=\"stroke fill\"",
        Transform::identity(),
    )?;
    let first = fill_then_stroke
        .pixel(14, 20)
        .ok_or("missing first overlap pixel")?;
    let second = stroke_then_fill
        .pixel(14, 20)
        .ok_or("missing second overlap pixel")?;
    assert!(first.blue() > first.red());
    assert!(second.red() > second.blue());
    Ok(())
}

#[test]
fn stroke_caps_keep_their_distinct_geometry() -> Result<(), String> {
    for (cap, outer_visible, near_visible) in [
        ("square", true, true),
        ("round", false, true),
        ("butt", false, false),
    ] {
        let rendered = render_path(
            "M16 20H48",
            &format!(
                "fill=\"none\" stroke=\"#ffffff\" stroke-width=\"8\" stroke-linecap=\"{cap}\" shape-rendering=\"crispEdges\""
            ),
            Transform::identity(),
        )?;
        assert_eq!(is_visible(&rendered, 12, 16), outer_visible);
        assert_eq!(is_visible(&rendered, 13, 20), near_visible);
    }
    Ok(())
}

#[test]
fn stroke_joins_and_miter_limit_keep_their_distinct_geometry() -> Result<(), String> {
    for (join, miter_limit, tip_visible, shoulder_visible) in [
        ("miter", "3", true, true),
        ("miter", "1", false, false),
        ("round", "3", false, true),
        ("bevel", "3", false, false),
    ] {
        let rendered = render_path(
            "M16 48L32 16L48 48",
            &format!(
                "fill=\"none\" stroke=\"#ffffff\" stroke-width=\"8\" stroke-linejoin=\"{join}\" stroke-miterlimit=\"{miter_limit}\""
            ),
            Transform::identity(),
        )?;
        assert_eq!(is_visible(&rendered, 32, 9), tip_visible);
        assert_eq!(is_visible(&rendered, 32, 13), shoulder_visible);
    }
    Ok(())
}

#[test]
fn unsupported_styles_keep_the_existing_resvg_pixels() -> Result<(), String> {
    for attributes in [
        "fill=\"url(#g)\"",
        "fill=\"none\" stroke=\"#00ff00\" stroke-width=\"12\" stroke-linejoin=\"miter-clip\"",
        "fill=\"#ff0000\" fill-opacity=\"0.5\" stroke=\"url(#g)\" stroke-width=\"4\"",
        "fill=\"url(#g)\" stroke=\"#00ff00\" stroke-width=\"4\"",
        "fill=\"none\" stroke=\"none\"",
        "fill=\"#ff0000\" fill-opacity=\"0.5\" stroke=\"#0000ff\" stroke-width=\"4\"",
        "fill=\"#ff0000\" stroke=\"#0000ff\" stroke-opacity=\"0.5\" stroke-width=\"4\"",
    ] {
        assert_matches_existing_resvg("M5 5H58V58H5Z", attributes)?;
    }
    Ok(())
}

#[test]
fn paths_without_html_rasterizer_keep_the_existing_resvg_route() -> Result<(), String> {
    let svg = svg_with_path(SQUARE_PATH, "fill=\"#ff0000\" fill-opacity=\"0.5\"");
    let (tree, mut expected) = parse(&svg)?;
    let mut actual = allocate_pixmap()?;
    resvg::render(&tree, Transform::identity(), &mut expected.as_mut());
    SvgCompositeRenderer::render_with_text(
        &tree,
        Transform::identity(),
        &mut actual.as_mut(),
        &AuthoredClipInfo::default(),
        None,
    )?;
    assert_eq!(actual.data(), expected.data());
    Ok(())
}

fn render_path(path: &str, attributes: &str, transform: Transform) -> Result<Pixmap, String> {
    let svg = svg_with_path(path, attributes);
    let (tree, mut pixmap) = parse(&svg)?;
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    SvgCompositeRenderer::render_with_text(
        &tree,
        transform,
        &mut pixmap.as_mut(),
        &AuthoredClipInfo::default(),
        Some(&rasterizer),
    )?;
    Ok(pixmap)
}

fn assert_matches_existing_resvg(path: &str, attributes: &str) -> Result<(), String> {
    let svg = svg_with_path(path, attributes);
    let (tree, mut expected) = parse(&svg)?;
    let mut actual = allocate_pixmap()?;
    resvg::render(&tree, Transform::identity(), &mut expected.as_mut());
    let rasterizer = HtmlGlyphRasterizer::new(tree.fontdb());
    SvgCompositeRenderer::render_with_text(
        &tree,
        Transform::identity(),
        &mut actual.as_mut(),
        &AuthoredClipInfo::default(),
        Some(&rasterizer),
    )?;
    assert_eq!(actual.data(), expected.data());
    Ok(())
}

fn is_visible(pixmap: &Pixmap, x: u32, y: u32) -> bool {
    pixmap.pixel(x, y).is_some_and(|pixel| pixel.alpha() > 0)
}

fn svg_with_path(path: &str, attributes: &str) -> String {
    let path_markup = format!("<path d=\"{path}\" {attributes}/>");
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{SIZE}\" height=\"{SIZE}\"><defs><linearGradient id=\"g\"><stop stop-color=\"red\"/><stop offset=\"1\" stop-color=\"blue\"/></linearGradient></defs>{path_markup}</svg>"
    )
}

fn parse(svg: &str) -> Result<(usvg::Tree, Pixmap), String> {
    let tree =
        usvg::Tree::from_str(svg, &usvg::Options::default()).map_err(|error| error.to_string())?;
    Ok((tree, allocate_pixmap()?))
}

fn allocate_pixmap() -> Result<Pixmap, String> {
    Pixmap::new(SIZE, SIZE).ok_or_else(|| "failed to allocate path test pixmap".to_owned())
}
