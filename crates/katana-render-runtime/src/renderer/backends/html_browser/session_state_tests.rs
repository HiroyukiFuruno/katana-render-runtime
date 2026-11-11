use super::tests::{test_source, viewport};
use super::*;

type TestResult<T = ()> = Result<T, String>;
const TEST_VIEWPORT_WIDTH: u32 = 160;
const TEST_VIEWPORT_HEIGHT: u32 = 120;
const TEST_POINTER_COORDINATE: f32 = 20.0;

#[test]
fn session_creation_rejects_invalid_viewport_before_starting_runtime() -> TestResult {
    let source = test_source("https://example.test/invalid-viewport.html")?;
    let viewport = HtmlBrowserViewport {
        width: 0,
        height: TEST_VIEWPORT_HEIGHT,
        device_scale_factor: 1.0,
    };
    assert!(matches!(
        HtmlBrowserSession::new(source, viewport),
        Err(HtmlBrowserError::InvalidViewport)
    ));
    Ok(())
}

#[test]
fn session_creation_revalidates_mutated_source() -> TestResult {
    let mut source = test_source("https://example.test/oversized.html")?;
    let max = super::super::HTML_BROWSER_MAX_SOURCE_BYTES;
    source.raw_html = "x".repeat(max + 1);
    assert!(matches!(
        HtmlBrowserSession::new(source, viewport(TEST_VIEWPORT_WIDTH, TEST_VIEWPORT_HEIGHT)?),
        Err(HtmlBrowserError::SourceTooLarge { actual_bytes, max_bytes })
            if actual_bytes == max + 1 && max_bytes == max
    ));
    Ok(())
}

#[test]
fn session_rejects_invalid_input_and_out_of_order_internal_frames() -> TestResult {
    let mut session = HtmlBrowserSession::new(
        test_source("https://example.test/a.html")?,
        viewport(TEST_VIEWPORT_WIDTH, TEST_VIEWPORT_HEIGHT)?,
    )
    .map_err(|error| error.to_string())?;
    assert_validation_errors(&mut session);
    assert_sync_rejects_out_of_order_frame(&mut session)?;
    assert_missing_runtime_and_closed_state(&mut session)?;
    Ok(())
}

fn assert_validation_errors(session: &mut HtmlBrowserSession) {
    let invalid_viewport = HtmlBrowserViewport {
        width: 0,
        height: TEST_VIEWPORT_HEIGHT,
        device_scale_factor: 1.0,
    };
    assert_eq!(
        session.resize(invalid_viewport),
        Err(HtmlBrowserError::InvalidViewport)
    );
    assert_eq!(
        session.dispatch_input(HtmlBrowserInput::PointerMove {
            x: f32::NAN,
            y: TEST_POINTER_COORDINATE,
        }),
        Err(HtmlBrowserError::InvalidInputCoordinates)
    );
}

fn assert_sync_rejects_out_of_order_frame(session: &mut HtmlBrowserSession) -> TestResult {
    let initial = session
        .latest_frame()
        .cloned()
        .ok_or_else(|| "initial frame must exist".to_string())?;
    let unaccepted = refresh_unaccepted_frame(session)?;
    let public_ahead = frame_ahead_of(&initial, &unaccepted)?;

    assert_sync_rejects_unaccepted_frame(session, &unaccepted, &public_ahead);
    assert_retry_accepts_unaccepted_frame(session, &initial, &unaccepted, &public_ahead)?;
    Ok(())
}

fn refresh_unaccepted_frame(session: &mut HtmlBrowserSession) -> TestResult<HtmlBrowserFrame> {
    let interactive = session
        .interactive
        .as_mut()
        .ok_or_else(|| "interactive runtime must exist".to_string())?;
    interactive
        .refresh_frame()
        .map_err(|error| error.to_string())?;
    interactive
        .latest_frame()
        .cloned()
        .ok_or_else(|| "refreshed inner frame must exist".to_string())
}

fn frame_ahead_of(
    initial: &HtmlBrowserFrame,
    unaccepted: &HtmlBrowserFrame,
) -> Result<HtmlBrowserFrame, String> {
    HtmlBrowserFrame::new(
        unaccepted.generation + 1,
        initial.origin.clone(),
        initial.viewport,
        initial.pixel_format,
        initial.pixels.clone(),
    )
    .map_err(|error| error.to_string())
}

fn assert_sync_rejects_unaccepted_frame(
    session: &mut HtmlBrowserSession,
    unaccepted: &HtmlBrowserFrame,
    public_ahead: &HtmlBrowserFrame,
) {
    session.latest_frame = Some(public_ahead.clone());
    assert!(matches!(
        session.sync_interactive_state(),
        Err(HtmlBrowserError::StaleFrameGeneration { .. })
    ));
    assert_eq!(session.latest_frame(), Some(public_ahead));
    assert_eq!(
        session
            .interactive
            .as_ref()
            .and_then(HtmlInteractiveSession::latest_frame),
        Some(unaccepted)
    );
}

fn assert_retry_accepts_unaccepted_frame(
    session: &mut HtmlBrowserSession,
    initial: &HtmlBrowserFrame,
    unaccepted: &HtmlBrowserFrame,
    public_ahead: &HtmlBrowserFrame,
) -> TestResult {
    session.latest_frame = Some(initial.clone());
    session
        .sync_interactive_state()
        .map_err(|error| error.to_string())?;
    let accepted = session
        .latest_frame()
        .ok_or_else(|| "retried frame must be accepted".to_string())?;
    assert_eq!(accepted.generation, unaccepted.generation);
    assert_eq!(accepted.pixels, unaccepted.pixels);
    assert!(
        session
            .interactive
            .as_ref()
            .is_some_and(|interactive| interactive.latest_frame().is_none())
    );
    assert_eq!(initial.generation + 1, accepted.generation);
    assert!(session.latest_frame().is_some_and(|frame| {
        frame.generation == accepted.generation && frame.pixels == unaccepted.pixels
    }));
    assert_snapshot_pixels(initial, accepted);
    assert_eq!(public_ahead.generation, unaccepted.generation + 1);
    assert_snapshot_pixels(public_ahead, initial);
    Ok(())
}

fn assert_snapshot_pixels(snapshot: &HtmlBrowserFrame, expected: &HtmlBrowserFrame) {
    assert_eq!(snapshot.pixels, expected.pixels);
}

fn assert_missing_runtime_and_closed_state(session: &mut HtmlBrowserSession) -> TestResult {
    let initial = session
        .latest_frame()
        .cloned()
        .ok_or_else(|| "initial frame must exist".to_string())?;
    session.interactive = None;
    assert_eq!(
        session.advance_time(std::time::Duration::from_millis(1)),
        Err(HtmlBrowserError::RuntimeNotStarted)
    );
    assert_eq!(
        session.dispatch_input(HtmlBrowserInput::Focus { focused: true }),
        Err(HtmlBrowserError::RuntimeNotStarted)
    );
    assert_eq!(
        session.sync_interactive_state(),
        Err(HtmlBrowserError::RuntimeNotStarted)
    );
    session.close().map_err(|error| error.to_string())?;
    assert_eq!(
        session.accept_frame(initial),
        Err(HtmlBrowserError::SessionClosed)
    );
    Ok(())
}

#[test]
fn close_releases_owned_source_frame_and_pending_navigation() -> TestResult {
    let mut session = HtmlBrowserSession::new(
        test_source("https://example.test/close.html")?,
        viewport(TEST_VIEWPORT_WIDTH, TEST_VIEWPORT_HEIGHT)?,
    )
    .map_err(|error| error.to_string())?;
    assert!(!session.source().raw_html.is_empty());
    assert!(session.latest_frame().is_some());
    session.pending_navigation = Some(
        HtmlBrowserNavigationEvent::new("https://example.test/next.html")
            .map_err(|error| error.to_string())?,
    );

    session.close().map_err(|error| error.to_string())?;

    assert!(session.source().raw_html.is_empty());
    assert_eq!(session.source().raw_html.capacity(), 0);
    assert!(session.latest_frame().is_none());
    assert!(session.take_frame_update().is_none());
    assert!(session.take_navigation().is_none());
    session.close().map_err(|error| error.to_string())?;
    Ok(())
}
