use super::*;
use std::time::Duration;

type TestResult<T = ()> = Result<T, String>;
const TIMER_VIEWPORT_WIDTH: u32 = 160;
const TIMER_VIEWPORT_HEIGHT: u32 = 120;
const TIMER_BEFORE_DEADLINE_MS: u64 = 24;

#[test]
fn public_session_advance_time_drives_delayed_timer_and_resets_on_navigation() -> TestResult {
    let timer_source = || {
        HtmlBrowserSource::new(
            "<output>waiting</output><script>setTimeout(() => document.querySelector('output').textContent = 'done', 25)</script>",
            "https://example.test/timer.html",
        )
        .map_err(to_string)
    };
    let mut session = start_session(timer_source()?)?;
    let initial_generation = generation(&session)?;
    assert_no_frame_update(&mut session, initial_generation)?;
    let completed_generation = advance_and_generation(&mut session, 1)?;
    assert!(completed_generation > initial_generation);

    session
        .navigate(HtmlBrowserNavigation::new(timer_source()?).map_err(to_string)?)
        .map_err(to_string)?;
    let reset_generation = generation(&session)?;
    assert_no_frame_update(&mut session, reset_generation)?;
    assert!(advance_and_generation(&mut session, 1)? > reset_generation);
    session.close().map_err(to_string)?;
    assert_eq!(
        session.advance_time(Duration::from_millis(25)),
        Err(HtmlBrowserError::SessionClosed)
    );
    Ok(())
}

fn assert_no_frame_update(session: &mut HtmlBrowserSession, expected: u64) -> TestResult {
    session
        .advance_time(Duration::from_millis(TIMER_BEFORE_DEADLINE_MS))
        .map_err(to_string)?;
    assert_eq!(generation(session)?, expected);
    Ok(())
}

fn advance_and_generation(session: &mut HtmlBrowserSession, milliseconds: u64) -> TestResult<u64> {
    session
        .advance_time(Duration::from_millis(milliseconds))
        .map_err(to_string)?;
    generation(session)
}

fn start_session(source: HtmlBrowserSource) -> TestResult<HtmlBrowserSession> {
    let viewport = HtmlBrowserViewport::new(TIMER_VIEWPORT_WIDTH, TIMER_VIEWPORT_HEIGHT, 1.0)
        .map_err(to_string)?;
    HtmlBrowserSession::new(source, viewport).map_err(to_string)
}

fn generation(session: &HtmlBrowserSession) -> TestResult<u64> {
    session
        .latest_frame()
        .map(|frame| frame.generation)
        .ok_or_else(|| "frame must exist".to_owned())
}

fn to_string(error: impl std::fmt::Display) -> String {
    error.to_string()
}
