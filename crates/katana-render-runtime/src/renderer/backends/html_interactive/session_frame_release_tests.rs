use super::{
    HtmlBrowserSource, HtmlBrowserViewport, HtmlInteractiveSession, TEST_VIEWPORT_HEIGHT,
    TEST_VIEWPORT_WIDTH, TestResult, required_frame,
};

#[test]
fn frame_release_keeps_an_unaccepted_generation_available() -> TestResult {
    let source = HtmlBrowserSource::new(
        "<body style=\"margin:0;background:red\"></body>",
        "https://example.test/frame-release.html",
    )?;
    let viewport = HtmlBrowserViewport::new(TEST_VIEWPORT_WIDTH, TEST_VIEWPORT_HEIGHT, 1.0)?;
    let mut session = HtmlInteractiveSession::start(source, viewport)?;
    let frame = required_frame(&session)?.clone();

    session.release_frame_if_generation(frame.generation + 1);
    assert_eq!(required_frame(&session)?, &frame);
    session.release_frame_if_generation(frame.generation);
    assert!(session.latest_frame().is_none());

    Ok(())
}
