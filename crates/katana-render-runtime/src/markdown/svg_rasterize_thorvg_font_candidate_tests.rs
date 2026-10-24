use resvg::usvg::{Options, Size, Tree};
use thorvg::Thorvg;

use super::{create_picture, render_pixels};
use crate::markdown::svg_rasterize::font::bundled_font_db;
use crate::markdown::svg_rasterize::render::RasterTarget;

const FONT_ALIAS: &str = "KrrBundledSansProbe";
const SCALE: f32 = 1.0;
const ALPHA_CHANNEL: usize = 3;
const CHANNEL_COUNT: usize = 4;
const OPAQUE_ALPHA: u8 = u8::MAX;
type RenderPair = (Vec<u8>, Vec<u8>, Vec<u8>);

#[test]
fn registered_static_ttf_changes_text_pixels_without_changing_canvas() -> Result<(), String> {
    let text_svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="32"><text x="2" y="24" font-family="KrrBundledSansProbe" font-size="20">Aa</text></svg>"#;
    let empty_svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="32"></svg>"#;
    let size = parse_size(text_svg)?;
    let (unregistered, text, empty) = render_pair(text_svg, empty_svg, size)?;
    assert_empty_pixels(&unregistered, "unregistered font")?;
    assert_same_canvas(&text, &empty)?;
    assert_text_pixels(&text, &empty)
}

fn parse_size(svg: &str) -> Result<Size, String> {
    Tree::from_str(svg, &Options::default())
        .map(|tree| tree.size())
        .map_err(|error| error.to_string())
}

fn render_pair(text_svg: &str, empty_svg: &str, size: Size) -> Result<RenderPair, String> {
    let engine = Thorvg::init(0).map_err(|error| format!("engine init failed: {error:?}"))?;
    let font_bytes = bundled_font_bytes()?;
    let target = RasterTarget::new(size, SCALE);
    let unregistered = render_one(&engine, text_svg, size, &target)?;
    engine
        .load_font_data(FONT_ALIAS, &font_bytes, Some("ttf"))
        .map_err(|error| format!("font registration failed: {error:?}"))?;
    let text = render_one(&engine, text_svg, size, &target)?;
    let empty = render_one(&engine, empty_svg, size, &target)?;
    Ok((unregistered, text, empty))
}

fn bundled_font_bytes() -> Result<Vec<u8>, String> {
    let database = bundled_font_db();
    let face = database.faces().next().ok_or("bundled face missing")?;
    database
        .with_face_data(face.id, |data, index| (index == 0).then(|| data.to_vec()))
        .flatten()
        .ok_or_else(|| "bundled static TTF face index is unavailable".to_string())
}

fn render_one(
    engine: &Thorvg,
    svg: &str,
    size: Size,
    target: &RasterTarget,
) -> Result<Vec<u8>, String> {
    let picture = create_picture(engine, svg, size, target.effective_scale)
        .map_err(|error| error.to_string())?;
    let mut buffer = vec![0_u32; target.width as usize * target.height as usize];
    render_pixels(engine, picture, &mut buffer, target.width, target.height)
        .map_err(|error| error.to_string())?;
    Ok(buffer.into_iter().flat_map(u32::to_le_bytes).collect())
}

fn assert_same_canvas(text: &[u8], empty: &[u8]) -> Result<(), String> {
    if text.len() != empty.len() {
        return Err(format!(
            "text/empty canvas lengths differ: {} {}",
            text.len(),
            empty.len()
        ));
    }
    Ok(())
}

fn assert_empty_pixels(pixels: &[u8], label: &str) -> Result<(), String> {
    if pixels.iter().any(|channel| *channel != 0) {
        return Err(format!("{label} produced nonzero pixels"));
    }
    Ok(())
}

fn assert_text_pixels(text: &[u8], empty: &[u8]) -> Result<(), String> {
    let mut changed = 0;
    for (text_pixel, empty_pixel) in text
        .chunks_exact(CHANNEL_COUNT)
        .zip(empty.chunks_exact(CHANNEL_COUNT))
    {
        if text_pixel != empty_pixel {
            changed += 1;
        }
        assert_premultiplied_pixel(text_pixel, empty_pixel)?;
    }
    if changed == 0 {
        return Err("registered font produced no pixel difference".to_string());
    }
    Ok(())
}

fn assert_premultiplied_pixel(text: &[u8], empty: &[u8]) -> Result<(), String> {
    if empty.iter().any(|channel| *channel != 0) {
        return Err(format!("empty SVG produced nonzero pixel: {empty:?}"));
    }
    let alpha = text[ALPHA_CHANNEL];
    if alpha > 0
        && alpha < OPAQUE_ALPHA
        && text[..ALPHA_CHANNEL].iter().any(|channel| *channel > alpha)
    {
        return Err(format!("non-premultiplied text pixel: {text:?}"));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
#[path = "svg_rasterize_thorvg_physical_font_tests.rs"]
mod physical_font_tests;
