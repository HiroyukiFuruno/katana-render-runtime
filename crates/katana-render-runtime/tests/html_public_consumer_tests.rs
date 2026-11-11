use katana_render_runtime::{
    HtmlBrowserFrame, HtmlBrowserInput, HtmlBrowserSession, HtmlBrowserSource, HtmlBrowserViewport,
    HtmlRuntime,
};
use std::error::Error;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const SCALE_FACTORS: [f32; 2] = [1.0, 2.0];
const WIDTH: u32 = 320;
const HEIGHT: u32 = 240;
const CONTENT_WIDTH: u32 = 800;
const CONTENT_HEIGHT: u32 = 600;
const SCROLL_X: f32 = 400.0;
const SCROLL_Y: f32 = 300.0;
const TARGET_X: f32 = 430.0;
const TARGET_Y: f32 = 325.0;
const CHANNELS: usize = 4;
const BLUE: [u8; CHANNELS] = [0, 0, 255, 255];
const RED: [u8; CHANNELS] = [255, 0, 0, 255];
const SOURCE: &str = r#"<!doctype html><html><body style="margin:0;overflow:scroll;background:white"><button style="position:absolute;left:420px;top:320px;width:40px;height:40px;border:0;background:blue" onclick="this.style.background='red'"></button><div style="width:800px;height:600px"></div></body></html>"#;
const FOCUS_MARKER_X: u32 = 248;
const FOCUS_CASES: [(&str, u32); 2] = [("query", 8), ("note", 48)];
const FOCUS_SOURCE: &str = r#"<!doctype html><html><body style="margin:0;background:white">
<div id=search-result style="position:absolute;left:240px;top:0;width:32px;height:32px;background:blue"></div>
<input id=search type=search style="display:block;width:140px">
<div id=area-result style="position:absolute;left:240px;top:40px;width:32px;height:32px;background:blue"></div>
<textarea id=area style="display:block;width:140px"></textarea>
<script>
const search = document.getElementById('search');
search.addEventListener('input', () => {
  if (search.value === 'query') document.getElementById('search-result').style.background = 'red';
});
const area = document.getElementById('area');
area.addEventListener('input', (event) => {
  if (event.target === area && document.activeElement === area) {
    document.getElementById('area-result').style.background = 'red';
  }
});
</script>
</body></html>"#;

#[test]
fn public_runtime_preserves_xy_input_and_state_across_resize() -> TestResult {
    for scale in SCALE_FACTORS {
        exercise_public_session(scale)?;
    }
    Ok(())
}

#[test]
fn public_runtime_tabs_from_search_to_textarea_and_dispatches_input() -> TestResult {
    let source = HtmlBrowserSource::new(FOCUS_SOURCE, "https://example.test/focus.html")?;
    let viewport = HtmlBrowserViewport::new(WIDTH, HEIGHT, 1.0)?;
    let mut session = HtmlRuntime.open(source, viewport)?;
    for (text, marker_y) in FOCUS_CASES {
        let before = session.latest_frame().ok_or("initial frame missing")?;
        assert_eq!(pixel(before, FOCUS_MARKER_X, marker_y)?, BLUE);
        session.dispatch_input(HtmlBrowserInput::KeyDown { key: "Tab".into() })?;
        session.dispatch_input(HtmlBrowserInput::Text { text: text.into() })?;
        let frame = session.latest_frame().ok_or("typed frame missing")?;
        assert_eq!(pixel(frame, FOCUS_MARKER_X, marker_y)?, RED, "typed {text}");
    }
    Ok(())
}

fn exercise_public_session(scale: f32) -> TestResult {
    let source = HtmlBrowserSource::new(SOURCE, "https://example.test/consumer.html")?;
    let viewport = scaled_viewport(WIDTH, HEIGHT, scale)?;
    let mut session = HtmlRuntime.open(source, viewport)?;
    let initial_generation = assert_frame(&session, viewport, 0.0, 0.0)?;
    session.dispatch_input(HtmlBrowserInput::Scroll {
        delta_x: SCROLL_X * scale,
        delta_y: SCROLL_Y * scale,
    })?;
    let scrolled_generation = assert_frame(&session, viewport, SCROLL_X, SCROLL_Y)?;
    assert!(scrolled_generation > initial_generation);
    assert_target(&session, BLUE)?;
    click_target(&mut session, scale)?;
    assert_target(&session, RED)?;
    let resized = scaled_viewport(CONTENT_WIDTH, CONTENT_HEIGHT, scale)?;
    session.resize(resized)?;
    let resized_generation = assert_frame(&session, resized, 0.0, 0.0)?;
    assert!(resized_generation > scrolled_generation);
    assert_target(&session, RED)?;
    Ok(())
}

fn scaled_viewport(width: u32, height: u32, scale: f32) -> TestResult<HtmlBrowserViewport> {
    Ok(HtmlBrowserViewport::new(
        (width as f32 * scale) as u32,
        (height as f32 * scale) as u32,
        scale,
    )?)
}

fn assert_frame(
    session: &HtmlBrowserSession,
    viewport: HtmlBrowserViewport,
    scroll_x: f32,
    scroll_y: f32,
) -> TestResult<u64> {
    let (frame, metrics) = session
        .latest_frame_with_metrics()
        .ok_or("frame and metrics missing")?;
    assert_eq!(frame.viewport, viewport);
    assert_eq!(
        frame.pixels.len(),
        viewport.width as usize * viewport.height as usize * CHANNELS
    );
    assert_eq!(frame.generation, metrics.generation);
    assert_eq!(
        (metrics.viewport_width, metrics.viewport_height),
        (viewport.logical_width(), viewport.logical_height())
    );
    assert_eq!(
        (metrics.content_width, metrics.content_height),
        (CONTENT_WIDTH as f32, CONTENT_HEIGHT as f32)
    );
    assert_eq!((metrics.scroll_x, metrics.scroll_y), (scroll_x, scroll_y));
    Ok(frame.generation)
}

fn click_target(session: &mut HtmlBrowserSession, scale: f32) -> TestResult {
    let x = (TARGET_X - SCROLL_X) * scale;
    let y = (TARGET_Y - SCROLL_Y) * scale;
    session.dispatch_input(HtmlBrowserInput::PointerDown { x, y, button: 0 })?;
    session.dispatch_input(HtmlBrowserInput::PointerUp { x, y, button: 0 })?;
    Ok(())
}

fn assert_target(session: &HtmlBrowserSession, expected: [u8; CHANNELS]) -> TestResult {
    let (frame, metrics) = session
        .latest_frame_with_metrics()
        .ok_or("target frame missing")?;
    let scale = frame.viewport.device_scale_factor;
    let x = ((TARGET_X - metrics.scroll_x) * scale).floor() as u32;
    let y = ((TARGET_Y - metrics.scroll_y) * scale).floor() as u32;
    assert_eq!(pixel(frame, x, y)?, expected);
    Ok(())
}

fn pixel(frame: &HtmlBrowserFrame, x: u32, y: u32) -> TestResult<[u8; CHANNELS]> {
    let offset = (y as usize * frame.viewport.width as usize + x as usize) * CHANNELS;
    Ok(frame
        .pixels
        .get(offset..offset + CHANNELS)
        .ok_or("pixel outside frame")?
        .try_into()?)
}
