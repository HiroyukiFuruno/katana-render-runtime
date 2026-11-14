use super::super::{ACTIVE, MAX_CAPTURED_RECORDS, with_actual_route};
use super::{SvgRasterizeOps, tests::render_test_svg};
use tiny_skia::IntRect;

const SIZE: u32 = 32;
const EXTRA_PATHS: usize = 2;
const FILL_PATH: &str = "<path d='M4 4H20V20H4Z' fill='red'/>";
const STROKE_PATH: &str = "<path d='M4 4H20V20H4Z' fill='none' stroke='blue' stroke-width='2'/>";

fn svg(paths: impl IntoIterator<Item = &'static str>) -> String {
    let body = paths.into_iter().collect::<String>();
    format!("<svg xmlns='http://www.w3.org/2000/svg' width='{SIZE}' height='{SIZE}'>{body}</svg>")
}

fn crop() -> Result<IntRect, String> {
    IntRect::from_xywh(0, 0, SIZE, SIZE).ok_or_else(|| "invalid crop".to_owned())
}

#[test]
fn actual_path_record_limit_preserves_pixels_and_paint_absence() -> Result<(), String> {
    let input = svg([FILL_PATH, STROKE_PATH]
        .into_iter()
        .cycle()
        .take(MAX_CAPTURED_RECORDS + EXTRA_PATHS));
    let ordinary = render_test_svg(&input, SIZE)?;
    let (audited, capture) = with_actual_route(|| render_test_svg(&input, SIZE));
    assert_eq!(ordinary.data(), audited?.data());
    assert!(ordinary.data().iter().any(|byte| *byte != 0));
    assert_eq!(capture.path_paints.len(), MAX_CAPTURED_RECORDS);
    assert_eq!(capture.path_paints_dropped, EXTRA_PATHS);
    assert_single_paint_records(&capture.path_paints);
    assert!(capture.crop_stack.is_empty());
    assert!(SvgRasterizeOps::enter_path_crop(crop()?).is_none());
    Ok(())
}

fn assert_single_paint_records(records: &[serde_json::Value]) {
    assert!(records[0]["stroke"].is_null());
    assert!(records[0]["fill"].is_object());
    assert!(records[1]["fill"].is_null());
    assert!(records[1]["stroke"].is_object());
}

fn exercise_overflow(input: &str, crop: IntRect) -> Result<(), String> {
    let mut guards = (0..=MAX_CAPTURED_RECORDS)
        .map(|_| SvgRasterizeOps::enter_path_crop(crop).ok_or("missing crop scope"))
        .collect::<Result<Vec<_>, _>>()?;
    render_test_svg(input, SIZE)?;
    drop(guards.pop());
    render_test_svg(input, SIZE)?;
    guards.clear();
    render_test_svg(input, SIZE)?;
    Ok(())
}

#[test]
fn overflowing_real_crop_scopes_suppress_records_and_restore() -> Result<(), String> {
    let input = svg([FILL_PATH]);
    let (result, capture) = with_actual_route(|| exercise_overflow(&input, crop()?));
    result?;
    assert_eq!(capture.path_paints_dropped, 1);
    assert_eq!(capture.path_paints.len(), 2);
    assert_eq!(
        capture.path_paints[0]["crops"].as_array().map(Vec::len),
        Some(MAX_CAPTURED_RECORDS)
    );
    assert_eq!(
        capture.path_paints[1]["crops"].as_array().map(Vec::len),
        Some(0)
    );
    assert!(capture.crop_stack.is_empty());
    assert_eq!(capture.crop_overflow, 0);
    assert!(SvgRasterizeOps::enter_path_crop(crop()?).is_none());
    Ok(())
}

#[test]
fn missing_path_capture_preserves_real_svg_pixels_and_recovers() -> Result<(), String> {
    let input = svg([FILL_PATH]);
    let crop = crop()?;
    let ordinary = render_test_svg(&input, SIZE)?;
    let (rendered, missing) = with_actual_route(|| {
        let guard = SvgRasterizeOps::enter_path_crop(crop).ok_or("crop scope missing")?;
        ACTIVE.with(|active| active.take());
        drop(guard);
        assert!(SvgRasterizeOps::enter_path_crop(crop).is_none());
        render_test_svg(&input, SIZE)
    });
    assert_eq!(ordinary.data(), rendered?.data());
    assert!(missing.path_paints.is_empty());
    let (rendered, next) = with_actual_route(|| render_test_svg(&input, SIZE));
    assert_eq!(ordinary.data(), rendered?.data());
    assert_eq!(next.path_paints.len(), 1);
    Ok(())
}
