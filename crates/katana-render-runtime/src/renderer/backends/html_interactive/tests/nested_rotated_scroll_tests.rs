use super::support::{TestResult, start_with_viewport, to_string};
use crate::renderer::backends::html_browser::{
    HtmlBrowserFrame, HtmlBrowserInput, HtmlBrowserScrollMetrics,
};

const VIEWPORT_SIZE: u32 = 160;
const OUTER_ORIGIN: f32 = 120.0;
const OUTER_HALF_SIZE: f32 = 50.0;
const EXTENT_TOLERANCE: f32 = 0.5;
const SCROLL_DISTANCE: f32 = 400.0;
const EXPECTED_SCROLL: u32 = 81;
const INSIDE_WORLD: u32 = 180;
const OUTSIDE_WORLD: u32 = 212;
const RGBA_CHANNELS: usize = 4;
const RED: [u8; RGBA_CHANNELS] = [255, 0, 0, 255];
const WHITE: [u8; RGBA_CHANNELS] = [255, 255, 255, 255];

struct Capture {
    frame: HtmlBrowserFrame,
    metrics: HtmlBrowserScrollMetrics,
}

fn document(child: &str) -> String {
    format!(
        "<style>html,body{{margin:0}} #background{{position:fixed;left:0;top:0;width:160px;height:160px;background:white}} #outer{{position:absolute;left:120px;top:120px;width:100px;height:100px;overflow:hidden;transform:rotate(45deg)}} #inner{{position:absolute;left:55px;top:25px;width:50px;height:50px;overflow:hidden;transform:rotate(-45deg)}} #paint{{position:absolute;background:red;{child}}}</style><div id='background'></div><div id='outer'><div id='inner'><div id='paint'></div></div></div>"
    )
}

fn capture(child: &str) -> TestResult<Capture> {
    let mut session = start_with_viewport(&document(child), VIEWPORT_SIZE, VIEWPORT_SIZE)?;
    session
        .dispatch_input(HtmlBrowserInput::Scroll {
            delta_x: SCROLL_DISTANCE,
            delta_y: SCROLL_DISTANCE,
        })
        .map_err(to_string)?;
    let frame = session.latest_frame().ok_or("frame missing")?.clone();
    let metrics = *session
        .latest_scroll_metrics()
        .ok_or("scroll metrics missing")?;
    Ok(Capture { frame, metrics })
}

fn pixel(frame: &HtmlBrowserFrame, world: u32) -> TestResult<&[u8]> {
    let coordinate = world
        .checked_sub(EXPECTED_SCROLL)
        .ok_or("world coordinate before frame")?;
    let pixel = coordinate as usize * VIEWPORT_SIZE as usize + coordinate as usize;
    let start = pixel * RGBA_CHANNELS;
    frame
        .pixels
        .get(start..start + RGBA_CHANNELS)
        .ok_or("sample outside frame".to_owned())
}

fn assert_scroll_geometry(metrics: &HtmlBrowserScrollMetrics) {
    let extent = OUTER_ORIGIN + OUTER_HALF_SIZE * (1.0 + std::f32::consts::SQRT_2);
    assert!((metrics.content_width - extent).abs() < EXTENT_TOLERANCE);
    assert!((metrics.content_height - extent).abs() < EXTENT_TOLERANCE);
    assert_eq!(metrics.max_scroll_x(), EXPECTED_SCROLL as f32);
    assert_eq!(metrics.max_scroll_y(), EXPECTED_SCROLL as f32);
    assert_eq!(metrics.scroll_x, EXPECTED_SCROLL as f32);
    assert_eq!(metrics.scroll_y, EXPECTED_SCROLL as f32);
}

#[test]
fn nested_rotated_clips_bound_both_scroll_axes_and_pixels() -> TestResult {
    let oversized = capture("left:-100px;top:-100px;width:300px;height:300px")?;
    // 塗りの端とclip境界を離し、両方の境界でのアンチエイリアス合成を比較から外す。
    let slightly_oversized = capture("left:-2px;top:-2px;width:54px;height:54px")?;
    assert_scroll_geometry(&oversized.metrics);
    assert_scroll_geometry(&slightly_oversized.metrics);
    assert_eq!(oversized.frame.viewport, slightly_oversized.frame.viewport);
    assert!(oversized.frame.pixels == slightly_oversized.frame.pixels);
    assert_eq!(pixel(&oversized.frame, INSIDE_WORLD)?, RED);
    assert_eq!(pixel(&oversized.frame, OUTSIDE_WORLD)?, WHITE);
    Ok(())
}
