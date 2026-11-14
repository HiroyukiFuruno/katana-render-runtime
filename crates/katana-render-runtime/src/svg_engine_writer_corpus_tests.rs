use std::path::{Path, PathBuf};

use imageproc::{image::RgbaImage, stats::root_mean_squared_error};
use rstest::rstest;

use super::{upstream_writer_font_database, usvg, writer_coverage_tests::render_pair};

const CHANNELS: usize = 4;
const RGB_CHANNELS: usize = 3;
const ALPHA_CHANNEL: usize = 3;
const MAX_CHANNEL: f64 = 255.0;
const SCORE_SCALE: f64 = 100.0;
const MIN_SCORE: f64 = 99.0;

#[rstest]
fn upstream_text_and_use_preserve_rendering_after_first_write(
    #[files("../../tests/fixtures/svg-engine/upstream/resvg/tests/tests/text/text/*.svg")]
    #[files("../../tests/fixtures/svg-engine/upstream/resvg/tests/tests/text/tspan/*.svg")]
    #[files("../../tests/fixtures/svg-engine/upstream/resvg/tests/tests/structure/use/*.svg")]
    input: PathBuf,
) -> Result<(), String> {
    let (original, parsed) = roundtrip_fixture(&input)?;
    assert_eq!(
        original.size(),
        parsed.size(),
        "viewport: {}",
        input.display()
    );
    let (before, after) = render_pair(&original, &parsed)?;
    let score = score_pixmaps(&before, &after)?;
    /* WHY: writerは既定で座標を丸め、画素完全一致を保証しないため、アルファ乗算を戻したRGBAで99以上を確認する。 */
    assert!(
        score >= MIN_SCORE,
        "{}: RGBA score {score}",
        input.display()
    );
    Ok(())
}

fn roundtrip_fixture(input: &Path) -> Result<(usvg::Tree, usvg::Tree), String> {
    let bytes = std::fs::read(input).map_err(|error| format!("{}: {error}", input.display()))?;
    let options = usvg::Options {
        fontdb: upstream_writer_font_database()?,
        resources_dir: input.parent().map(Path::to_path_buf),
        ..usvg::Options::default()
    };
    let original = usvg::Tree::from_data(&bytes, &options)
        .map_err(|error| format!("parse {}: {error}", input.display()))?;
    let serialized = original.to_string(&usvg::WriteOptions::default());
    let parsed = usvg::Tree::from_str(&serialized, &options)
        .map_err(|error| format!("reparse {}: {error}", input.display()))?;
    Ok((original, parsed))
}

fn score_pixmaps(before: &tiny_skia::Pixmap, after: &tiny_skia::Pixmap) -> Result<f64, String> {
    if before.width() != after.width() || before.height() != after.height() {
        return Err("writer raster dimensions differ".to_owned());
    }
    score_rgba(&straight_rgba(before)?, &straight_rgba(after)?)
}

fn straight_rgba(pixmap: &tiny_skia::Pixmap) -> Result<RgbaImage, String> {
    validate_rgba_dimensions(pixmap.width(), pixmap.height())?;
    let mut bytes = Vec::with_capacity(pixmap.data().len());
    for pixel in pixmap.pixels() {
        let color = pixel.demultiply();
        bytes.extend_from_slice(&[color.red(), color.green(), color.blue(), color.alpha()]);
    }
    RgbaImage::from_raw(pixmap.width(), pixmap.height(), bytes)
        .ok_or_else(|| "invalid RGBA raster".to_owned())
}

fn score_rgba(before: &RgbaImage, after: &RgbaImage) -> Result<f64, String> {
    if before.dimensions() != after.dimensions() {
        return Err("RGBA raster dimensions differ".to_owned());
    }
    validate_rgba_dimensions(before.width(), before.height())?;
    Ok(SCORE_SCALE * (1.0 - root_mean_squared_error(before, after) / MAX_CHANNEL))
}

fn validate_rgba_dimensions(width: u32, height: u32) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err("RGBA raster dimensions are empty".to_owned());
    }
    width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(CHANNELS as u32))
        .ok_or("RGBA raster exceeds imageproc dimensions")?;
    Ok(())
}

#[test]
fn rgba_scoring_rejects_mismatched_and_empty_dimensions() -> Result<(), String> {
    let one = RgbaImage::from_pixel(1, 1, imageproc::image::Rgba([0, 0, 0, u8::MAX]));
    let two = RgbaImage::from_pixel(2, 1, imageproc::image::Rgba([0, 0, 0, u8::MAX]));
    let empty = RgbaImage::new(0, 1);
    let small_pixmap = tiny_skia::Pixmap::new(1, 1).ok_or("pixmap allocation failed")?;
    let wider_pixmap = tiny_skia::Pixmap::new(2, 1).ok_or("pixmap allocation failed")?;
    assert!(score_rgba(&one, &two).is_err());
    assert!(score_rgba(&empty, &empty).is_err());
    assert!(score_pixmaps(&small_pixmap, &wider_pixmap).is_err());
    Ok(())
}

#[rstest]
#[case::pixel_count_overflow(u32::MAX, 2)]
#[case::channel_byte_overflow(u32::MAX / CHANNELS as u32 + 1, 1)]
fn rgba_dimension_guard_rejects_overflow_without_allocating(
    #[case] width: u32,
    #[case] height: u32,
) {
    assert!(validate_rgba_dimensions(width, height).is_err());
}

#[derive(Clone, Copy, Debug)]
enum GlyphChange {
    Missing,
    Moved,
    MissingAlpha,
    Color,
}

const CONTROL_SIDE: u32 = 200;
const GLYPH_X: i32 = 60;
const OTHER_GLYPH_X: i32 = 120;
const GLYPH_Y: i32 = 80;
const GLYPH_WIDTH: u32 = 10;
const GLYPH_HEIGHT: u32 = 16;
const GLYPH_STROKE: u32 = 2;
const VERTICAL_STROKES: u32 = 2;
const GLYPH_MIDDLE_Y: i32 = 7;

#[rstest]
#[case::missing(GlyphChange::Missing)]
#[case::moved_one_pixel(GlyphChange::Moved)]
#[case::missing_alpha(GlyphChange::MissingAlpha)]
#[case::color_changed(GlyphChange::Color)]
fn rgba_score_rejects_local_glyph_changes(#[case] change: GlyphChange) -> Result<(), String> {
    let before = glyph_control(None);
    let after = glyph_control(Some(change));
    assert_eq!(
        before.get_pixel(OTHER_GLYPH_X as u32, GLYPH_Y as u32),
        after.get_pixel(OTHER_GLYPH_X as u32, GLYPH_Y as u32)
    );
    assert_control_change(&before, &after, change);
    assert_eq!(score_rgba(&before, &before)?, SCORE_SCALE);
    let score = score_rgba(&before, &after)?;
    assert!(score < MIN_SCORE, "{change:?} unexpectedly scored {score}");
    Ok(())
}

fn assert_control_change(before: &RgbaImage, after: &RgbaImage, change: GlyphChange) {
    let original = before.get_pixel(GLYPH_X as u32, GLYPH_Y as u32).0;
    let candidate = after.get_pixel(GLYPH_X as u32, GLYPH_Y as u32).0;
    match change {
        GlyphChange::MissingAlpha => {
            assert_eq!(&original[..RGB_CHANNELS], &candidate[..RGB_CHANNELS]);
            assert_ne!(original[ALPHA_CHANNEL], candidate[ALPHA_CHANNEL]);
        }
        GlyphChange::Color => {
            assert_eq!(original[ALPHA_CHANNEL], candidate[ALPHA_CHANNEL]);
            assert_ne!(&original[..RGB_CHANNELS], &candidate[..RGB_CHANNELS]);
        }
        GlyphChange::Missing | GlyphChange::Moved => assert_ne!(original, candidate),
    }
}

fn glyph_control(change: Option<GlyphChange>) -> RgbaImage {
    use imageproc::image::Rgba;
    let black = Rgba([0, 0, 0, u8::MAX]);
    let white = Rgba([u8::MAX, u8::MAX, u8::MAX, u8::MAX]);
    let mut image = RgbaImage::from_pixel(CONTROL_SIDE, CONTROL_SIDE, black);
    draw_h(&mut image, OTHER_GLYPH_X, white);
    match change {
        Some(GlyphChange::Missing) => {}
        Some(GlyphChange::Moved) => draw_h(&mut image, GLYPH_X + 1, white),
        Some(GlyphChange::MissingAlpha) => {
            draw_h(&mut image, GLYPH_X, Rgba([u8::MAX, u8::MAX, u8::MAX, 0]));
        }
        Some(GlyphChange::Color) => draw_h(&mut image, GLYPH_X, Rgba([u8::MAX, 0, 0, u8::MAX])),
        None => draw_h(&mut image, GLYPH_X, white),
    }
    image
}

fn draw_h(image: &mut RgbaImage, x: i32, color: imageproc::image::Rgba<u8>) {
    use imageproc::{drawing::draw_filled_rect_mut, rect::Rect};
    let left = Rect::at(x, GLYPH_Y).of_size(GLYPH_STROKE, GLYPH_HEIGHT);
    let right = Rect::at(x + (GLYPH_WIDTH - GLYPH_STROKE) as i32, GLYPH_Y)
        .of_size(GLYPH_STROKE, GLYPH_HEIGHT);
    let middle = Rect::at(x + GLYPH_STROKE as i32, GLYPH_Y + GLYPH_MIDDLE_Y)
        .of_size(GLYPH_WIDTH - VERTICAL_STROKES * GLYPH_STROKE, GLYPH_STROKE);
    for rect in [left, right, middle] {
        draw_filled_rect_mut(image, rect, color);
    }
}
