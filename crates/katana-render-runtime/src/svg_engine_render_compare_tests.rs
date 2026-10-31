use tiny_skia::Pixmap;

const CHANNELS: usize = 4;
const ALPHA: usize = 3;
const MAX_ALPHA: f64 = 255.0;
const ROUNDING_OFFSET: f64 = 0.5;
const CHANNEL_THRESHOLD: u8 = 1;
const DIAGNOSTIC_PIXEL_LIMIT: usize = 8;

pub(super) fn pixel_differences(actual: &Pixmap, png: &[u8]) -> Result<usize, String> {
    let expected = image::load_from_memory(png)
        .map_err(|error| error.to_string())?
        .to_rgba8();
    let width = actual.width().max(expected.width());
    let height = actual.height().max(expected.height());
    let mut differences = 0;
    for y in 0..height {
        for x in 0..width {
            let actual_pixel = actual.pixel(x, y).map(unpremultiply);
            let expected_pixel =
                (x < expected.width() && y < expected.height()).then(|| expected.get_pixel(x, y).0);
            if differs(actual_pixel, expected_pixel) {
                report_difference(x, y, actual_pixel, expected_pixel, differences);
                differences += 1;
            }
        }
    }
    Ok(differences)
}

fn report_difference(
    x: u32,
    y: u32,
    actual: Option<[u8; CHANNELS]>,
    expected: Option<[u8; CHANNELS]>,
    index: usize,
) {
    if index < DIAGNOSTIC_PIXEL_LIMIT {
        eprintln!("pixel ({x},{y}) differs: actual={actual:?} expected={expected:?}");
    }
}

fn unpremultiply(pixel: tiny_skia::PremultipliedColorU8) -> [u8; CHANNELS] {
    let alpha = pixel.alpha();
    let a = f64::from(alpha) / MAX_ALPHA;
    [
        (f64::from(pixel.red()) / a + ROUNDING_OFFSET) as u8,
        (f64::from(pixel.green()) / a + ROUNDING_OFFSET) as u8,
        (f64::from(pixel.blue()) / a + ROUNDING_OFFSET) as u8,
        alpha,
    ]
}

fn differs(actual: Option<[u8; CHANNELS]>, expected: Option<[u8; CHANNELS]>) -> bool {
    let (Some(actual), Some(expected)) = (actual, expected) else {
        return true;
    };
    if actual[ALPHA] == 0 && expected[ALPHA] == 0 {
        return false;
    }
    actual
        .into_iter()
        .zip(expected)
        .any(|(a, b)| a.abs_diff(b) > CHANNEL_THRESHOLD)
}

#[test]
fn comparison_preserves_upstream_threshold_and_transparent_rule() {
    const OPAQUE: [u8; CHANNELS] = [10, 20, 30, 255];
    const ONE_STEP: [u8; CHANNELS] = [11, 20, 30, 255];
    const TWO_STEPS: [u8; CHANNELS] = [12, 20, 30, 255];
    assert!(!differs(Some(OPAQUE), Some(ONE_STEP)));
    assert!(differs(Some(OPAQUE), Some(TWO_STEPS)));
    assert!(!differs(Some([0, 0, 0, 0]), Some([255, 255, 255, 0])));
    assert!(differs(Some(OPAQUE), None));
}

#[test]
fn pixmap_comparison_counts_each_differing_pixel() -> Result<(), String> {
    const WIDTH: u32 = 10;
    let mut actual = Pixmap::new(WIDTH, 1).ok_or("actual pixmap unavailable")?;
    actual.fill(tiny_skia::Color::from_rgba8(2, 0, 0, 255));
    let mut expected = Pixmap::new(WIDTH, 1).ok_or("expected pixmap unavailable")?;
    expected.fill(tiny_skia::Color::from_rgba8(0, 0, 0, 255));
    let expected_png = expected.encode_png().map_err(|error| error.to_string())?;
    let expected_differences = usize::try_from(WIDTH).map_err(|error| error.to_string())?;
    assert_eq!(
        pixel_differences(&actual, &expected_png)?,
        expected_differences
    );
    Ok(())
}
