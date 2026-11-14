use super::super::HtmlBrowserOverflowPolicy;
use super::*;

#[path = "session_scroll_extent_tests.rs"]
mod extent_tests;
#[path = "session_scroll_root_clip_tests.rs"]
mod root_clip_tests;
#[path = "session_scroll_target_tests.rs"]
mod target_tests;

type TestResult<T = ()> = Result<T, String>;

const PHYSICAL_HEIGHT: u32 = 480;
const DEVICE_SCALE_FACTOR: f32 = 2.0;
const LOGICAL_WIDTH: f32 = 320.0;
const LOGICAL_HEIGHT: f32 = 240.0;
const CSS_SCROLL_DELTA_X: f32 = 400.0;
const CSS_SCROLL_DELTA_Y: f32 = 300.0;
const PHYSICAL_SCROLL_DELTA_X: f32 = 800.0;
const PHYSICAL_SCROLL_DELTA_Y: f32 = 600.0;
const TARGET_VIEWPORT_X: f32 = 40.0;
const TARGET_VIEWPORT_Y: f32 = 40.0;
const TARGET_CONTENT_X: f32 = 430.0;
const TARGET_CONTENT_Y: f32 = 325.0;
const RESIZED_PHYSICAL_WIDTH: u32 = 1600;
const SOURCE_ORIGIN: &str = "https://example.test/scroll.html";
const RGBA_BYTES: u32 = 4;
const BLUE: [u8; 4] = [0, 0, 255, 255];
const RED: [u8; 4] = [255, 0, 0, 255];
const SCROLL_FIXTURE: &str = r#"
    <!doctype html>
    <html>
      <body style="margin:0;overflow:scroll;background:white">
        <div id="fixed" style="position:fixed;left:4px;top:4px;width:10px;height:10px;background:blue"></div>
        <button id="target" style="position:absolute;left:420px;top:320px;width:40px;height:40px;background:blue" onclick="this.style.background='red'">target</button>
        <div style="width:800px;height:600px"></div>
      </body>
    </html>
"#;

#[derive(Clone, Copy)]
struct RootClipCase {
    label: &'static str,
    html_overflow: &'static str,
    body_overflow: &'static str,
    expected_x: HtmlBrowserOverflowPolicy,
    expected_y: HtmlBrowserOverflowPolicy,
}

const ROOT_CLIP_CASES: [RootClipCase; 4] = [
    RootClipCase {
        label: "html single-axis",
        html_overflow: "overflow-x:clip;overflow-y:visible",
        body_overflow: "overflow:visible",
        expected_x: HtmlBrowserOverflowPolicy::Hidden,
        expected_y: HtmlBrowserOverflowPolicy::Auto,
    },
    RootClipCase {
        label: "html both-axis",
        html_overflow: "overflow:clip",
        body_overflow: "overflow:visible",
        expected_x: HtmlBrowserOverflowPolicy::Hidden,
        expected_y: HtmlBrowserOverflowPolicy::Hidden,
    },
    RootClipCase {
        label: "body single-axis propagated",
        html_overflow: "overflow:visible",
        body_overflow: "overflow-x:clip;overflow-y:visible",
        expected_x: HtmlBrowserOverflowPolicy::Hidden,
        expected_y: HtmlBrowserOverflowPolicy::Auto,
    },
    RootClipCase {
        label: "body both-axis propagated",
        html_overflow: "overflow:visible",
        body_overflow: "overflow:clip",
        expected_x: HtmlBrowserOverflowPolicy::Hidden,
        expected_y: HtmlBrowserOverflowPolicy::Hidden,
    },
];

fn scroll_source() -> TestResult<HtmlBrowserSource> {
    HtmlBrowserSource::new(SCROLL_FIXTURE, SOURCE_ORIGIN).map_err(|error| error.to_string())
}

fn viewport_for_scale(scale: f32) -> TestResult<HtmlBrowserViewport> {
    HtmlBrowserViewport::new(
        (LOGICAL_WIDTH * scale) as u32,
        (LOGICAL_HEIGHT * scale) as u32,
        scale,
    )
    .map_err(|error| error.to_string())
}

fn open_scroll_session(scale: f32) -> TestResult<HtmlBrowserSession> {
    HtmlBrowserSession::new(scroll_source()?, viewport_for_scale(scale)?)
        .map_err(|error| error.to_string())
}

fn required_metrics(session: &HtmlBrowserSession) -> TestResult<&HtmlBrowserScrollMetrics> {
    session
        .latest_scroll_metrics()
        .ok_or_else(|| "committed scroll metrics must exist".to_string())
}

fn pixel(frame: &HtmlBrowserFrame, x: u32, y: u32) -> TestResult<[u8; RGBA_BYTES as usize]> {
    let index = ((y * frame.viewport.width + x) * RGBA_BYTES) as usize;
    frame
        .pixels
        .get(index..index + RGBA_BYTES as usize)
        .ok_or_else(|| format!("missing RGBA pixel at ({x},{y})"))?
        .try_into()
        .map_err(|_| "RGBA pixel must have four bytes".to_string())
}

pub(super) fn pixel_at_content_css(
    session: &HtmlBrowserSession,
    content_x: f32,
    content_y: f32,
) -> TestResult<[u8; RGBA_BYTES as usize]> {
    let (frame, metrics) = session
        .latest_frame_with_metrics()
        .ok_or_else(|| "frame and scroll metrics must be paired".to_string())?;
    if frame.generation != metrics.generation {
        return Err("frame and scroll metrics must have the same generation".to_string());
    }

    let scale = frame.viewport.device_scale_factor;
    let physical_x = (content_x - metrics.scroll_x) * scale;
    let physical_y = (content_y - metrics.scroll_y) * scale;
    if !physical_x.is_finite() || !physical_y.is_finite() || physical_x < 0.0 || physical_y < 0.0 {
        return Err("content CSS coordinate must map inside the physical frame".to_string());
    }

    pixel(frame, physical_x.floor() as u32, physical_y.floor() as u32)
}

#[test]
fn dpr_table_preserves_css_metrics_scroll_and_fixed_pixels() -> TestResult {
    for scale in [1.0_f32, 2.0_f32] {
        let mut session = open_scroll_session(scale)?;
        assert_initial_dpr_state(&session, scale)?;
        consume_and_scroll(&mut session, scale)?;
        assert_scrolled_dpr_state(&session, scale)?;
    }
    Ok(())
}

fn assert_initial_dpr_state(session: &HtmlBrowserSession, scale: f32) -> TestResult {
    let (frame, metrics) = session
        .latest_frame_with_metrics()
        .ok_or_else(|| "initial frame and metrics must be paired".to_string())?;

    assert_initial_geometry(frame, metrics);
    assert_initial_scroll_state(frame, metrics, scale)?;
    Ok(())
}

fn assert_initial_geometry(frame: &HtmlBrowserFrame, metrics: &HtmlBrowserScrollMetrics) {
    assert_eq!(frame.viewport.logical_width(), LOGICAL_WIDTH);
    assert_eq!(frame.viewport.logical_height(), LOGICAL_HEIGHT);
    assert_eq!(frame.generation, metrics.generation);
    assert_eq!(metrics.viewport_width, LOGICAL_WIDTH);
    assert_eq!(metrics.viewport_height, LOGICAL_HEIGHT);
    assert_eq!(metrics.content_width, 800.0);
    assert_eq!(metrics.content_height, 600.0);
}

fn assert_initial_scroll_state(
    frame: &HtmlBrowserFrame,
    metrics: &HtmlBrowserScrollMetrics,
    scale: f32,
) -> TestResult {
    assert_eq!(metrics.scroll_x, 0.0);
    assert_eq!(metrics.scroll_y, 0.0);
    assert_eq!(
        pixel(frame, (4.0 * scale) as u32, (4.0 * scale) as u32)?,
        BLUE
    );
    Ok(())
}

fn consume_and_scroll(session: &mut HtmlBrowserSession, scale: f32) -> TestResult {
    session
        .take_frame_update_with_metrics()
        .ok_or("initial update missing")?;
    session
        .dispatch_input(HtmlBrowserInput::Scroll {
            delta_x: CSS_SCROLL_DELTA_X * scale,
            delta_y: CSS_SCROLL_DELTA_Y * scale,
        })
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn assert_scrolled_dpr_state(session: &HtmlBrowserSession, scale: f32) -> TestResult {
    let (frame, metrics) = session
        .latest_frame_with_metrics()
        .ok_or_else(|| "scrolled frame and metrics must be paired".to_string())?;
    assert_eq!(metrics.scroll_x, 400.0);
    assert_eq!(metrics.scroll_y, 300.0);
    assert_eq!(
        pixel(frame, (4.0 * scale) as u32, (4.0 * scale) as u32)?,
        BLUE
    );
    Ok(())
}

#[test]
fn physical_scroll_delta_commits_css_offsets_and_clamps_both_axes() -> TestResult {
    let mut session = open_scroll_session(DEVICE_SCALE_FACTOR)?;
    let initial_generation = required_metrics(&session)?.generation;
    let fixed_before = pixel(session.latest_frame().ok_or("initial frame missing")?, 8, 8)?;
    assert_eq!(fixed_before, BLUE);
    session
        .take_frame_update_with_metrics()
        .ok_or("initial update missing")?;

    session
        .dispatch_input(HtmlBrowserInput::Scroll {
            delta_x: PHYSICAL_SCROLL_DELTA_X,
            delta_y: PHYSICAL_SCROLL_DELTA_Y,
        })
        .map_err(|error| error.to_string())?;
    let (frame, metrics) = session
        .latest_frame_with_metrics()
        .ok_or_else(|| "scrolled frame and metrics must be paired".to_string())?;
    assert!(frame.generation > initial_generation);
    assert_eq!(metrics.generation, frame.generation);
    assert_eq!(metrics.scroll_x, 400.0);
    assert_eq!(metrics.scroll_y, 300.0);
    assert_eq!(metrics.max_scroll_x(), 480.0);
    assert_eq!(metrics.max_scroll_y(), 360.0);
    assert_eq!(pixel(frame, 8, 8)?, fixed_before);
    Ok(())
}

#[test]
fn pending_update_consumption_is_shared_and_hidden_x_rejects_user_scroll() -> TestResult {
    let source = HtmlBrowserSource::new(
        SCROLL_FIXTURE.replace("overflow:scroll", "overflow-x:hidden;overflow-y:scroll"),
        SOURCE_ORIGIN,
    )
    .map_err(|error| error.to_string())?;
    let mut session = HtmlBrowserSession::new(source, viewport_for_scale(DEVICE_SCALE_FACTOR)?)
        .map_err(|error| error.to_string())?;
    session
        .take_frame_update_with_metrics()
        .ok_or("initial update missing")?;

    session
        .dispatch_input(HtmlBrowserInput::Scroll {
            delta_x: PHYSICAL_SCROLL_DELTA_X,
            delta_y: PHYSICAL_SCROLL_DELTA_Y,
        })
        .map_err(|error| error.to_string())?;
    let metrics = required_metrics(&session)?;
    assert_eq!(metrics.overflow_x, HtmlBrowserOverflowPolicy::Hidden);
    assert_eq!(metrics.overflow_y, HtmlBrowserOverflowPolicy::Scroll);
    assert_eq!(metrics.scroll_x, 0.0);
    assert_eq!(metrics.scroll_y, 300.0);
    assert!(session.take_frame_update().is_some());
    assert!(session.take_frame_update_with_metrics().is_none());
    Ok(())
}

#[test]
fn close_clears_committed_metrics_with_frame() -> TestResult {
    let mut session = open_scroll_session(DEVICE_SCALE_FACTOR)?;
    assert!(session.latest_frame_with_metrics().is_some());
    session.close().map_err(|error| error.to_string())?;
    assert!(session.latest_frame().is_none());
    assert!(session.latest_scroll_metrics().is_none());
    assert!(session.latest_frame_with_metrics().is_none());
    Ok(())
}
