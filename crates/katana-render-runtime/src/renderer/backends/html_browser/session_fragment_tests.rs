use super::{
    HtmlBrowserError, HtmlBrowserSession, HtmlBrowserSource, HtmlBrowserViewport,
    shared_input_test_support::dispatch_primary_click,
};
use crate::HtmlBrowserOrigin;

type TestResult<T = ()> = Result<T, String>;
const TEST_POINTER_COORDINATE: f32 = 20.0;
const TEST_VIEWPORT_WIDTH: u32 = 160;
const TEST_VIEWPORT_HEIGHT: u32 = 120;

#[test]
fn session_accepts_same_document_fragment_frames_without_host_navigation() -> TestResult {
    let mut session = HtmlBrowserSession::new(fragment_source()?, viewport()?)
        .map_err(|error| error.to_string())?;
    let initial_generation = session
        .take_frame_update()
        .map(|frame| frame.generation)
        .ok_or_else(|| "initial frame update must exist".to_string())?;

    dispatch_primary_click(&mut session, TEST_POINTER_COORDINATE)?;

    assert_fragment_frame(&mut session, initial_generation)
}

#[test]
fn rejected_stale_fragment_frame_preserves_the_accepted_origin() -> TestResult {
    let mut session = HtmlBrowserSession::new(fragment_source()?, viewport()?)
        .map_err(|error| error.to_string())?;
    dispatch_primary_click(&mut session, TEST_POINTER_COORDINATE)?;
    let mut stale = session
        .latest_frame()
        .cloned()
        .ok_or_else(|| "fragment frame must exist".to_string())?;
    stale.generation = stale.generation.saturating_sub(1);
    stale.origin = HtmlBrowserOrigin::parse("https://example.test/docs/index.html#other")
        .map_err(|error| error.to_string())?;

    let result = session.sync_interactive_frame(Some(stale));

    assert!(matches!(
        result,
        Err(HtmlBrowserError::StaleFrameGeneration { .. })
    ));
    assert_eq!(
        session.source().origin.as_str(),
        "https://example.test/docs/index.html#target"
    );
    Ok(())
}

fn fragment_source() -> TestResult<HtmlBrowserSource> {
    HtmlBrowserSource::new(
        format!(
            "<a href=#target>Jump</a>{}<h2 id=target>Target</h2><p>After</p>",
            "<p>spacer</p>".repeat(30)
        ),
        "https://example.test/docs/index.html",
    )
    .map_err(|error| error.to_string())
}

fn viewport() -> TestResult<HtmlBrowserViewport> {
    HtmlBrowserViewport::new(TEST_VIEWPORT_WIDTH, TEST_VIEWPORT_HEIGHT, 1.0)
        .map_err(|error| error.to_string())
}

fn assert_fragment_frame(session: &mut HtmlBrowserSession, initial_generation: u64) -> TestResult {
    assert!(session.take_navigation().is_none());
    assert_eq!(
        session.source().origin.as_str(),
        "https://example.test/docs/index.html#target"
    );
    assert!(session.take_frame_update().is_some_and(|frame| {
        frame.generation > initial_generation
            && frame.origin.as_str() == "https://example.test/docs/index.html#target"
    }));
    Ok(())
}
