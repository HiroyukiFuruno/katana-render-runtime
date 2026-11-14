use super::super::{HtmlGlyphRasterizer, PreparedSpan};
use core_graphics::{
    base::kCGImageAlphaPremultipliedLast, color_space::CGColorSpace, context::CGContext,
    data_provider::CGDataProvider, font::CGFont, geometry::CGPoint,
};
use core_text::font::new_from_CGFont;
use image::ImageEncoder;
use resvg::usvg::fontdb::{Database, ID, Weight};
use skia_safe::Typeface;
use skrifa::{FontRef, raw::TableProvider};
use std::sync::Arc;
use std::time::Instant;

const FAMILY: &str = "Trebuchet MS";
const SAMPLE: &str = "AV";
const FONT_SIZE: f64 = 16.0;
const WIDTH: usize = 128;
const HEIGHT: usize = 64;
const CHANNELS: usize = 4;
const ALPHA_CHANNEL: usize = 3;
const BITS_PER_COMPONENT: usize = 8;

#[test]
fn physical_trebuchet_coretext_and_skia_raster_paths_are_observed() -> Result<(), String> {
    with_skia_fixture(|database, rasterizer, id, typeface, span| {
        let skia_start = Instant::now();
        let skia_pixels = super::draw_pixels(span, typeface)?;
        let skia_render_ns = skia_start.elapsed().as_nanos();
        let (font_bytes, index) = physical_bytes(database, rasterizer, id)?;
        let coretext_start = Instant::now();
        let coretext_pixels = render_coretext(&font_bytes, index, &span.glyph_ids, &span.glyphs)?;
        let coretext_render_ns = coretext_start.elapsed().as_nanos();
        assert_glyph_contract(span, typeface, &skia_pixels, &coretext_pixels)?;
        save_probe_images(&skia_pixels, &coretext_pixels)?;
        eprintln!(
            "coretext-skia candidate family={FAMILY} glyphs={} skia_render_ns={skia_render_ns} coretext_render_ns={coretext_render_ns} delta={:?} timing_note=skia_face_preinitialized_coretext_font_initialized_in_timer",
            span.glyph_ids.len(),
            pixel_delta(&skia_pixels, &coretext_pixels),
        );
        Ok(())
    })
}

#[test]
fn coretext_only_uses_fontdb_bytes_and_usvg_glyph_ids() -> Result<(), String> {
    let total_started = Instant::now();
    let database = super::super::super::font::html_font_db_for_text(FAMILY, SAMPLE);
    let rasterizer = HtmlGlyphRasterizer::new(&database);
    let id = super::find_face(&database, FAMILY, Weight::NORMAL)?;
    let _ = rasterizer.face_file(id)?;
    let (font_bytes, index) = database_face_bytes(&database, id)?;
    let face = FontRef::from_index(&font_bytes, 0).map_err(|_| "physical font parse failed")?;
    face.head().map_err(|_| "physical font head unavailable")?;
    let tree = super::parse_pixel_tree(&database, FAMILY, Weight::NORMAL)?;
    let text = pixel_fixture_text(&tree)?;
    let [span, ..] = text.layouted() else {
        return Err("pixel fixture span missing".into());
    };
    let glyph_ids = super::pixel_glyph_ids(&span.positioned_glyphs)?;
    let started = Instant::now();
    let pixels = render_coretext(&font_bytes, 0, &glyph_ids, &span.positioned_glyphs)?;
    let elapsed_ns = started.elapsed().as_nanos();
    assert_pixels("CoreText-only", &pixels)?;
    eprintln!(
        "coretext-only family={FAMILY} bytes={} face_index={index} glyphs={} render_ns={elapsed_ns} total_ns={}",
        font_bytes.len(),
        glyph_ids.len(),
        total_started.elapsed().as_nanos()
    );
    Ok(())
}

#[test]
fn skia_only_uses_same_usvg_glyph_ids_and_canvas() -> Result<(), String> {
    let total_started = Instant::now();
    with_skia_fixture(|_, _, _, typeface, span| {
        let started = Instant::now();
        let pixels = super::draw_pixels(span, typeface)?;
        let elapsed_ns = started.elapsed().as_nanos();
        assert_pixels("Skia-only", &pixels)?;
        eprintln!(
            "skia-only family={FAMILY} glyphs={} render_ns={elapsed_ns} total_ns={}",
            span.glyph_ids.len(),
            total_started.elapsed().as_nanos()
        );
        Ok(())
    })
}

fn with_skia_fixture<T>(
    action: impl for<'a> FnOnce(
        &'a Database,
        &'a HtmlGlyphRasterizer<'a>,
        ID,
        &'a skia_safe::Typeface,
        &'a PreparedSpan,
    ) -> Result<T, String>,
) -> Result<T, String> {
    let database = super::super::super::font::html_font_db_for_text(FAMILY, SAMPLE);
    let rasterizer = HtmlGlyphRasterizer::new(&database);
    let id = super::find_face(&database, FAMILY, Weight::NORMAL)?;
    let cached = rasterizer
        .ensure_face(id)?
        .ok_or("file typeface unavailable")?;
    let typeface = cached.typeface.as_skia();
    let span = prepare_span(&database, id, typeface)?;
    action(&database, &rasterizer, id, typeface, &span)
}

fn prepare_span(
    database: &Database,
    id: resvg::usvg::fontdb::ID,
    typeface: &Typeface,
) -> Result<super::super::PreparedSpan, String> {
    let upem = typeface
        .units_per_em()
        .ok_or("physical typeface units-per-em unavailable")? as f32;
    let tree = super::parse_pixel_tree(database, FAMILY, Weight::NORMAL)?;
    super::prepare_pixel_span(pixel_fixture_text(&tree)?, id, upem, typeface)
}

fn pixel_fixture_text(tree: &resvg::usvg::Tree) -> Result<&resvg::usvg::Text, String> {
    match tree.node_by_id("run") {
        Some(resvg::usvg::Node::Text(text)) => Ok(text),
        _ => Err("Trebuchet pixel fixture text missing".to_string()),
    }
}
fn physical_bytes(
    database: &Database,
    rasterizer: &HtmlGlyphRasterizer<'_>,
    id: resvg::usvg::fontdb::ID,
) -> Result<(Vec<u8>, u32), String> {
    let Some((_, index)) = rasterizer.face_file(id)? else {
        return Err("Trebuchet source is not file-backed".to_string());
    };
    let (bytes, actual_index) = database_face_bytes(database, id)?;
    if index != actual_index {
        return Err("Trebuchet source face index changed".to_string());
    }
    Ok((bytes, 0))
}

fn database_face_bytes(
    database: &Database,
    id: resvg::usvg::fontdb::ID,
) -> Result<(Vec<u8>, u32), String> {
    let (bytes, index) = database
        .with_face_data(id, |bytes, face_index| (bytes.to_vec(), face_index))
        .ok_or("Trebuchet source bytes unavailable")?;
    let bytes = if index == 0 {
        bytes
    } else {
        super::super::super::physical_font_face::repack_face(&bytes, index)?
    };
    Ok((bytes, index))
}
fn render_coretext(
    bytes: &[u8],
    index: u32,
    glyph_ids: &[u16],
    glyphs: &[resvg::usvg::layout::PositionedGlyph],
) -> Result<Vec<u8>, String> {
    if index != 0 {
        return Err("CoreText candidate requires a single-face source".to_string());
    }
    let provider = CGDataProvider::from_buffer(Arc::new(bytes.to_vec()));
    let cgfont = CGFont::from_data_provider(provider).map_err(|_| "CGFont creation failed")?;
    let font = new_from_CGFont(&cgfont, FONT_SIZE);
    let mut context = bitmap_context();
    let positions = glyph_positions(glyphs);
    font.draw_glyphs(glyph_ids, &positions, context.clone());
    Ok(context.data().to_vec())
}

fn bitmap_context() -> CGContext {
    let color_space = CGColorSpace::create_device_rgb();
    let context = CGContext::create_bitmap_context(
        None,
        WIDTH,
        HEIGHT,
        BITS_PER_COMPONENT,
        WIDTH * CHANNELS,
        &color_space,
        kCGImageAlphaPremultipliedLast,
    );
    context.set_allows_font_smoothing(true);
    context.set_should_smooth_fonts(true);
    context.set_allows_antialiasing(true);
    context.set_should_antialias(true);
    context.set_allows_font_subpixel_positioning(true);
    context.set_should_subpixel_position_fonts(true);
    context.set_rgb_fill_color(0.0, 0.0, 0.0, 1.0);
    context
}

fn glyph_positions(glyphs: &[resvg::usvg::layout::PositionedGlyph]) -> Vec<CGPoint> {
    glyphs
        .iter()
        .map(|glyph| {
            let transform = glyph.transform();
            /* WHY: CoreTextの下原点を、共通fixtureの上原点へ対応させる。 */
            CGPoint::new(
                f64::from(transform.tx),
                f64::from(HEIGHT as f32 - transform.ty),
            )
        })
        .collect()
}

fn assert_glyph_contract(
    span: &super::super::PreparedSpan,
    typeface: &Typeface,
    skia_pixels: &[u8],
    coretext_pixels: &[u8],
) -> Result<(), String> {
    if span.glyph_ids.is_empty()
        || span.glyph_ids.len() != span.glyphs.len()
        || typeface.count_glyphs() == 0
    {
        return Err("physical glyph contract is empty".to_string());
    }
    assert_pixels("Skia", skia_pixels)?;
    assert_pixels("CoreText", coretext_pixels)
}

fn assert_premultiplied(label: &str, pixels: &[u8]) -> Result<(), String> {
    if pixels.chunks_exact(CHANNELS).any(|pixel| {
        pixel[..ALPHA_CHANNEL]
            .iter()
            .any(|channel| *channel > pixel[ALPHA_CHANNEL])
    }) {
        return Err(format!("{label} pixels are not premultiplied RGBA"));
    }
    Ok(())
}

fn assert_pixels(label: &str, pixels: &[u8]) -> Result<(), String> {
    if pixels.len() != WIDTH * HEIGHT * CHANNELS {
        return Err(format!("{label} canvas size changed"));
    }
    if !pixels
        .chunks_exact(CHANNELS)
        .any(|pixel| pixel[ALPHA_CHANNEL] != 0)
    {
        return Err(format!("{label} produced no ink"));
    }
    assert_premultiplied(label, pixels)
}

fn pixel_delta(left: &[u8], right: &[u8]) -> (usize, u8) {
    left.iter()
        .zip(right)
        .fold((0, 0), |(count, maximum), (left, right)| {
            let delta = left.abs_diff(*right);
            (count + usize::from(delta != 0), maximum.max(delta))
        })
}

fn save_probe_images(skia: &[u8], coretext: &[u8]) -> Result<(), String> {
    let Ok(directory) = std::env::var("KRR_CORETEXT_PROBE_OUTPUT") else {
        return Ok(());
    };
    let directory = std::path::Path::new(&directory);
    if !directory.is_absolute() {
        return Err("KRR_CORETEXT_PROBE_OUTPUT must be absolute".to_string());
    }
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let width = u32::try_from(WIDTH).map_err(|_| "probe width overflow")?;
    let height = u32::try_from(HEIGHT).map_err(|_| "probe height overflow")?;
    write_probe_png(&directory.join("skia.png"), skia, width, height)?;
    write_probe_png(&directory.join("coretext.png"), coretext, width, height)
}

fn write_probe_png(
    path: &std::path::Path,
    pixels: &[u8],
    width: u32,
    height: u32,
) -> Result<(), String> {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    image::codecs::png::PngEncoder::new(std::io::BufWriter::new(file))
        .write_image(pixels, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|error| error.to_string())
}
