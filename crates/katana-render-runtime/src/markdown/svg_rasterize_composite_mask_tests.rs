use super::mask::composite_crop_with_mask;
use crate::{HtmlBrowserSession, HtmlBrowserSource, HtmlBrowserViewport};

#[path = "svg_rasterize_composite_mask_aa_control_tests.rs"]
mod aa_control;

const WIDTH: u32 = 4;
const HEIGHT: u32 = 3;
const CANVAS_WIDTH: u32 = 6;
const CANVAS_HEIGHT: u32 = 5;
const CROP_X: i32 = 2;
const CROP_Y: i32 = 1;
const CROP_WIDTH: u32 = 2;
const CROP_HEIGHT: u32 = 2;
const MASK_MAX: u8 = 255;
const MIDPOINT: u8 = 127;
const OPAQUE_SOURCE: [u8; 4] = [80, 40, 20, 255];
const OPAQUE_BACKDROP: [u8; 4] = [20, 40, 80, 255];
const PARTIAL_SOURCE: [u8; 4] = [6, 4, 2, 128];
const PARTIAL_BACKDROP: [u8; 4] = [1, 3, 2, 64];
const FRACTIONAL_SIZE: u32 = 48;
const FRACTIONAL_BACKDROP: [u8; 4] = [30, 30, 30, 255];

#[test]
fn identical_sources_preserve_every_mask_coverage() -> Result<(), String> {
    let crop = full_canvas_crop()?;
    for color in [FRACTIONAL_BACKDROP, PARTIAL_SOURCE] {
        let source = solid_pixmap(color)?;
        for coverage in 0..=MASK_MAX {
            let mut actual = source.clone();
            composite_crop_with_mask(&mut actual.as_mut(), crop, &source, &mask_for(coverage)?)?;
            assert_pixel(&actual, color, coverage)?;
        }
    }
    Ok(())
}

#[test]
fn distinct_premultiplied_source_matches_single_round_formula() -> Result<(), String> {
    let crop = full_canvas_crop()?;
    let source = solid_pixmap(OPAQUE_SOURCE)?;
    let backdrop = solid_pixmap(OPAQUE_BACKDROP)?;
    for coverage in 0..=MASK_MAX {
        let mut actual = backdrop.clone();
        composite_crop_with_mask(&mut actual.as_mut(), crop, &source, &mask_for(coverage)?)?;
        let expected = expected_pixel(OPAQUE_BACKDROP, OPAQUE_SOURCE, coverage);
        assert_pixel(&actual, expected, coverage)?;
    }
    Ok(())
}

#[test]
fn transparent_premultiplied_source_matches_formula_at_all_coverages() -> Result<(), String> {
    let crop = full_canvas_crop()?;
    let source = solid_pixmap(PARTIAL_SOURCE)?;
    let backdrop = solid_pixmap(PARTIAL_BACKDROP)?;
    for coverage in 0..=MASK_MAX {
        let mut actual = backdrop.clone();
        composite_crop_with_mask(&mut actual.as_mut(), crop, &source, &mask_for(coverage)?)?;
        let expected = expected_pixel(PARTIAL_BACKDROP, PARTIAL_SOURCE, coverage);
        assert_pixel(&actual, expected, coverage)?;
    }
    Ok(())
}

#[test]
fn mismatched_dimensions_are_rejected_without_pixel_changes() -> Result<(), String> {
    let mut backdrop = solid_pixmap(OPAQUE_BACKDROP)?;
    let before = backdrop.data().to_vec();
    let short = tiny_skia::Pixmap::new(WIDTH - 1, HEIGHT).ok_or("failed short pixmap")?;
    let mask = mask_for(MIDPOINT)?;
    if composite_crop_with_mask(&mut backdrop.as_mut(), full_canvas_crop()?, &short, &mask).is_ok()
    {
        return Err("mismatched dimensions unexpectedly accepted".to_string());
    }
    if backdrop.data() != before {
        return Err("dimension error changed backdrop".to_string());
    }
    Ok(())
}

fn full_canvas_crop() -> Result<tiny_skia::IntRect, String> {
    tiny_skia::IntRect::from_xywh(0, 0, WIDTH, HEIGHT).ok_or("invalid full canvas fixture".into())
}

#[test]
fn cropped_composite_preserves_nonzero_guards_for_all_coverages() -> Result<(), String> {
    for (source_color, guard_color) in [
        (OPAQUE_SOURCE, OPAQUE_BACKDROP),
        (PARTIAL_SOURCE, PARTIAL_BACKDROP),
        (PARTIAL_SOURCE, PARTIAL_SOURCE),
    ] {
        let source = solid_pixmap_size(CROP_WIDTH, CROP_HEIGHT, source_color)?;
        for coverage in 0..=MASK_MAX {
            let mut actual = solid_pixmap_size(CANVAS_WIDTH, CANVAS_HEIGHT, guard_color)?;
            let mask = mask_for_size(CROP_WIDTH, CROP_HEIGHT, coverage)?;
            let crop = tiny_skia::IntRect::from_xywh(CROP_X, CROP_Y, CROP_WIDTH, CROP_HEIGHT)
                .ok_or("invalid crop")?;
            let mut actual_mut = actual.as_mut();
            composite_crop_with_mask(&mut actual_mut, crop, &source, &mask)?;
            let expected = expected_pixel(guard_color, source_color, coverage);
            assert_crop(&actual, CROP_X, CROP_Y, expected)?;
            assert_guards(&actual, guard_color)?;
        }
    }
    Ok(())
}

#[test]
fn invalid_crops_are_rejected_without_mutation() -> Result<(), String> {
    let source = solid_pixmap_size(CROP_WIDTH, CROP_HEIGHT, OPAQUE_SOURCE)?;
    let mask = mask_for_size(CROP_WIDTH, CROP_HEIGHT, MIDPOINT)?;
    let short_source = solid_pixmap_size(CROP_WIDTH - 1, CROP_HEIGHT, OPAQUE_SOURCE)?;
    let short_mask = mask_for_size(CROP_WIDTH - 1, CROP_HEIGHT, MIDPOINT)?;
    for crop in [
        tiny_skia::IntRect::from_xywh(-1, CROP_Y, CROP_WIDTH, CROP_HEIGHT),
        tiny_skia::IntRect::from_xywh(CROP_X, -1, CROP_WIDTH, CROP_HEIGHT),
        tiny_skia::IntRect::from_xywh(CANVAS_WIDTH as i32 - 1, CROP_Y, CROP_WIDTH, CROP_HEIGHT),
        tiny_skia::IntRect::from_xywh(CROP_X, CANVAS_HEIGHT as i32 - 1, CROP_WIDTH, CROP_HEIGHT),
    ] {
        assert_invalid_crop(crop.ok_or("invalid test crop")?, &source, &mask)?;
    }
    let valid_crop = tiny_skia::IntRect::from_xywh(CROP_X, CROP_Y, CROP_WIDTH, CROP_HEIGHT)
        .ok_or("invalid test crop")?;
    assert_invalid_crop(valid_crop, &short_source, &mask)?;
    assert_invalid_crop(valid_crop, &source, &short_mask)
}

fn assert_invalid_crop(
    crop: tiny_skia::IntRect,
    source: &tiny_skia::Pixmap,
    mask: &tiny_skia::Mask,
) -> Result<(), String> {
    let mut actual = solid_pixmap_size(CANVAS_WIDTH, CANVAS_HEIGHT, OPAQUE_BACKDROP)?;
    let before = actual.data().to_vec();
    let mut actual_mut = actual.as_mut();
    if composite_crop_with_mask(&mut actual_mut, crop, source, mask).is_ok() {
        return Err("invalid crop unexpectedly accepted".to_string());
    }
    if actual.data() != before {
        return Err("invalid crop changed destination".to_string());
    }
    Ok(())
}

#[test]
fn fractional_generated_clip_preserves_uniform_backdrop() -> Result<(), String> {
    let clipped = render_html_frame(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48"><defs><clipPath id="clip"><rect x="4.296875" y="4.296875" width="32" height="32"/></clipPath></defs><g clip-path="url(#clip)"><rect width="48" height="48" fill="#1e1e1e"/></g></svg>"##,
    )?;
    let background_only =
        render_html_frame(r##"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48"/>"##)?;
    for (case, pixels) in [
        ("fractional clip", clipped),
        ("background only", background_only),
    ] {
        if let Some((index, pixel)) = pixels
            .chunks_exact(RGBA_CHANNELS)
            .enumerate()
            .find(|(_, pixel)| *pixel != FRACTIONAL_BACKDROP)
        {
            let x = index as u32 % FRACTIONAL_SIZE;
            let y = index as u32 / FRACTIONAL_SIZE;
            return Err(format!(
                "{case} changed uniform fractional backdrop at ({x}, {y}): expected {FRACTIONAL_BACKDROP:?}, actual [{}, {}, {}, {}]",
                pixel[0], pixel[1], pixel[2], pixel[3]
            ));
        }
    }
    Ok(())
}

fn mask_for(coverage: u8) -> Result<tiny_skia::Mask, String> {
    mask_for_size(WIDTH, HEIGHT, coverage)
}

fn mask_for_size(width: u32, height: u32, coverage: u8) -> Result<tiny_skia::Mask, String> {
    tiny_skia::Mask::from_vec(
        vec![coverage; (width * height) as usize],
        tiny_skia::IntSize::from_wh(width, height).ok_or("invalid mask size")?,
    )
    .ok_or("failed to create mask".to_string())
}

fn solid_pixmap(color: [u8; RGBA_CHANNELS]) -> Result<tiny_skia::Pixmap, String> {
    solid_pixmap_size(WIDTH, HEIGHT, color)
}

fn solid_pixmap_size(
    width: u32,
    height: u32,
    color: [u8; RGBA_CHANNELS],
) -> Result<tiny_skia::Pixmap, String> {
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or("failed to create pixmap")?;
    for pixel in pixmap.data_mut().chunks_exact_mut(RGBA_CHANNELS) {
        pixel.copy_from_slice(&color);
    }
    Ok(pixmap)
}

fn assert_crop(
    pixmap: &tiny_skia::Pixmap,
    x: i32,
    y: i32,
    expected: [u8; RGBA_CHANNELS],
) -> Result<(), String> {
    for row in 0..CROP_HEIGHT {
        let start = (((y as u32 + row) * CANVAS_WIDTH + x as u32) * RGBA_CHANNELS as u32) as usize;
        let end = start + (CROP_WIDTH as usize * RGBA_CHANNELS);
        if pixmap.data()[start..end]
            .chunks_exact(RGBA_CHANNELS)
            .any(|pixel| pixel != expected)
        {
            return Err("cropped pixels differ".to_string());
        }
    }
    Ok(())
}

fn assert_guards(pixmap: &tiny_skia::Pixmap, expected: [u8; RGBA_CHANNELS]) -> Result<(), String> {
    for (index, pixel) in pixmap.data().chunks_exact(RGBA_CHANNELS).enumerate() {
        let x = (index as u32) % CANVAS_WIDTH;
        let y = (index as u32) / CANVAS_WIDTH;
        let in_crop = (CROP_X as u32..CROP_X as u32 + CROP_WIDTH).contains(&x)
            && (CROP_Y as u32..CROP_Y as u32 + CROP_HEIGHT).contains(&y);
        if !in_crop && pixel != expected {
            return Err("crop guard pixel changed".to_string());
        }
    }
    Ok(())
}

fn expected_pixel(
    backdrop: [u8; RGBA_CHANNELS],
    source: [u8; RGBA_CHANNELS],
    coverage: u8,
) -> [u8; RGBA_CHANNELS] {
    let coverage = f64::from(coverage) / f64::from(MASK_MAX);
    std::array::from_fn(|index| {
        let start = f64::from(backdrop[index]);
        (start + (f64::from(source[index]) - start) * coverage).round() as u8
    })
}

fn assert_pixel(
    pixmap: &tiny_skia::Pixmap,
    expected: [u8; RGBA_CHANNELS],
    coverage: u8,
) -> Result<(), String> {
    if pixmap
        .data()
        .chunks_exact(RGBA_CHANNELS)
        .any(|pixel| pixel != expected)
    {
        return Err(format!("unexpected pixel at coverage {coverage}"));
    }
    Ok(())
}

fn render_html_frame(svg: &str) -> Result<Vec<u8>, String> {
    let html = format!(
        "<style>html,body{{margin:0;width:48px;height:48px;background:#1e1e1e}}svg{{display:block}}</style>{svg}"
    );
    let source = HtmlBrowserSource::new(html, "https://example.test/clip-identity.html")
        .map_err(|error| error.to_string())?;
    let viewport = HtmlBrowserViewport::new(FRACTIONAL_SIZE, FRACTIONAL_SIZE, 1.0)
        .map_err(|error| error.to_string())?;
    let session = HtmlBrowserSession::new(source, viewport).map_err(|error| error.to_string())?;
    session
        .latest_frame()
        .map(|frame| frame.pixels.clone())
        .ok_or_else(|| "fractional clip frame missing".to_owned())
}

const RGBA_CHANNELS: usize = 4;
