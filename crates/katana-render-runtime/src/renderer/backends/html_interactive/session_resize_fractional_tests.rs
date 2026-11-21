use super::*;

const FRACTIONAL_ANCHOR_Y: f32 = 229.25;
const FRACTIONAL_ANCHOR_TOLERANCE: f32 = 0.001;
const FRACTIONAL_RESIZE_WIDTH: u32 = 400;
const FRACTIONAL_RESIZE_HEIGHT: u32 = 360;
const FRACTIONAL_RESIZE_SCALE: f32 = 2.0;
const CLAMPED_RESIZE_WIDTH: u32 = 320;
const CLAMPED_RESIZE_HEIGHT: u32 = 3_000;
const CLAMPED_RESIZE_SCALE: f32 = 2.0;
const TOP_SCROLL_Y: f32 = 0.0;

#[test]
fn resize_preserves_fractional_css_anchor_and_clamps_after_dpr_change() -> TestResult {
    let html = format!(
        "<style>html,body{{margin:0}}</style><div style='height:{FRACTIONAL_ANCHOR_Y}px'></div><input id=target value=initial style='display:block;height:1px'><div style='height:1000px'></div>"
    );
    let source = HtmlBrowserSource::new(&html, "https://example.test/fractional-anchor.html")?;
    let initial_viewport =
        HtmlBrowserViewport::new(TEST_VIEWPORT_WIDTH, TEST_VIEWPORT_HEIGHT, 1.0)?;
    let mut session = HtmlInteractiveSession::start(source, initial_viewport)?;
    assert_fractional_anchor_position(&mut session)?;
    set_retained_value(&mut session)?;
    session.resize_anchor = Some("target".to_string());
    assert_fractional_anchor_resize(&mut session)?;
    assert_fractional_anchor_clamp(&mut session)?;
    Ok(())
}

fn assert_fractional_anchor_position(session: &mut HtmlInteractiveSession) -> TestResult {
    let layout = session.layout()?;
    let anchor_y = layout
        .anchor_positions
        .get("target")
        .copied()
        .ok_or(HtmlBrowserError::RuntimeNotStarted)?;
    assert!(
        (anchor_y - FRACTIONAL_ANCHOR_Y).abs() < FRACTIONAL_ANCHOR_TOLERANCE,
        "fractional anchor position mismatch: actual={anchor_y}, expected={FRACTIONAL_ANCHOR_Y}, tolerance={FRACTIONAL_ANCHOR_TOLERANCE}"
    );
    Ok(())
}

fn assert_fractional_anchor_resize(session: &mut HtmlInteractiveSession) -> TestResult {
    let viewport = HtmlBrowserViewport::new(
        FRACTIONAL_RESIZE_WIDTH,
        FRACTIONAL_RESIZE_HEIGHT,
        FRACTIONAL_RESIZE_SCALE,
    )?;
    let generation = required_frame(session)?.generation;
    session.resize(viewport)?;
    let resized = required_frame(session)?.clone();
    assert_eq!(resized.generation, generation + 1);
    assert_frame_fits_viewport(&resized, viewport);
    assert_eq!(resized.scroll_y, FRACTIONAL_ANCHOR_Y);
    assert!(runtime_snapshot(session)?.contains("value=\"retained\""));
    assert_refresh_matches_resized(session, &resized)
}

fn assert_fractional_anchor_clamp(session: &mut HtmlInteractiveSession) -> TestResult {
    let viewport = HtmlBrowserViewport::new(
        CLAMPED_RESIZE_WIDTH,
        CLAMPED_RESIZE_HEIGHT,
        CLAMPED_RESIZE_SCALE,
    )?;
    session.resize(viewport)?;
    let resized = required_frame(session)?.clone();
    assert_frame_fits_viewport(&resized, viewport);
    assert_eq!(resized.scroll_y, TOP_SCROLL_Y);
    assert!(runtime_snapshot(session)?.contains("value=\"retained\""));
    assert_refresh_matches_resized(session, &resized)
}
