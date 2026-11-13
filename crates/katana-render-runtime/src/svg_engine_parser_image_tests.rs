use super::{resvg, usvg};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::{ExtendedColorType, ImageEncoder, codecs::webp::WebPEncoder};
use rstest::rstest;
use std::{error::Error, path::Path, sync::Arc};
use tiny_skia::{Pixmap, Transform};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const RESOURCES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/svg-engine/upstream/resvg/tests/resources"
);
const SVG_IMAGE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1"><rect width="2" height="1" fill="red"/></svg>"#;
const VIEWPORT: u32 = 4;
const OPAQUE_RGB: [u8; 3] = [20, 100, 200];
const OPAQUE_RGB_WIDE: [u8; 6] = [20, 100, 200, 20, 100, 200];
const SINGLE_PIXEL: (u32, u32) = (1, 1);
const WIDE_PIXELS: (u32, u32) = (2, 1);
const BLUE_SAMPLE: (u32, u32) = (3, 1);
const BLUE_RGBA: (u8, u8, u8, u8) = (0, 0, 255, 255);

#[derive(Clone, Copy, Debug)]
enum Format {
    Png,
    Gif,
    Jpeg,
    Webp,
    Svg,
}

fn matches_format(kind: &usvg::ImageKind, format: Format) -> bool {
    matches!(
        (kind, format),
        (usvg::ImageKind::PNG(_), Format::Png)
            | (usvg::ImageKind::GIF(_), Format::Gif)
            | (usvg::ImageKind::JPEG(_), Format::Jpeg)
            | (usvg::ImageKind::WEBP(_), Format::Webp)
            | (usvg::ImageKind::SVG(_), Format::Svg)
    )
}

fn assert_png_image(tree: &usvg::Tree) -> TestResult {
    const PNG_SIZE: f32 = 2.0;
    let valid = tree.node_by_id("valid").ok_or("valid PNG missing")?;
    assert!(matches!(valid, usvg::Node::Group(group) if
        matches!(group.children(), [usvg::Node::Image(image)] if matches_format(image.kind(), Format::Png))
    ));
    let bounds = valid.abs_bounding_box();
    assert_eq!((bounds.width(), bounds.height()), (PNG_SIZE, PNG_SIZE));
    Ok(())
}

fn assert_render_matches_with_blue_sibling(tree: &usvg::Tree, expected: &usvg::Tree) -> TestResult {
    let actual = render_test_image(tree)?;
    assert_eq!(actual.data(), render_test_image(expected)?.data());
    assert_blue_sibling(&actual)
}

fn assert_blue_sibling(image: &Pixmap) -> TestResult {
    let blue = image
        .pixel(BLUE_SAMPLE.0, BLUE_SAMPLE.1)
        .ok_or("sibling sample missing")?;
    assert_eq!(
        (blue.red(), blue.green(), blue.blue(), blue.alpha()),
        BLUE_RGBA
    );
    Ok(())
}

#[test]
fn unsupported_bmp_is_not_mistaken_for_svg() -> TestResult {
    const BMP: &str =
        "Qk06AAAAAAAAADYAAAAoAAAAAQAAAAEAAAABABgAAAAAAAQAAAAAAAAAAAAAAAAAAAAAAAAAAAD/AA==";
    let data = STANDARD.decode(BMP)?;
    assert!(matches!(
        imagesize::image_type(&data),
        Ok(imagesize::ImageType::Bmp)
    ));
    let source = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='4' height='4'><image id='unsupported' width='1' height='1' href='data:text/plain;base64,{BMP}'/><rect id='control' width='1' height='1'/></svg>"
    );
    let tree = usvg::Tree::from_str(&source, &usvg::Options::default())?;
    assert!(tree.node_by_id("unsupported").is_none());
    assert!(tree.node_by_id("control").is_some());
    Ok(())
}

#[test]
fn image_without_href_preserves_png_and_visible_sibling() -> TestResult {
    const MISSING: &str = "<image id='missing' width='2' height='2'/>";
    let png = STANDARD.encode(std::fs::read(Path::new(RESOURCES).join("image.png"))?);
    let source = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='4' height='4'>{MISSING}<image id='valid' width='2' height='2' href='data:image/png;base64,{png}'/><rect id='sibling' x='2' width='2' height='4' fill='blue'/></svg>"
    );
    let tree = usvg::Tree::from_str(&source, &usvg::Options::default())?;
    let expected = usvg::Tree::from_str(&source.replace(MISSING, ""), &usvg::Options::default())?;
    assert!(tree.node_by_id("missing").is_none());
    assert_png_image(&tree)?;
    assert!(tree.node_by_id("sibling").is_some());
    assert_render_matches_with_blue_sibling(&tree, &expected)
}

fn render_test_image(tree: &usvg::Tree) -> TestResult<Pixmap> {
    let mut pixmap = Pixmap::new(VIEWPORT, VIEWPORT).ok_or("pixmap unavailable")?;
    resvg::render(tree, Transform::identity(), &mut pixmap.as_mut());
    Ok(pixmap)
}

#[rstest]
#[case::png("image.png", Format::Png)]
#[case::gif("image.gif", Format::Gif)]
#[case::jpeg("image.jpg", Format::Jpeg)]
#[case::webp("image.webp", Format::Webp)]
fn plain_text_data_resolver_sniffs_real_raster_magic(
    #[case] file: &str,
    #[case] expected: Format,
) -> TestResult {
    let bytes = std::fs::read(Path::new(RESOURCES).join(file))?;
    let resolver = usvg::ImageHrefResolver::default_data_resolver();
    let kind = resolver("text/plain", Arc::new(bytes), &usvg::Options::default())
        .ok_or("image magic was not recognised")?;
    assert!(matches_format(&kind, expected));
    Ok(())
}

#[rstest]
#[case::png("image.png", Format::Png)]
#[case::gif("image.gif", Format::Gif)]
#[case::jpeg("image.jpg", Format::Jpeg)]
#[case::webp("image.webp", Format::Webp)]
fn string_resolver_loads_real_images_from_resource_directory(
    #[case] file: &str,
    #[case] expected: Format,
) -> TestResult {
    let options = usvg::Options {
        resources_dir: Some(Path::new(RESOURCES).to_path_buf()),
        ..Default::default()
    };
    let resolver = usvg::ImageHrefResolver::default_string_resolver();
    let kind = resolver(file, &options).ok_or("file was not resolved")?;
    assert!(matches_format(&kind, expected));
    Ok(())
}

#[test]
fn data_resolver_accepts_svg_and_rejects_bad_or_unknown_data() {
    let options = usvg::Options::default();
    let resolver = usvg::ImageHrefResolver::default_data_resolver();
    let svg = resolver("text/plain", Arc::new(SVG_IMAGE.to_vec()), &options);
    assert!(
        svg.as_ref()
            .is_some_and(|kind| matches_format(kind, Format::Svg))
    );
    assert!(resolver("text/plain", Arc::new(b"<svg".to_vec()), &options).is_none());
    assert!(resolver("image/unknown", Arc::new(SVG_IMAGE.to_vec()), &options).is_none());
    assert_eq!(
        format!("{:?}", usvg::ImageHrefResolver::default()),
        "ImageHrefResolver { .. }"
    );
}

#[test]
fn file_resolver_rejects_missing_directory_and_unknown_file() -> TestResult {
    let temporary = tempfile::tempdir()?;
    std::fs::write(temporary.path().join("unknown.bin"), b"not an image")?;
    let options = usvg::Options {
        resources_dir: Some(temporary.path().to_path_buf()),
        ..Default::default()
    };
    let resolver = usvg::ImageHrefResolver::default_string_resolver();
    for path in ["missing.png", ".", "unknown.bin"] {
        assert!(resolver(path, &options).is_none(), "{path}");
    }
    Ok(())
}

fn webp_svg(
    color: ExtendedColorType,
    bytes: &[u8],
    dimensions: (u32, u32),
    attributes: &str,
) -> TestResult<String> {
    let mut data = Vec::new();
    WebPEncoder::new_lossless(&mut data).write_image(bytes, dimensions.0, dimensions.1, color)?;
    let decoder = image_webp::WebPDecoder::new(std::io::Cursor::new(&data))?;
    assert_eq!(
        decoder.has_alpha(),
        matches!(color, ExtendedColorType::Rgba8)
    );
    let url = STANDARD.encode(data);
    Ok(format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='4' height='4'><image id='image' {attributes} href='data:text/plain;base64,{url}'/></svg>"
    ))
}

#[rstest]
#[case::opaque(ExtendedColorType::Rgb8, &OPAQUE_RGB[..], (20, 100, 200, 255))]
#[case::alpha(ExtendedColorType::Rgba8, &[200, 100, 50, 128][..], (100, 50, 25, 128))]
fn lossless_webp_decodes_real_rgb_and_rgba(
    #[case] color: ExtendedColorType,
    #[case] source: &[u8],
    #[case] expected: (u8, u8, u8, u8),
) -> TestResult {
    let svg = webp_svg(
        color,
        source,
        SINGLE_PIXEL,
        "width='1' height='1' image-rendering='pixelated'",
    )?;
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default())?;
    let mut pixmap = Pixmap::new(VIEWPORT, VIEWPORT).ok_or("pixmap unavailable")?;
    resvg::render(&tree, Transform::identity(), &mut pixmap.as_mut());
    let pixel = pixmap.pixel(0, 0).ok_or("pixel unavailable")?;
    assert_eq!(
        (pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()),
        expected
    );
    Ok(())
}

#[rstest]
#[case::width("width='3'", 3.0, 1.5)]
#[case::height("height='2'", 4.0, 2.0)]
#[case::both("width='3' height='2' preserveAspectRatio='none'", 3.0, 2.0)]
fn raster_image_viewport_uses_intrinsic_aspect_ratio(
    #[case] attributes: &str,
    #[case] width: f32,
    #[case] height: f32,
) -> TestResult {
    let svg = webp_svg(
        ExtendedColorType::Rgb8,
        &OPAQUE_RGB_WIDE,
        WIDE_PIXELS,
        attributes,
    )?;
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default())?;
    let node = tree.node_by_id("image").ok_or("image missing")?;
    let bounds = node.abs_bounding_box();
    assert_eq!((bounds.width(), bounds.height()), (width, height));
    Ok(())
}

#[rstest]
#[case::png("image.png")]
#[case::jpeg("image.jpg")]
#[case::gif("image.gif")]
#[case::webp("image.webp")]
fn truncated_recognized_raster_does_not_paint(#[case] file: &str) -> TestResult {
    let bytes = std::fs::read(Path::new(RESOURCES).join(file))?;
    let cutoff = (1..bytes.len())
        .find(|end| imagesize::blob_size(&bytes[..*end]).is_ok())
        .ok_or("no recognizable raster header")?;
    let url = STANDARD.encode(&bytes[..cutoff]);
    let input = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='4' height='4'><image id='image' width='2' height='2' href='data:text/plain;base64,{url}'/></svg>"
    );
    let tree = usvg::Tree::from_str(&input, &usvg::Options::default())?;
    assert!(
        tree.node_by_id("image").is_some(),
        "recognized header must reach rendering"
    );
    let mut pixmap = Pixmap::new(VIEWPORT, VIEWPORT).ok_or("pixmap unavailable")?;
    resvg::render(&tree, Transform::identity(), &mut pixmap.as_mut());
    assert!(pixmap.data().iter().all(|value| *value == 0));
    Ok(())
}

#[test]
fn interlaced_gif_with_zero_height_frame_keeps_visible_sibling() -> TestResult {
    let bytes = zero_height_gif_frame()?;
    let encoded = STANDARD.encode(&bytes);
    let source = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='4' height='4'><image id='image' width='2' height='2' href='data:image/gif;base64,{encoded}'/><rect x='2' width='2' height='4' fill='blue'/></svg>"
    );
    let tree = usvg::Tree::from_str(&source, &usvg::Options::default())?;
    assert!(tree.node_by_id("image").is_some());
    let image = render_test_image(&tree)?;
    assert_eq!(image.pixel(0, 0).ok_or("image sample missing")?.alpha(), 0);
    assert_blue_sibling(&image)
}

fn zero_height_gif_frame() -> TestResult<Vec<u8>> {
    const FRAME_DESCRIPTOR: usize = 405;
    const FRAME_HEIGHT_OFFSET: usize = FRAME_DESCRIPTOR + 7;
    const FRAME_HEIGHT_END: usize = FRAME_HEIGHT_OFFSET + std::mem::size_of::<u16>();
    let mut bytes = std::fs::read(Path::new(RESOURCES).join("image.gif"))?;
    assert_eq!(
        &bytes[FRAME_DESCRIPTOR..FRAME_DESCRIPTOR + 10],
        &[0x2c, 0, 0, 0, 0, 64, 0, 64, 0, 0x40]
    );
    bytes[FRAME_HEIGHT_OFFSET..FRAME_HEIGHT_END].copy_from_slice(&0_u16.to_le_bytes());
    assert!(imagesize::blob_size(&bytes).is_ok());
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options.read_info(bytes.as_slice())?;
    let frame = decoder.read_next_frame()?.ok_or("GIF frame missing")?;
    assert_eq!((frame.width, frame.height), (64, 0));
    Ok(bytes)
}
