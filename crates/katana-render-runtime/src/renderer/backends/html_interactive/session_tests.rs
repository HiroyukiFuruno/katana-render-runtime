use super::super::super::html_browser::HtmlBrowserFrame;
use super::*;
use crate::renderer::backends::html_interactive::types::LayoutResult;

type TestResult<T = ()> = Result<T, HtmlBrowserError>;
const TEST_VIEWPORT_WIDTH: u32 = 320;
const TEST_VIEWPORT_HEIGHT: u32 = 240;
const RESIZED_VIEWPORT_LOGICAL_WIDTH: f32 = 180.0;
const RESIZED_VIEWPORT_LOGICAL_HEIGHT: f32 = 120.0;

#[path = "session_resize_fractional_tests.rs"]
mod fractional_resize;

fn required_frame(session: &HtmlInteractiveSession) -> Result<&HtmlBrowserFrame, HtmlBrowserError> {
    session
        .latest_frame()
        .ok_or(HtmlBrowserError::RuntimeNotStarted)
}

fn runtime_snapshot(session: &HtmlInteractiveSession) -> Result<String, HtmlBrowserError> {
    session
        .runtime
        .snapshot()
        .map_err(|error| HtmlBrowserError::RuntimeFailure {
            error: error.to_string(),
        })
}

fn set_retained_value(session: &mut HtmlInteractiveSession) -> TestResult {
    let node = session
        .runtime
        .node_for_element_id("target")
        .ok_or(HtmlBrowserError::RuntimeNotStarted)?;
    session
        .runtime
        .set_value(node.0, "retained")
        .map_err(|error| HtmlBrowserError::RuntimeFailure {
            error: error.to_string(),
        })
}

fn anchor_scroll_for_viewport(
    source: HtmlBrowserSource,
    viewport: HtmlBrowserViewport,
    anchor: &str,
) -> TestResult<(f32, f32)> {
    let mut reference = HtmlInteractiveSession::start(source, viewport)?;
    let layout = reference.layout()?;
    let anchor_y = layout
        .anchor_positions
        .get(anchor)
        .copied()
        .ok_or(HtmlBrowserError::RuntimeNotStarted)?;
    let maximum_scroll = (layout.content_height.round() - viewport.logical_height()).max(0.0);
    Ok((anchor_y.clamp(0.0, maximum_scroll), anchor_y))
}

fn assert_anchor_resize_frame(
    session: &HtmlInteractiveSession,
    viewport: HtmlBrowserViewport,
    expected_scroll: f32,
) -> TestResult {
    let frame = required_frame(session)?;
    let maximum_scroll = (frame.content_height.round() - viewport.logical_height()).max(0.0);
    assert_eq!(frame.generation, 2);
    assert_eq!(frame.viewport, viewport);
    assert_eq!(
        frame.pixels.len(),
        viewport.width as usize * viewport.height as usize * 4
    );
    assert_eq!(viewport.logical_width(), 160.0);
    assert_eq!(viewport.logical_height(), 200.0);
    assert!(frame.scroll_y.is_finite());
    assert!((frame.scroll_y - maximum_scroll).abs() < 1.0);
    assert_eq!(frame.scroll_y, expected_scroll);
    Ok(())
}

fn assert_noop_resize(
    session: &mut HtmlInteractiveSession,
    viewport: HtmlBrowserViewport,
) -> TestResult<HtmlBrowserFrame> {
    let initial = required_frame(session)?.clone();
    session.resize(viewport)?;
    let frame = required_frame(session)?;
    assert_eq!(frame.generation, initial.generation + 1);
    assert_eq!(frame.pixels, initial.pixels);
    assert_eq!(frame.scroll_y, initial.scroll_y);
    Ok(frame.clone())
}

pub(super) fn assert_frame_fits_viewport(frame: &HtmlBrowserFrame, viewport: HtmlBrowserViewport) {
    assert_eq!(frame.viewport, viewport);
    assert_eq!(
        frame.pixels.len(),
        viewport.width as usize * viewport.height as usize * 4
    );
}

fn assert_logical_viewport_size(
    viewport: HtmlBrowserViewport,
    logical_width: f32,
    logical_height: f32,
) {
    assert_eq!(viewport.logical_width(), logical_width);
    assert_eq!(viewport.logical_height(), logical_height);
}

fn assert_resized_pixels_match_refresh(
    session: &mut HtmlInteractiveSession,
    viewport: HtmlBrowserViewport,
    previous_generation: u64,
) -> TestResult {
    session.resize(viewport)?;
    let resized = required_frame(session)?.clone();
    assert_eq!(resized.generation, previous_generation + 1);
    assert_frame_fits_viewport(&resized, viewport);
    assert_logical_viewport_size(
        viewport,
        RESIZED_VIEWPORT_LOGICAL_WIDTH,
        RESIZED_VIEWPORT_LOGICAL_HEIGHT,
    );
    assert_refresh_matches_resized(session, &resized)
}

fn assert_refresh_matches_resized(
    session: &mut HtmlInteractiveSession,
    resized: &HtmlBrowserFrame,
) -> TestResult {
    session.refresh_frame()?;
    let recomputed = required_frame(session)?;
    assert_eq!(recomputed.generation, resized.generation + 1);
    assert_eq!(recomputed.viewport, resized.viewport);
    assert_eq!(recomputed.pixels, resized.pixels);
    assert_eq!(recomputed.scroll_y, resized.scroll_y);
    assert_eq!(recomputed.content_height, resized.content_height);
    Ok(())
}

fn marker_box_height(session: &mut HtmlInteractiveSession) -> TestResult<f32> {
    let marker = session
        .runtime
        .node_for_element_id("marker")
        .ok_or(HtmlBrowserError::RuntimeNotStarted)?;
    session
        .element_boxes
        .iter()
        .find(|element| element.node_id == marker.0)
        .map(|element| element.height)
        .ok_or(HtmlBrowserError::RuntimeNotStarted)
}

fn assert_intersection_resize_relayout(session: &mut HtmlInteractiveSession) -> TestResult {
    let resized = required_frame(session)?.clone();
    let resized_marker_height = marker_box_height(session)?;
    assert_eq!(resized_marker_height, 80.0);
    let callback_runs = session
        .runtime
        .observe_feature_state("window.__resizeIoRuns")
        .map_err(|error| HtmlBrowserError::RuntimeFailure { error })?;
    assert!(callback_runs.as_u64().is_some_and(|runs| runs > 0));

    session.refresh_frame()?;
    let refreshed = required_frame(session)?;
    assert_eq!(refreshed.pixels, resized.pixels);
    assert_eq!(refreshed.content_height, resized.content_height);
    assert_eq!(refreshed.scroll_y, resized.scroll_y);
    assert_eq!(marker_box_height(session)?, resized_marker_height);
    let retained_runs = session
        .runtime
        .observe_feature_state("window.__resizeIoRuns")
        .map_err(|error| HtmlBrowserError::RuntimeFailure { error })?;
    assert_eq!(retained_runs, callback_runs);
    Ok(())
}

#[test]
fn raster_and_frame_failures_are_mapped_to_runtime_errors() -> TestResult {
    let source = HtmlBrowserSource::new("<p>frame</p>", "https://example.test/frame.html")?;
    let viewport = HtmlBrowserViewport::new(TEST_VIEWPORT_WIDTH, TEST_VIEWPORT_HEIGHT, 1.0)?;
    let mut session = HtmlInteractiveSession::start(source, viewport)?;

    assert!(session.rasterize("not an svg document").is_err());
    assert!(
        session
            .rasterize("<svg xmlns='http://www.w3.org/2000/svg' width='1' height='1'/>")
            .is_err()
    );
    assert!(matches!(
        session.resize(HtmlBrowserViewport {
            width: 0,
            height: TEST_VIEWPORT_HEIGHT,
            device_scale_factor: 1.0,
        }),
        Err(HtmlBrowserError::InvalidViewport)
    ));
    let layout = empty_viewport_layout(session.viewport);
    assert!(session.store_frame(layout, vec![0]).is_err());
    Ok(())
}

fn empty_viewport_layout(viewport: HtmlBrowserViewport) -> LayoutResult {
    LayoutResult {
        svg: String::new(),
        generated_clip_provenance: crate::markdown::svg_rasterize::GeneratedClipProvenance::default(
        ),
        generated_html_text_provenance: Default::default(),
        hit_targets: Vec::new(),
        element_boxes: Vec::new(),
        anchor_positions: HashMap::new(),
        content_width: viewport.logical_width(),
        content_height: viewport.logical_height(),
        viewport_overflow: (
            crate::HtmlBrowserOverflowPolicy::Auto,
            crate::HtmlBrowserOverflowPolicy::Auto,
        ),
    }
}

#[test]
fn resize_preserves_frame_pixels_and_updates_dimensions_once() -> TestResult {
    let source = HtmlBrowserSource::new(
        "<main style='width:280px;height:360px;background:#dae8fc'><p>Resize</p></main>",
        "https://example.test/resize.html",
    )?;
    let initial_viewport = HtmlBrowserViewport::new(320, 240, 1.0)?;
    let mut session = HtmlInteractiveSession::start(source, initial_viewport)?;
    let unchanged = assert_noop_resize(&mut session, initial_viewport)?;
    let resized_viewport = HtmlBrowserViewport::new(360, 240, 2.0)?;
    assert_resized_pixels_match_refresh(&mut session, resized_viewport, unchanged.generation)?;
    Ok(())
}

#[test]
fn resize_resolves_anchor_clamps_scroll_and_keeps_runtime_dom() -> TestResult {
    let source = HtmlBrowserSource::new(
        "<main style='height:900px'><p style='margin:0'>many words resize the preceding content and move the target anchor across viewport widths in this retained document</p><div id=target style='margin-top:720px'>Target</div></main>",
        "https://example.test/anchor.html",
    )?;
    let initial_viewport = HtmlBrowserViewport::new(320, 160, 1.0)?;
    let mut session = HtmlInteractiveSession::start(source.clone(), initial_viewport)?;
    let (_, initial_anchor) =
        anchor_scroll_for_viewport(source.clone(), initial_viewport, "target")?;
    set_retained_value(&mut session)?;
    session.resize_anchor = Some("target".to_string());
    session.scroll_geometry.scroll_y = 10_000.0;

    let resized_viewport = HtmlBrowserViewport::new(320, 400, 2.0)?;
    let (expected_scroll, resized_anchor) =
        anchor_scroll_for_viewport(source, resized_viewport, "target")?;
    assert!((initial_anchor - resized_anchor).abs() > 1.0);
    assert!(
        expected_scroll < resized_anchor.round(),
        "expected anchor-clamped scroll {expected_scroll} to precede rounded anchor {resized_anchor} (rounded {})",
        resized_anchor.round()
    );
    session.resize(resized_viewport)?;
    assert_anchor_resize_frame(&session, resized_viewport, expected_scroll)?;
    let snapshot = runtime_snapshot(&session)?;
    assert!(snapshot.contains("value=\"retained\""));
    Ok(())
}

#[test]
fn resize_relayouts_after_intersection_callback_mutates_the_dom() -> TestResult {
    let source = HtmlBrowserSource::new(
        "<div id=marker style='position:absolute;top:200px;height:40px'>Marker</div><script>window.__resizeIoRuns = 0; const marker = document.getElementById('marker'); new IntersectionObserver((entries) => { if (entries.some((entry) => entry.isIntersecting)) { window.__resizeIoRuns += 1; marker.style.height = '80px'; } }).observe(marker);</script>",
        "https://example.test/intersection-resize.html",
    )?;
    let initial_viewport = HtmlBrowserViewport::new(240, 80, 1.0)?;
    let mut session = HtmlInteractiveSession::start(source, initial_viewport)?;
    let initial_generation = required_frame(&session)?.generation;
    let initial_runs = session
        .runtime
        .observe_feature_state("window.__resizeIoRuns")
        .map_err(|error| HtmlBrowserError::RuntimeFailure { error })?;
    assert_eq!(initial_runs, 0);

    let resized_viewport = HtmlBrowserViewport::new(480, 640, 2.0)?;
    session.resize(resized_viewport)?;
    let frame = required_frame(&session)?;
    assert_eq!(frame.generation, initial_generation + 1);
    assert_eq!(frame.viewport, resized_viewport);
    assert_eq!(frame.pixels.len(), 480 * 640 * 4);
    assert_eq!(resized_viewport.logical_width(), 240.0);
    assert_eq!(resized_viewport.logical_height(), 320.0);
    assert_intersection_resize_relayout(&mut session)?;
    Ok(())
}
