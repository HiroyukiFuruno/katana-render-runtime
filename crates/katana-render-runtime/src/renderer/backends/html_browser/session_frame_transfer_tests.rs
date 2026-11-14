use super::super::{
    HtmlBrowserInput, HtmlBrowserSessionState, HtmlBrowserSource, HtmlBrowserViewport,
};
use super::{HtmlBrowserError, HtmlBrowserFrame, HtmlBrowserSession, HtmlInteractiveSession};
use crate::HtmlBrowserOrigin;

type TestResult<T = ()> = Result<T, String>;

const ORIGIN: &str = "https://example.test/frame-transfer.html";
const INITIAL_WIDTH: u32 = 80;
const RESIZED_WIDTH: u32 = INITIAL_WIDTH * 2;
const HEIGHT: u32 = 60;
const POINTER_COORDINATE: f32 = 20.0;
const HTML: &str = "<body style='margin:0;background:red;height:200px'></body>";

#[derive(Clone, Copy, Debug)]
enum Rejection {
    Stale,
    OriginMismatch,
    Closed,
}

fn viewport(width: u32) -> TestResult<HtmlBrowserViewport> {
    HtmlBrowserViewport::new(width, HEIGHT, 1.0).map_err(|error| error.to_string())
}

fn unpublished_session() -> TestResult<HtmlBrowserSession> {
    unpublished_session_at(HTML, ORIGIN)
}

fn unpublished_session_at(html: &str, origin: &str) -> TestResult<HtmlBrowserSession> {
    let source = HtmlBrowserSource::new(html, origin).map_err(|error| error.to_string())?;
    let viewport = viewport(INITIAL_WIDTH)?;
    let interactive = HtmlInteractiveSession::start(source.clone(), viewport)
        .map_err(|error| error.to_string())?;
    Ok(HtmlBrowserSession {
        source,
        viewport,
        state: HtmlBrowserSessionState::Active,
        latest_frame: None,
        latest_scroll_metrics: None,
        frame_update_pending: false,
        pending_navigation: None,
        interactive: Some(interactive),
    })
}

fn inner_frame(session: &HtmlBrowserSession) -> TestResult<&HtmlBrowserFrame> {
    session
        .interactive
        .as_ref()
        .and_then(HtmlInteractiveSession::latest_frame)
        .ok_or_else(|| "unaccepted inner frame must exist".to_owned())
}

fn sync(session: &mut HtmlBrowserSession) -> TestResult {
    session
        .sync_interactive_state()
        .map_err(|error| error.to_string())
}

fn stage_resize(session: &mut HtmlBrowserSession) -> TestResult {
    session
        .interactive
        .as_mut()
        .ok_or_else(|| "interactive runtime must exist".to_owned())?
        .resize(viewport(RESIZED_WIDTH)?)
        .map_err(|error| error.to_string())
}

fn assert_transferred(session: &mut HtmlBrowserSession, pointer: *const u8) -> TestResult {
    let (frame, metrics) = session
        .take_frame_update_with_metrics()
        .ok_or_else(|| "accepted frame and metrics must be published together".to_owned())?;
    assert_eq!(frame.pixels.as_ptr(), pointer);
    assert_eq!(frame.generation, metrics.generation);
    assert_eq!(frame.viewport.logical_width(), metrics.viewport_width);
    assert_eq!(frame.scroll_y, metrics.scroll_y);
    assert_eq!(frame.content_height, metrics.content_height);
    assert!(inner_frame(session).is_err());
    Ok(())
}

#[test]
fn first_frame_moves_inner_pixels_into_the_paired_public_frame() -> TestResult {
    let mut session = unpublished_session()?;
    let pointer = inner_frame(&session)?.pixels.as_ptr();
    sync(&mut session)?;
    assert_transferred(&mut session, pointer)?;
    session.close().map_err(|error| error.to_string())
}

#[test]
fn resized_frame_moves_inner_pixels_and_preserves_owned_snapshot() -> TestResult {
    let mut session = unpublished_session()?;
    sync(&mut session)?;
    let snapshot = session
        .latest_frame()
        .cloned()
        .ok_or_else(|| "initial frame must exist".to_owned())?;
    let saved_pixels = snapshot.pixels.clone();
    stage_resize(&mut session)?;
    let pointer = inner_frame(&session)?.pixels.as_ptr();
    sync(&mut session)?;
    assert_transferred(&mut session, pointer)?;
    assert_eq!(snapshot.viewport.width, INITIAL_WIDTH);
    assert_eq!(snapshot.pixels, saved_pixels);
    assert_eq!(
        session.latest_frame().map(|frame| frame.viewport.width),
        Some(RESIZED_WIDTH)
    );
    session.close().map_err(|error| error.to_string())
}

fn reject_next_frame(session: &mut HtmlBrowserSession, rejection: Rejection) -> TestResult {
    match rejection {
        Rejection::Stale => {
            let generation = inner_frame(session)?.generation;
            let current = session
                .latest_frame
                .as_mut()
                .ok_or_else(|| "accepted frame must exist".to_owned())?;
            current.generation = generation + 1;
        }
        Rejection::OriginMismatch => {
            session.source.origin = HtmlBrowserOrigin::parse("https://other.test/frame.html")
                .map_err(|error| error.to_string())?;
        }
        Rejection::Closed => session.state = HtmlBrowserSessionState::Closed,
    }
    Ok(())
}

fn assert_rejection(error: HtmlBrowserError, rejection: Rejection) {
    assert!(matches!(
        (error, rejection),
        (
            HtmlBrowserError::StaleFrameGeneration { .. },
            Rejection::Stale
        ) | (
            HtmlBrowserError::FrameOriginMismatch { .. },
            Rejection::OriginMismatch
        ) | (HtmlBrowserError::SessionClosed, Rejection::Closed)
    ));
}

fn assert_failed_transfer_preserved(
    session: &mut HtmlBrowserSession,
    rejection: Rejection,
    pointer: *const u8,
) -> TestResult {
    reject_next_frame(session, rejection)?;
    let prior = session.latest_frame().cloned();
    let metrics = session.latest_scroll_metrics().copied();
    let origin = session.source.origin.clone();
    let result = session.sync_interactive_state();
    match result {
        Err(error) => assert_rejection(error, rejection),
        Ok(()) => return Err("staged rejection must fail".to_owned()),
    }
    assert_eq!(session.latest_frame(), prior.as_ref());
    assert_eq!(session.latest_scroll_metrics(), metrics.as_ref());
    assert_eq!(session.source.origin, origin);
    assert_eq!(inner_frame(session)?.pixels.as_ptr(), pointer);
    assert!(session.take_frame_update().is_none());
    Ok(())
}

fn retry_rejected_transfer(rejection: Rejection) -> TestResult {
    let mut session = unpublished_session()?;
    sync(&mut session)?;
    let initial = session.latest_frame().cloned();
    assert!(session.take_frame_update().is_some());
    stage_resize(&mut session)?;
    let pointer = inner_frame(&session)?.pixels.as_ptr();
    assert_failed_transfer_preserved(&mut session, rejection, pointer)?;
    session.state = HtmlBrowserSessionState::Active;
    session.source.origin = HtmlBrowserOrigin::parse(ORIGIN).map_err(|error| error.to_string())?;
    session.latest_frame = initial;
    sync(&mut session)?;
    assert_transferred(&mut session, pointer)?;
    session.close().map_err(|error| error.to_string())
}

#[test]
fn rejected_resized_frame_keeps_inner_ownership_until_successful_retry() -> TestResult {
    for rejection in [
        Rejection::Stale,
        Rejection::OriginMismatch,
        Rejection::Closed,
    ] {
        retry_rejected_transfer(rejection)?;
    }
    Ok(())
}

fn retry_rejected_initial_transfer(rejection: Rejection) -> TestResult {
    let mut session = unpublished_session()?;
    let pointer = inner_frame(&session)?.pixels.as_ptr();
    assert_failed_transfer_preserved(&mut session, rejection, pointer)?;
    session.state = HtmlBrowserSessionState::Active;
    session.source.origin = HtmlBrowserOrigin::parse(ORIGIN).map_err(|error| error.to_string())?;
    sync(&mut session)?;
    assert_transferred(&mut session, pointer)?;
    session.close().map_err(|error| error.to_string())
}

#[test]
fn rejected_initial_frame_keeps_inner_ownership_until_successful_retry() -> TestResult {
    for rejection in [Rejection::OriginMismatch, Rejection::Closed] {
        retry_rejected_initial_transfer(rejection)?;
    }
    Ok(())
}

fn stage_fragment_resize(session: &mut HtmlBrowserSession) -> TestResult {
    let interactive = session
        .interactive
        .as_mut()
        .ok_or_else(|| "interactive runtime must exist".to_owned())?;
    for input in [
        HtmlBrowserInput::PointerDown {
            x: POINTER_COORDINATE,
            y: POINTER_COORDINATE,
            button: 0,
        },
        HtmlBrowserInput::PointerUp {
            x: POINTER_COORDINATE,
            y: POINTER_COORDINATE,
            button: 0,
        },
    ] {
        interactive
            .dispatch_input(input)
            .map_err(|error| error.to_string())?;
    }
    interactive
        .resize(viewport(RESIZED_WIDTH)?)
        .map_err(|error| error.to_string())
}

#[test]
fn fragment_resize_transfer_accepts_the_same_document_origin() -> TestResult {
    let html = "<a href=#target style='display:block;height:30px'>Jump</a><div style='height:200px'></div><p id=target>Target</p>";
    let mut session = unpublished_session_at(html, ORIGIN)?;
    sync(&mut session)?;
    stage_fragment_resize(&mut session)?;
    let pointer = inner_frame(&session)?.pixels.as_ptr();
    sync(&mut session)?;
    assert_transferred(&mut session, pointer)?;
    assert_eq!(session.source.origin.as_str(), format!("{ORIGIN}#target"));
    assert!(session.take_navigation().is_none());
    session.close().map_err(|error| error.to_string())
}

#[test]
fn initial_fragment_frame_moves_pixels_and_keeps_fragment_origin() -> TestResult {
    let origin = format!("{ORIGIN}#target");
    let html = "<div style='height:200px'></div><div id=target>Target</div>";
    let mut session = unpublished_session_at(html, &origin)?;
    let pointer = inner_frame(&session)?.pixels.as_ptr();
    sync(&mut session)?;
    assert_transferred(&mut session, pointer)?;
    assert_eq!(session.source.origin.as_str(), origin);
    assert!(
        session
            .latest_frame()
            .is_some_and(|frame| frame.scroll_y > 0.0)
    );
    session.close().map_err(|error| error.to_string())
}
