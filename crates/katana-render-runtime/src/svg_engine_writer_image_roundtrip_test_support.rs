use super::{
    ALPHA_CHANNEL, RGBA_CHANNELS, assert_image_size, assert_rgba_pixel, image_node, inline_png_svg,
    parse_writer_svg, render_tree, usvg, writer_options,
};

const IMAGE_FIXTURE_DIRECTORY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/svg-engine/upstream/resvg/tests/tests/structure/image"
);
const UPSTREAM_IMAGE_DIMENSION: f32 = 64.0;
const RASTER_MODE_SOURCE_DIMENSION: f32 = 2.0;
const RASTER_MODE_IMAGE_DIMENSION: f32 = 4.0;
const RASTER_MODE_VIEWPORT_DIMENSION: f32 = 6.0;
const RASTER_MODE_IMAGE_ORIGIN: f32 = 1.0;

#[derive(Clone, Copy, Debug)]
pub(super) enum RasterFormat {
    Jpeg,
    Gif,
    Webp,
}

pub(super) fn assert_embedded_raster_roundtrip(
    fixture_name: &str,
    format: RasterFormat,
) -> Result<(), String> {
    let path = std::path::Path::new(IMAGE_FIXTURE_DIRECTORY).join(fixture_name);
    let input = std::fs::read_to_string(&path)
        .map_err(|error| format!("failed reading {}: {error}", path.display()))?;
    let original = parse_raster_tree(&input, fixture_name, format)?;

    let before = render_tree(&original)?;
    assert!(
        before
            .data()
            .chunks_exact(RGBA_CHANNELS)
            .any(|pixel| pixel[ALPHA_CHANNEL] != 0),
        "{fixture_name} should render visible raster pixels"
    );
    assert_rgba_pixel(&before, (0, 0), (0, 0, 0, 0))?;

    let serialized = original.to_string(&writer_options(fixture_name));
    let reparsed = parse_raster_tree(&serialized, fixture_name, format)?;
    let after = render_tree(&reparsed)?;
    assert_eq!(
        before.data(),
        after.data(),
        "{fixture_name} roundtrip pixels"
    );
    Ok(())
}

fn parse_raster_tree(
    input: &str,
    fixture_name: &str,
    format: RasterFormat,
) -> Result<usvg::Tree, String> {
    let tree = parse_writer_svg(input, fixture_name)?;
    let image = image_node(&tree, "image1")?;
    assert_raster_format(image, format);
    assert_image_size(image, (UPSTREAM_IMAGE_DIMENSION, UPSTREAM_IMAGE_DIMENSION));
    Ok(tree)
}

fn assert_raster_format(image: &usvg::Image, expected: RasterFormat) {
    let matches = matches!(
        (image.kind(), expected),
        (usvg::ImageKind::JPEG(_), RasterFormat::Jpeg)
            | (usvg::ImageKind::GIF(_), RasterFormat::Gif)
            | (usvg::ImageKind::WEBP(_), RasterFormat::Webp)
    );
    assert!(matches, "embedded raster image kind did not match fixture");
}

pub(super) fn raster_mode_outputs() -> Result<Vec<Vec<u8>>, String> {
    let modes = [
        ("optimizeQuality", usvg::ImageRendering::OptimizeQuality),
        ("optimizeSpeed", usvg::ImageRendering::OptimizeSpeed),
        ("smooth", usvg::ImageRendering::Smooth),
        ("high-quality", usvg::ImageRendering::HighQuality),
        ("crisp-edges", usvg::ImageRendering::CrispEdges),
        ("pixelated", usvg::ImageRendering::Pixelated),
    ];
    let mut output = Vec::with_capacity(modes.len());
    for (mode, expected_mode) in modes {
        output.push(render_raster_mode(mode, expected_mode)?);
    }
    Ok(output)
}

fn render_raster_mode(mode: &str, expected_mode: usvg::ImageRendering) -> Result<Vec<u8>, String> {
    let input = inline_png_svg()?
        .replace("width=\"4\" height=\"4\"", "width=\"6\" height=\"6\"")
        .replace("width=\"2\" height=\"2\"", "width=\"4\" height=\"4\"")
        .replace(
            "<image id=\"rgba\"",
            &format!("<image id=\"rgba\" style=\"image-rendering: {mode}\""),
        );
    let tree = parse_writer_svg(&input, mode)?;
    assert_raster_mode_image(&tree, mode, expected_mode)?;
    let before = render_tree(&tree)?;
    assert!(
        before
            .data()
            .chunks_exact(RGBA_CHANNELS)
            .any(|pixel| pixel[ALPHA_CHANNEL] != 0),
        "image-rendering={mode} must paint pixels"
    );
    let serialized = tree.to_string(&writer_options(mode));
    let reparsed = parse_writer_svg(&serialized, mode)?;
    assert_raster_mode_image(&reparsed, mode, expected_mode)?;
    let after = render_tree(&reparsed)?;
    super::super::writer_coverage_tests::assert_pixmaps_equal(&before, &after, mode);
    Ok(before.data().to_vec())
}

fn assert_raster_mode_image(
    tree: &usvg::Tree,
    mode: &str,
    expected_mode: usvg::ImageRendering,
) -> Result<(), String> {
    let image = image_node(tree, "rgba")?;
    assert_eq!(image.rendering_mode(), expected_mode);
    assert!(matches!(image.kind(), usvg::ImageKind::PNG(_)));
    assert_image_size(
        image,
        (RASTER_MODE_SOURCE_DIMENSION, RASTER_MODE_SOURCE_DIMENSION),
    );
    assert_raster_mode_geometry(tree, image, mode);
    Ok(())
}

fn assert_raster_mode_geometry(tree: &usvg::Tree, image: &usvg::Image, mode: &str) {
    let bounds = image.abs_bounding_box();
    assert_eq!(
        (bounds.left(), bounds.top(), bounds.right(), bounds.bottom()),
        (
            RASTER_MODE_IMAGE_ORIGIN,
            RASTER_MODE_IMAGE_ORIGIN,
            RASTER_MODE_IMAGE_ORIGIN + RASTER_MODE_IMAGE_DIMENSION,
            RASTER_MODE_IMAGE_ORIGIN + RASTER_MODE_IMAGE_DIMENSION
        ),
        "display bounds for image-rendering={mode}"
    );
    assert_raster_mode_viewport(tree, mode);
}

fn assert_raster_mode_viewport(tree: &usvg::Tree, mode: &str) {
    assert_eq!(
        tree.size().width(),
        RASTER_MODE_VIEWPORT_DIMENSION,
        "viewport width for image-rendering={mode}"
    );
    assert_eq!(
        tree.size().height(),
        RASTER_MODE_VIEWPORT_DIMENSION,
        "viewport height for image-rendering={mode}"
    );
}
