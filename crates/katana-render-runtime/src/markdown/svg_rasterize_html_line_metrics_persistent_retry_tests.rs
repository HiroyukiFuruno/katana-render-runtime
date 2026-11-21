use super::super::super::super::font::font_source_generation;
use super::super::super::super::font_file_fixture::{
    self as shared_font_file_fixture, FontFileFixture,
};
use super::{
    FAMILY, Request, SIZE, TEXT, WEIGHT, assert_repaint, clear_persistent, first_metrics,
    observed_metrics, with_render_scoped_stamp_batch,
};
use std::sync::atomic::{AtomicUsize, Ordering};

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);
type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn observed_file_change_retries_without_reusing_previous_validated_metrics() -> TestResult<()> {
    let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "krr-line-metrics-token-{}-{sequence}.ttf",
        std::process::id()
    ));
    let fixture = shared_font_file_fixture::create(path, FONT_BYTES)?;
    clear_persistent();
    let (expected, reusable) = with_render_scoped_stamp_batch(first_metrics)?;
    let mut attempts = 0;
    let actual = with_render_scoped_stamp_batch(|| {
        attempts += 1;
        retry_attempt(&fixture, expected, reusable, attempts)
    })?;
    assert_eq!(actual.0, expected);
    assert_eq!(attempts, if actual.1 { 2 } else { 1 });
    clear_persistent();
    Ok(())
}

fn retry_attempt(
    fixture: &FontFileFixture,
    expected: (f32, f32),
    reusable: bool,
    attempt: usize,
) -> TestResult<((f32, f32), bool)> {
    let (actual, stats) = observed_metrics()?;
    assert_eq!(actual, expected);
    if attempt == 1 {
        if !font_source_generation(&fixture.database, fixture.face_id).durable_reusable() {
            return Ok((actual, false));
        }
        assert_repaint(&stats, reusable);
        let mut bytes = FONT_BYTES.to_vec();
        bytes.push(0);
        std::fs::write(&fixture.path, bytes)?;
    } else {
        assert!(Request::new(TEXT, FAMILY, SIZE, WEIGHT, false).is_none());
        assert_repaint(&stats, false);
    }
    Ok((actual, true))
}
