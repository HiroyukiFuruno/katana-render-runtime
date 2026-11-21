use super::super::super::html_debug_trace::HtmlDebugTrace;
use super::super::{HtmlBrowserError, HtmlInteractiveSession};
use crate::{HtmlBrowserSource, HtmlBrowserViewport};

const VIEWPORT_WIDTH: u32 = 320;
const VIEWPORT_HEIGHT: u32 = 240;
const HTML: &str = "<body style='margin:0'><div style='text-spacing-trim:space-all'><p>「日本語」 Latin</p><span>inline text</span></div></body>";

fn open() -> Result<HtmlInteractiveSession, HtmlBrowserError> {
    HtmlInteractiveSession::start(
        HtmlBrowserSource::new(HTML, "https://example.test/traced-layout.html")?,
        HtmlBrowserViewport::new(VIEWPORT_WIDTH, VIEWPORT_HEIGHT, 1.0)?,
    )
}

#[test]
fn traced_space_all_refresh_preserves_real_frame_and_reports_existing_flow_statistics()
-> Result<(), String> {
    let mut session = open().map_err(|error| error.to_string())?;
    let snapshot = session.latest_frame().cloned().ok_or("initial frame")?;
    session.trace = HtmlDebugTrace::enabled_for_test();
    session.refresh_frame().map_err(|error| error.to_string())?;
    let frame = session.latest_frame().ok_or("refreshed frame")?;
    let metrics = session.latest_scroll_metrics().ok_or("refreshed metrics")?;
    assert_eq!(frame.generation, snapshot.generation + 1);
    assert_eq!(frame.generation, metrics.generation);
    assert_eq!(frame.pixels, snapshot.pixels);
    assert_eq!(
        session.flow_stats(),
        session.flow_measurements.borrow().stats()
    );
    Ok(())
}
