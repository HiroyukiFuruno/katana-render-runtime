use super::super::super::composite::TextRasterizer;
use super::super::HtmlGlyphRasterizer;
use resvg::usvg::fontdb::Weight;
use tiny_skia::{Pixmap, Transform};

const FAMILY: &str = "Trebuchet MS";
const SAMPLE: &str = "AV";
const WIDTH: u32 = 128;
const HEIGHT: u32 = 64;
const RGBA_CHANNELS: usize = 4;
const ALPHA_CHANNEL: usize = 3;

#[test]
fn coretext_backend_renders_fixture_with_expected_vertical_orientation() -> Result<(), String> {
    let database = super::super::super::font::html_font_db_for_text(FAMILY, SAMPLE);
    let tree = super::parse_pixel_tree(&database, FAMILY, Weight::NORMAL)?;
    let Some(resvg::usvg::Node::Text(text)) = tree.node_by_id("run") else {
        return Err("Trebuchet pixel fixture text missing".to_string());
    };
    let rasterizer = HtmlGlyphRasterizer::new_coretext(&database);
    let mut pixmap = Pixmap::new(WIDTH, HEIGHT).ok_or("CoreText test pixmap unavailable")?;
    rasterizer.render(text, Transform::identity(), &mut pixmap.as_mut())?;
    let pixels = pixmap.data().to_vec();
    if !has_upper_baseline_mass(&pixels)? {
        return Err("CoreText fixture orientation differs from SVG baseline".to_string());
    }
    let flipped = flip_rows(&pixels)?;
    if has_upper_baseline_mass(&flipped)? {
        return Err("CoreText orientation check accepted flipped rows".to_string());
    }
    Ok(())
}

fn has_upper_baseline_mass(pixels: &[u8]) -> Result<bool, String> {
    let split = usize::try_from(HEIGHT / 2).map_err(|_| "height overflow")?;
    let row_bytes = usize::try_from(WIDTH).map_err(|_| "width overflow")? * RGBA_CHANNELS;
    let expected_len = usize::try_from(WIDTH)
        .map_err(|_| "width overflow")?
        .checked_mul(usize::try_from(HEIGHT).map_err(|_| "height overflow")?)
        .and_then(|pixels| pixels.checked_mul(RGBA_CHANNELS))
        .ok_or("pixel buffer length overflow")?;
    if pixels.len() != expected_len {
        return Err("unexpected CoreText pixel buffer length".to_string());
    }
    let upper = alpha_mass(&pixels[..split * row_bytes]);
    let lower = alpha_mass(&pixels[split * row_bytes..]);
    if upper == 0 && lower == 0 {
        return Err("CoreText fixture contains no ink".to_string());
    }
    Ok(upper > lower)
}

fn alpha_mass(pixels: &[u8]) -> u64 {
    pixels
        .chunks_exact(RGBA_CHANNELS)
        .map(|pixel| u64::from(pixel[ALPHA_CHANNEL]))
        .sum()
}

fn flip_rows(pixels: &[u8]) -> Result<Vec<u8>, String> {
    let row_bytes = usize::try_from(WIDTH).map_err(|_| "width overflow")? * RGBA_CHANNELS;
    let rows = usize::try_from(HEIGHT).map_err(|_| "height overflow")?;
    if pixels.len() != rows * row_bytes {
        return Err("unexpected CoreText pixel buffer length".to_string());
    }
    let mut flipped = vec![0; pixels.len()];
    for row in 0..rows {
        let source = &pixels[row * row_bytes..(row + 1) * row_bytes];
        let destination = (rows - row - 1) * row_bytes;
        flipped[destination..destination + row_bytes].copy_from_slice(source);
    }
    Ok(flipped)
}
