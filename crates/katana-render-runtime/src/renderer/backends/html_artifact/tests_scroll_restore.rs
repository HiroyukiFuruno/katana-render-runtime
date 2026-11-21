use super::provenance;
use super::{
    HtmlBrowserInput, HtmlBrowserSource, HtmlBrowserViewport, HtmlFrameArtifact, HtmlRuntime,
    SAVE_TEST_HEIGHT, SAVE_TEST_WIDTH,
};

#[rstest::rstest]
#[case::ordinary_scroll("", 1.0, 120.0, 0.0, 120.0)]
#[case::scaled_scroll("", 2.0, 240.0, 0.0, 120.0)]
#[case::past_anchor("#target", 1.0, 120.0, 240.0, 360.0)]
#[case::above_anchor("#target", 2.0, -480.0, 240.0, 0.0)]
#[case::at_anchor("#target", 2.0, 0.0, 240.0, 240.0)]
fn resume_session_restores_saved_scroll_position(
    #[case] fragment: &str,
    #[case] scale: f32,
    #[case] delta_y: f32,
    #[case] initial_y: f32,
    #[case] saved_y: f32,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = scroll_source(fragment)?;
    let viewport = HtmlBrowserViewport::new(SAVE_TEST_WIDTH, SAVE_TEST_HEIGHT, scale)?;
    let provenance = provenance(&source, viewport)?;
    let mut session = HtmlRuntime.open(source.clone(), viewport)?;
    assert_eq!(scroll_y(&session)?, initial_y);
    session.dispatch_input(HtmlBrowserInput::Scroll {
        delta_x: 0.0,
        delta_y,
    })?;
    let frame = session.latest_frame().ok_or("saved frame")?.clone();
    assert_eq!(frame.scroll_y, saved_y);
    let artifact = HtmlFrameArtifact::new(provenance.clone(), frame)?;
    session.close()?;

    let mut resumed = artifact.resume_session(source, viewport, &provenance)?;
    assert_eq!(scroll_y(&resumed)?, saved_y);
    resumed.close().map_err(Into::into)
}

fn scroll_source(fragment: &str) -> Result<HtmlBrowserSource, crate::HtmlBrowserError> {
    HtmlBrowserSource::new(
        "<style>body{margin:0}</style><div style='height:240px'></div><div id='target' style='height:500px'>Target</div>",
        format!("https://example.test/scroll.html{fragment}"),
    )
}

fn scroll_y(session: &crate::HtmlBrowserSession) -> Result<f32, &'static str> {
    Ok(session.latest_frame().ok_or("scroll frame")?.scroll_y)
}
