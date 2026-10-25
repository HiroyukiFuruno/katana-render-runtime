#![cfg(test)]

use super::{measure_full_y, prepare_top_scroll, scroll_to_bottom, set_max_frame_elapsed_ms};
use crate::{HtmlBrowserSession, HtmlBrowserViewport};
#[cfg(feature = "baseline")]
use baseline::{HtmlBrowserInput, HtmlBrowserSource, HtmlRuntime};
#[cfg(feature = "candidate")]
use candidate::{HtmlBrowserInput, HtmlBrowserSource, HtmlRuntime};

const TALL_PAGE_VIEWPORT_WIDTH: u32 = 160;
const TALL_PAGE_VIEWPORT_HEIGHT: u32 = 120;

fn open_tall_session() -> Result<HtmlBrowserSession, Box<dyn std::error::Error>> {
    let viewport =
        HtmlBrowserViewport::new(TALL_PAGE_VIEWPORT_WIDTH, TALL_PAGE_VIEWPORT_HEIGHT, 1.0)?;
    let html = "<style>body{margin:0}</style><div style='height:400px'></div>";
    let source = HtmlBrowserSource::new(html.to_owned(), "https://example.test/full-y.html")?;
    Ok(HtmlRuntime.open(source, viewport)?)
}

fn dispatch_scroll(
    session: &mut HtmlBrowserSession,
    delta_y: f32,
) -> Result<(), Box<dyn std::error::Error>> {
    session.dispatch_input(HtmlBrowserInput::Scroll {
        delta_x: 0.0,
        delta_y,
    })?;
    Ok(())
}

#[test]
fn full_y_records_bottom_frame_and_returns_real_session_to_top(
) -> Result<(), Box<dyn std::error::Error>> {
    for height in [80, 1800] {
        let viewport =
            HtmlBrowserViewport::new(TALL_PAGE_VIEWPORT_WIDTH, TALL_PAGE_VIEWPORT_HEIGHT, 1.0)?;
        let html = format!(
            "<style>body{{margin:0}}</style><div style='height:{height}px;background:#abcdef'></div>"
        );
        let source = HtmlBrowserSource::new(html, "https://example.test/full-y.html")?;
        let mut session = HtmlRuntime.open(source, viewport)?;
        let (measurement, steps, final_y, max_scroll) = measure_full_y(&mut session, viewport)?;
        assert_eq!(steps > 0, max_scroll > 0.0);
        assert_eq!(final_y, max_scroll);
        assert_eq!(
            measurement["frame"]["scroll_y"].as_f64(),
            Some(f64::from(max_scroll))
        );
        assert_full_y_measurement(&measurement, &session, max_scroll)?;
        session.close()?;
    }
    Ok(())
}

fn assert_full_y_measurement(
    measurement: &serde_json::Value,
    session: &HtmlBrowserSession,
    max_scroll: f32,
) -> Result<(), Box<dyn std::error::Error>> {
    let elapsed = measurement["elapsed_ms"]
        .as_f64()
        .ok_or("missing elapsed")?;
    let maximum = measurement["max_frame_elapsed_ms"]
        .as_f64()
        .ok_or("missing maximum frame time")?;
    assert!(elapsed.is_finite() && maximum.is_finite());
    assert!(maximum >= 0.0 && elapsed >= maximum);
    assert!(
        measurement["peak_rss_bytes"]
            .as_u64()
            .ok_or("missing RSS")?
            > 0
    );
    let top_frame = session.latest_frame().ok_or("missing restored top frame")?;
    let bottom_generation = measurement["frame"]["generation"]
        .as_u64()
        .ok_or("missing captured bottom generation")?;
    assert_eq!(top_frame.scroll_y, 0.0);
    assert_eq!(top_frame.generation > bottom_generation, max_scroll > 0.0);
    Ok(())
}

#[test]
fn full_y_helpers_reject_missing_progress_and_incomplete_top_restore(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut session = open_tall_session()?;
    let mut scroll_y = 0.0;
    let mut steps = 0;
    let stalled = scroll_to_bottom(&mut session, &mut scroll_y, 10_000.0, &mut steps);
    assert_eq!(
        stalled
            .err()
            .ok_or("scroll unexpectedly reached requested extent")?
            .to_string(),
        "full-Y scroll made no progress before bottom"
    );
    assert!(steps > 0 && scroll_y > 0.0);
    session.close()?;
    let mut session = open_tall_session()?;
    dispatch_scroll(&mut session, 1_000.0)?;
    let actual_scroll = session
        .latest_frame()
        .ok_or("missing scrolled frame")?
        .scroll_y;
    assert!(actual_scroll > 1.0);
    assert_eq!(
        prepare_top_scroll(&mut session, actual_scroll / 2.0)
            .err()
            .ok_or("undersized restore delta reached top")?
            .to_string(),
        "full-Y preparation did not return to scroll origin"
    );
    session.close()?;
    Ok(())
}

#[test]
fn max_frame_time_requires_a_json_object() {
    let mut measurement = serde_json::Value::Null;
    assert_eq!(
        set_max_frame_elapsed_ms(&mut measurement, 1.0)
            .err()
            .map(|error| error.to_string()),
        Some("full-Y measurement must be a JSON object".to_owned())
    );
}
