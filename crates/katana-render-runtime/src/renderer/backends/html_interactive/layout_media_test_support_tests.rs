use super::{TestResult, capture_frame_pixels, session_frame_pixels};
use crate::{HtmlBrowserSession, HtmlBrowserSource, HtmlBrowserViewport};

const VIEWPORT_SIZE: u32 = 16;

#[test]
fn invalid_viewport_is_reported() -> TestResult {
    let result = capture_frame_pixels("<svg/>", "https://example.test/invalid.svg", 0, 1);
    let error = result.err().ok_or("invalid viewport was accepted")?;
    assert_eq!(
        error.to_string(),
        crate::HtmlBrowserError::InvalidViewport.to_string()
    );
    Ok(())
}

#[test]
fn closed_session_has_no_frame() -> TestResult {
    let source = HtmlBrowserSource::new(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"16\" height=\"16\"/>",
        "https://example.test/closed.svg",
    )?;
    let viewport = HtmlBrowserViewport::new(VIEWPORT_SIZE, VIEWPORT_SIZE, 1.0)?;
    let mut session = HtmlBrowserSession::new(source, viewport)?;
    session.close()?;

    let error = session_frame_pixels(&session)
        .err()
        .ok_or("closed session retained a frame")?;
    assert_eq!(error.to_string(), "missing SVG frame");
    Ok(())
}
