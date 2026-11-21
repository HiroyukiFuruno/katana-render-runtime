use super::*;

type TestResult<T = ()> = Result<T, String>;

const INITIAL_VIEWPORT_WIDTH: u32 = 240;
const INITIAL_VIEWPORT_HEIGHT: u32 = 80;
const INITIAL_DEVICE_SCALE_FACTOR: f32 = 1.0;
const RESIZED_VIEWPORT_WIDTH: u32 = 480;
const RESIZED_VIEWPORT_HEIGHT: u32 = 640;
const RESIZED_DEVICE_SCALE_FACTOR: f32 = 2.0;
const FUTURE_GENERATION_OFFSET: u64 = 2;

#[test]
fn failed_resize_closes_session_and_preserves_render_error() -> TestResult {
    const RENDER_ERROR: &str = "one-shot resize observer failure";
    let mut session = session_with_resize_observer(RENDER_ERROR)?;
    let viewport = resized_viewport()?;
    assert_original_resize_error(session.resize(viewport), RENDER_ERROR)?;
    assert_closed_after_resize(&mut session)
}

#[test]
fn failed_resize_sync_closes_session_and_preserves_stale_frame_error() -> TestResult {
    let mut session = session_with_html(
        "<main>Resize synchronization</main>".to_owned(),
        "https://example.test/resize-sync-error.html",
    )?;
    let mut stale_frame = session
        .latest_frame()
        .cloned()
        .ok_or_else(|| "initial frame must exist".to_string())?;
    stale_frame.generation += FUTURE_GENERATION_OFFSET;
    session.latest_frame = Some(stale_frame);

    let viewport = resized_viewport()?;
    assert_stale_frame_error(session.resize(viewport))?;
    assert_closed_after_resize(&mut session)
}

fn session_with_resize_observer(render_error: &str) -> TestResult<HtmlBrowserSession> {
    session_with_html(
        format!(
            "<div id=marker style='position:absolute;top:200px;height:40px'>Marker</div><script>window.__resizeThrows = true; const marker = document.getElementById('marker'); new IntersectionObserver((entries) => {{ if (window.__resizeThrows && entries.some((entry) => entry.isIntersecting)) {{ window.__resizeThrows = false; throw new Error('{render_error}'); }} }}).observe(marker);</script>"
        ),
        "https://example.test/resize-error.html",
    )
}

fn session_with_html(html: String, url: &str) -> TestResult<HtmlBrowserSession> {
    let source = HtmlBrowserSource::new(html, url).map_err(|error| error.to_string())?;
    let mut session = HtmlBrowserSession::new(
        source,
        HtmlBrowserViewport::new(
            INITIAL_VIEWPORT_WIDTH,
            INITIAL_VIEWPORT_HEIGHT,
            INITIAL_DEVICE_SCALE_FACTOR,
        )
        .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    assert!(session.latest_frame().is_some());
    assert!(session.take_frame_update().is_some());
    Ok(session)
}

fn resized_viewport() -> TestResult<HtmlBrowserViewport> {
    HtmlBrowserViewport::new(
        RESIZED_VIEWPORT_WIDTH,
        RESIZED_VIEWPORT_HEIGHT,
        RESIZED_DEVICE_SCALE_FACTOR,
    )
    .map_err(|error| error.to_string())
}

fn assert_original_resize_error(
    result: Result<(), HtmlBrowserError>,
    expected: &str,
) -> TestResult {
    match result {
        Err(HtmlBrowserError::RuntimeFailure { error }) if error.contains(expected) => Ok(()),
        Err(other) => Err(format!(
            "expected the original runtime error, got {other:?}"
        )),
        Ok(()) => {
            Err("expected the resize observer to fail while rendering the resized viewport".into())
        }
    }
}

fn assert_closed_after_resize(session: &mut HtmlBrowserSession) -> TestResult {
    assert_eq!(session.state(), HtmlBrowserSessionState::Closed);
    assert!(session.latest_frame().is_none());
    assert!(session.take_frame_update().is_none());
    assert!(!session.has_in_process_runtime());
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

fn assert_stale_frame_error(result: Result<(), HtmlBrowserError>) -> TestResult {
    match result {
        Err(HtmlBrowserError::StaleFrameGeneration { latest, received }) if latest > received => {
            Ok(())
        }
        Err(other) => Err(format!("expected the stale-frame error, got {other:?}")),
        Ok(()) => Err("expected resize synchronization to reject the stale frame".into()),
    }
}
