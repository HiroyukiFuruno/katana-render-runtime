use super::*;
use crate::{HTML_BROWSER_MAX_SOURCE_BYTES, HtmlBrowserOrigin};

type TestResult<T = ()> = Result<T, String>;
const TEST_VIEWPORT_WIDTH: u32 = 160;
const TEST_VIEWPORT_HEIGHT: u32 = 120;
const RESIZED_VIEWPORT_WIDTH: u32 = 240;
const RESIZED_VIEWPORT_HEIGHT: u32 = 180;
const TEST_POINTER_COORDINATE: f32 = 20.0;

#[test]
fn session_starts_an_in_process_runtime_with_an_initial_frame() -> TestResult {
    let mut session = HtmlBrowserSession::new(
        test_source("https://example.test/a.html")?,
        viewport(TEST_VIEWPORT_WIDTH, TEST_VIEWPORT_HEIGHT)?,
    )
    .map_err(|error| error.to_string())?;

    assert_eq!(session.state(), HtmlBrowserSessionState::Active);
    assert!(session.has_in_process_runtime());
    assert_eq!(
        session.latest_frame().map(|frame| frame.generation),
        Some(1)
    );
    assert_eq!(
        session.take_frame_update().map(|frame| frame.generation),
        Some(1)
    );
    assert!(session.take_frame_update().is_none());
    Ok(())
}

#[test]
fn session_navigates_by_replacing_its_in_process_runtime() -> TestResult {
    let mut session = HtmlBrowserSession::new(
        test_source("https://example.test/a.html")?,
        viewport(TEST_VIEWPORT_WIDTH, TEST_VIEWPORT_HEIGHT)?,
    )
    .map_err(|error| error.to_string())?;
    let _ = session.take_frame_update();

    session
        .navigate(
            HtmlBrowserNavigation::new(test_source("https://example.test/b.html")?)
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;

    assert_eq!(
        session.source().origin.as_str(),
        "https://example.test/b.html"
    );
    assert_eq!(
        session
            .take_frame_update()
            .map(|frame| frame.origin.as_str()),
        Some("https://example.test/b.html")
    );
    Ok(())
}

#[test]
fn closed_session_rejects_runtime_operations() -> TestResult {
    let mut session = HtmlBrowserSession::new(
        test_source("https://example.test/a.html")?,
        viewport(TEST_VIEWPORT_WIDTH, TEST_VIEWPORT_HEIGHT)?,
    )
    .map_err(|error| error.to_string())?;
    session.close().map_err(|error| error.to_string())?;

    assert_eq!(session.state(), HtmlBrowserSessionState::Closed);
    assert!(!session.has_in_process_runtime());
    assert_eq!(
        session.refresh_frame(),
        Err(HtmlBrowserError::SessionClosed)
    );
    Ok(())
}

#[test]
fn session_exposes_debug_refresh_resize_and_navigation_contracts() -> TestResult {
    let mut session = HtmlBrowserSession::new(
        test_source("https://example.test/docs/index.html")?,
        viewport(TEST_VIEWPORT_WIDTH, TEST_VIEWPORT_HEIGHT)?,
    )
    .map_err(|error| error.to_string())?;
    assert_session_identity(&session)?;
    refresh_and_resize(&mut session)?;
    dispatch_non_navigating_pointer(&mut session)?;
    Ok(())
}

#[test]
fn session_rejects_invalid_source_missing_runtime_and_invalid_frames() -> TestResult {
    let mut session = HtmlBrowserSession::new(
        test_source("https://example.test/a.html")?,
        viewport(TEST_VIEWPORT_WIDTH, TEST_VIEWPORT_HEIGHT)?,
    )
    .map_err(|error| error.to_string())?;
    assert_invalid_navigation_is_rejected(&mut session)?;
    assert_frame_invariants(&mut session)?;
    assert_missing_runtime_is_rejected(&mut session)?;
    Ok(())
}

#[test]
fn session_close_rejects_every_public_runtime_operation() -> TestResult {
    let mut session = HtmlBrowserSession::new(
        test_source("https://example.test/a.html")?,
        viewport(TEST_VIEWPORT_WIDTH, TEST_VIEWPORT_HEIGHT)?,
    )
    .map_err(|error| error.to_string())?;
    session.close().map_err(|error| error.to_string())?;

    assert_eq!(
        session.navigate(
            HtmlBrowserNavigation::new(test_source("https://example.test/b.html")?)
                .map_err(|error| error.to_string())?,
        ),
        Err(HtmlBrowserError::SessionClosed)
    );
    assert_eq!(
        session.resize(viewport(TEST_VIEWPORT_WIDTH, TEST_VIEWPORT_HEIGHT)?),
        Err(HtmlBrowserError::SessionClosed)
    );
    assert_eq!(
        session.refresh_frame(),
        Err(HtmlBrowserError::SessionClosed)
    );
    assert_eq!(
        session.dispatch_input(HtmlBrowserInput::Focus { focused: true }),
        Err(HtmlBrowserError::SessionClosed)
    );
    Ok(())
}

#[test]
fn session_reports_runtime_start_failure_during_navigation() -> TestResult {
    let mut session = HtmlBrowserSession::new(
        test_source("https://example.test/a.html")?,
        viewport(TEST_VIEWPORT_WIDTH, TEST_VIEWPORT_HEIGHT)?,
    )
    .map_err(|error| error.to_string())?;
    let source =
        HtmlBrowserSource::new("<script>const = ;</script>", "https://example.test/b.html")
            .map_err(|error| error.to_string())?;
    let navigation = HtmlBrowserNavigation::new(source).map_err(|error| error.to_string())?;

    assert!(matches!(
        session.navigate(navigation),
        Err(HtmlBrowserError::RuntimeFailure { .. })
    ));
    assert_eq!(session.state(), HtmlBrowserSessionState::Closed);
    assert!(!session.has_in_process_runtime());
    assert!(session.latest_frame().is_none());
    assert!(session.take_frame_update().is_none());
    assert!(session.take_navigation().is_none());
    assert_eq!(
        session.refresh_frame(),
        Err(HtmlBrowserError::SessionClosed)
    );
    assert_eq!(
        session.dispatch_input(HtmlBrowserInput::Focus { focused: true }),
        Err(HtmlBrowserError::SessionClosed)
    );
    Ok(())
}

#[test]
fn session_forwards_runtime_navigation_once() -> TestResult {
    let source = HtmlBrowserSource::new(
        "<a id=next href=linked.html>Next</a>",
        "https://example.test/docs/index.html",
    )
    .map_err(|error| error.to_string())?;
    let mut session =
        HtmlBrowserSession::new(source, viewport(TEST_VIEWPORT_WIDTH, TEST_VIEWPORT_HEIGHT)?)
            .map_err(|error| error.to_string())?;
    let (x, y) = (TEST_POINTER_COORDINATE, TEST_POINTER_COORDINATE);

    session
        .dispatch_input(HtmlBrowserInput::PointerDown { x, y, button: 0 })
        .map_err(|error| error.to_string())?;
    session
        .dispatch_input(HtmlBrowserInput::PointerUp { x, y, button: 0 })
        .map_err(|error| error.to_string())?;
    assert_eq!(
        session
            .take_navigation()
            .map(|navigation| navigation.url.as_str().to_string()),
        Some("https://example.test/docs/linked.html".to_string())
    );
    assert!(session.take_navigation().is_none());
    Ok(())
}

fn assert_session_identity(session: &HtmlBrowserSession) -> TestResult {
    let debug = format!("{session:?}");
    assert!(debug.contains("HtmlBrowserSession"));
    assert!(debug.contains("has_in_process_runtime: true"));
    assert_eq!(
        session.source().origin.as_str(),
        "https://example.test/docs/index.html"
    );
    assert_eq!(
        session.viewport(),
        viewport(TEST_VIEWPORT_WIDTH, TEST_VIEWPORT_HEIGHT)?
    );
    Ok(())
}

fn refresh_and_resize(session: &mut HtmlBrowserSession) -> TestResult {
    let initial = session
        .latest_frame()
        .map(|frame| frame.generation)
        .ok_or_else(|| "initial frame must exist".to_string())?;
    session.refresh_frame().map_err(|error| error.to_string())?;
    assert!(
        session
            .take_frame_update()
            .is_some_and(|frame| frame.generation > initial)
    );
    let resized = HtmlBrowserViewport::new(RESIZED_VIEWPORT_WIDTH, RESIZED_VIEWPORT_HEIGHT, 1.0)
        .map_err(|error| error.to_string())?;
    session.resize(resized).map_err(|error| error.to_string())?;
    assert_eq!(session.viewport(), resized);
    assert_eq!(
        session.take_frame_update().map(|frame| frame.viewport),
        Some(resized)
    );
    Ok(())
}

fn dispatch_non_navigating_pointer(session: &mut HtmlBrowserSession) -> TestResult {
    super::shared_input_test_support::dispatch_primary_click(session, TEST_POINTER_COORDINATE)?;
    assert!(session.take_navigation().is_none());
    Ok(())
}

fn assert_invalid_navigation_is_rejected(session: &mut HtmlBrowserSession) -> TestResult {
    let mut invalid_source = test_source("https://example.test/b.html")?;
    invalid_source.raw_html = "x".repeat(HTML_BROWSER_MAX_SOURCE_BYTES + 1);
    assert!(matches!(
        session.navigate(HtmlBrowserNavigation {
            source: invalid_source
        }),
        Err(HtmlBrowserError::SourceTooLarge { .. })
    ));
    Ok(())
}

fn assert_frame_invariants(session: &mut HtmlBrowserSession) -> TestResult {
    let latest = session
        .latest_frame()
        .cloned()
        .ok_or_else(|| "initial frame must exist".to_string())?;
    assert!(matches!(
        session.accept_frame(latest.clone()),
        Err(HtmlBrowserError::StaleFrameGeneration { .. })
    ));
    let mismatched = HtmlBrowserFrame::new(
        latest.generation + 1,
        HtmlBrowserOrigin::parse("https://other.test/index.html")
            .map_err(|error| error.to_string())?,
        latest.viewport,
        latest.pixel_format,
        latest.pixels,
    )
    .map_err(|error| error.to_string())?;
    assert!(matches!(
        session.accept_frame(mismatched),
        Err(HtmlBrowserError::FrameOriginMismatch { .. })
    ));
    Ok(())
}

fn assert_missing_runtime_is_rejected(session: &mut HtmlBrowserSession) -> TestResult {
    session.interactive = None;
    assert_eq!(
        session.refresh_frame(),
        Err(HtmlBrowserError::RuntimeNotStarted)
    );
    assert_eq!(
        session.resize(viewport(TEST_VIEWPORT_WIDTH, TEST_VIEWPORT_HEIGHT)?),
        Err(HtmlBrowserError::RuntimeNotStarted)
    );
    Ok(())
}

pub(super) fn test_source(origin: &str) -> TestResult<HtmlBrowserSource> {
    HtmlBrowserSource::new("<button id=action>Run</button>", origin)
        .map_err(|error| error.to_string())
}

pub(super) fn viewport(width: u32, height: u32) -> TestResult<HtmlBrowserViewport> {
    HtmlBrowserViewport::new(width, height, 1.0).map_err(|error| error.to_string())
}
