use super::{Mode, compare, parse_source, render, usvg};
use std::path::Path;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/svg-engine/upstream/resvg/tests/tests/filters/feMorphology/radius=10-0.svg"
);
const SVG_FILTER: &str = "<filter id=\"filter1\">";

#[test]
fn opaque_morphology_is_independent_of_filter_color_space() -> Result<(), String> {
    let source = std::fs::read_to_string(FIXTURE).map_err(|error| error.to_string())?;
    assert_opaque_fixture(&source)?;
    assert_eq!(source.matches(SVG_FILTER).count(), 1);
    let srgb = source.replacen(
        SVG_FILTER,
        "<filter id=\"filter1\" color-interpolation-filters=\"sRGB\">",
        1,
    );
    let original = render_source(&source, usvg::filter::ColorInterpolation::LinearRGB)?;
    let alternative = render_source(&srgb, usvg::filter::ColorInterpolation::SRGB)?;
    let png = alternative
        .encode_png()
        .map_err(|error| error.to_string())?;
    let different_pixels = compare::pixel_differences(&original, &png)?;
    assert_eq!(different_pixels, 0, "color-space differing pixels");
    Ok(())
}

fn render_source(
    source: &str,
    color: usvg::filter::ColorInterpolation,
) -> Result<tiny_skia::Pixmap, String> {
    let tree = parse_source(Path::new(FIXTURE), source.as_bytes())?;
    let filter = tree.filters().first().ok_or("fixture filter missing")?;
    let primitive = filter.primitives().first().ok_or("morphology missing")?;
    assert!(matches!(
        primitive.kind(),
        usvg::filter::Kind::Morphology(_)
    ));
    assert_eq!(primitive.color_interpolation(), color);
    render(&tree, Mode::Normal)
}

fn assert_opaque_fixture(source: &str) -> Result<(), String> {
    let document = usvg::roxmltree::Document::parse(source).map_err(|error| error.to_string())?;
    let rect = document
        .descendants()
        .find(|node| node.attribute("id") == Some("rect1"))
        .ok_or("filtered rect missing")?;
    assert_eq!(rect.attribute("fill"), Some("url(#rg1)"));
    assert_eq!(rect.attribute("opacity"), None);
    assert_eq!(rect.attribute("fill-opacity"), None);
    assert_eq!(
        (
            rect.attribute("x"),
            rect.attribute("y"),
            rect.attribute("width"),
            rect.attribute("height")
        ),
        (Some("20"), Some("20"), Some("160"), Some("160"))
    );
    let stops: Vec<_> = document
        .descendants()
        .filter(|node| node.has_tag_name("stop"))
        .collect();
    assert_eq!(stops.len(), 2);
    assert!(
        stops
            .iter()
            .all(|stop| matches!(stop.attribute("stop-opacity"), None | Some("1")))
    );
    Ok(())
}
