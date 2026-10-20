use crate::{
    HtmlBrowserFrame, HtmlBrowserInput, HtmlBrowserScrollMetrics, HtmlBrowserSession,
    HtmlBrowserSource, HtmlBrowserViewport,
};

const LOGICAL_WIDTH: u32 = 48;
const LOGICAL_HEIGHT: u32 = 32;
const DOCUMENT_HEIGHT: f32 = 128.0;
const RECT_SIZE: f32 = 8.0;
const RGBA_CHANNELS: usize = 4;
const ORIGINS: [(f32, f32); 3] = [(0.49, 0.0), (0.5, 1.0), (-0.51, -1.0)];
const MAX_EDGE_DRIFT: i32 = 1;
const SVG_TOP: f32 = 12.0;
const SCROLL_ROOT_X: f32 = 4.0;
const FRACTIONAL_SCROLL: f32 = 0.75;
const SAME_SNAPPED_ORIGINS: [f32; 2] = [0.49, 0.24];

struct Capture {
    frame: HtmlBrowserFrame,
    metrics: HtmlBrowserScrollMetrics,
}

struct PixelBounds {
    left: usize,
    top: usize,
    right: usize,
    bottom: usize,
    count: usize,
}

#[test]
fn inline_svg_origin_snaps_in_css_space_before_fractional_scroll() -> Result<(), String> {
    for scale in [1.0, 2.0] {
        assert_css_origin_cases(scale)?;
        assert_scroll_order_case(scale)?;
    }
    Ok(())
}

fn assert_css_origin_cases(scale: f32) -> Result<(), String> {
    for (origin, snapped) in ORIGINS {
        let actual = capture(origin, 0.0, scale, 0.0)?;
        let expected = capture(snapped, 0.0, scale, 0.0)?;
        assert_capture_pair(&actual, &expected, scale, 0.0, snapped, SVG_TOP)?;
    }
    Ok(())
}

fn assert_scroll_order_case(scale: f32) -> Result<(), String> {
    let [first_y, second_y] = SAME_SNAPPED_ORIGINS;
    let first = capture(SCROLL_ROOT_X, first_y, scale, FRACTIONAL_SCROLL)?;
    let second = capture(SCROLL_ROOT_X, second_y, scale, FRACTIONAL_SCROLL)?;
    assert_capture_pair(
        &first,
        &second,
        scale,
        FRACTIONAL_SCROLL,
        SCROLL_ROOT_X,
        SVG_TOP,
    )
}

fn assert_capture_pair(
    actual: &Capture,
    expected: &Capture,
    scale: f32,
    scroll_y: f32,
    root_x: f32,
    root_y: f32,
) -> Result<(), String> {
    assert_session_contract(actual, scale, scroll_y);
    assert_session_contract(expected, scale, scroll_y);
    assert_eq!(actual.metrics, expected.metrics, "scroll extent changed");
    assert_eq!(actual.frame.pixels, expected.frame.pixels);
    assert_red_bounds(&actual.frame, root_x, root_y, scale, scroll_y)
}

fn assert_session_contract(capture: &Capture, scale: f32, scroll_y: f32) {
    let frame = &capture.frame;
    let metrics = &capture.metrics;
    assert_frame_dimensions(frame, scale);
    assert_eq!(frame.generation, metrics.generation);
    assert_eq!(
        (metrics.viewport_width, metrics.viewport_height),
        (LOGICAL_WIDTH as f32, LOGICAL_HEIGHT as f32)
    );
    assert_eq!((frame.scroll_y, metrics.scroll_y), (scroll_y, scroll_y));
    assert_eq!(metrics.scroll_x, 0.0);
    assert_eq!(
        (frame.content_height, metrics.content_height),
        (DOCUMENT_HEIGHT, DOCUMENT_HEIGHT)
    );
    assert_eq!(metrics.content_width, LOGICAL_WIDTH as f32);
    assert_eq!(metrics.max_scroll_x(), 0.0);
    assert_eq!(
        metrics.max_scroll_y(),
        DOCUMENT_HEIGHT - LOGICAL_HEIGHT as f32
    );
}

fn assert_frame_dimensions(frame: &HtmlBrowserFrame, scale: f32) {
    let physical_width = (LOGICAL_WIDTH as f32 * scale) as u32;
    let physical_height = (LOGICAL_HEIGHT as f32 * scale) as u32;
    assert_eq!(
        (frame.viewport.width, frame.viewport.height),
        (physical_width, physical_height)
    );
    assert_eq!(frame.viewport.logical_width(), LOGICAL_WIDTH as f32);
    assert_eq!(frame.viewport.logical_height(), LOGICAL_HEIGHT as f32);
    assert_eq!(
        frame.pixels.len(),
        physical_width as usize * physical_height as usize * RGBA_CHANNELS
    );
}

fn assert_red_bounds(
    frame: &HtmlBrowserFrame,
    root_x: f32,
    root_y: f32,
    scale: f32,
    scroll_y: f32,
) -> Result<(), String> {
    let bounds = red_bounds(frame)?;
    let expected_left = ((root_x * scale).floor() as i32).max(0);
    let expected_top = (((root_y - scroll_y) * scale).floor() as i32).max(0);
    let expected_right =
        (((root_x + RECT_SIZE) * scale).ceil() as i32 - 1).min(frame.viewport.width as i32 - 1);
    let expected_bottom = (((root_y - scroll_y + RECT_SIZE) * scale).ceil() as i32 - 1)
        .min(frame.viewport.height as i32 - 1);
    assert_coordinate(bounds.left, expected_left, "red left");
    assert_coordinate(bounds.top, expected_top, "red top");
    assert_coordinate(bounds.right, expected_right, "red right");
    assert_coordinate(bounds.bottom, expected_bottom, "red bottom");
    assert!(bounds.count > 0, "SVG red rectangle is missing");
    Ok(())
}

fn assert_coordinate(actual: usize, expected: i32, label: &str) {
    assert!(
        (actual as i32 - expected).abs() <= MAX_EDGE_DRIFT,
        "{label}={actual}, expected={expected} ±{MAX_EDGE_DRIFT} physical pixel"
    );
}

fn red_bounds(frame: &HtmlBrowserFrame) -> Result<PixelBounds, String> {
    let width = frame.viewport.width as usize;
    let mut bounds = PixelBounds {
        left: usize::MAX,
        top: usize::MAX,
        right: 0,
        bottom: 0,
        count: 0,
    };
    for (index, pixel) in frame.pixels.chunks_exact(RGBA_CHANNELS).enumerate() {
        if pixel[0] > pixel[1] && pixel[0] > pixel[2] {
            let x = index % width;
            let y = index / width;
            bounds.left = bounds.left.min(x);
            bounds.top = bounds.top.min(y);
            bounds.right = bounds.right.max(x);
            bounds.bottom = bounds.bottom.max(y);
            bounds.count += 1;
        }
    }
    if bounds.count == 0 {
        return Err("SVG rectangle produced no red-dominant pixels".to_owned());
    }
    Ok(bounds)
}

fn capture(root_x: f32, root_y: f32, scale: f32, scroll_y: f32) -> Result<Capture, String> {
    let html = format!(
        "<!doctype html><html><head><style>html,body{{margin:0}}body{{height:{DOCUMENT_HEIGHT}px;background:#1e1e1e}}</style></head><body><div style=\"height:12px\"></div><svg style=\"display:block;margin-left:{root_x}px;margin-top:{root_y}px\" width=\"8\" height=\"8\"><rect width=\"8\" height=\"8\" fill=\"red\"/></svg></body></html>"
    );
    let source = HtmlBrowserSource::new(&html, "https://example.test/svg-origin.html")
        .map_err(|error| error.to_string())?;
    let viewport = HtmlBrowserViewport::new(
        (LOGICAL_WIDTH as f32 * scale) as u32,
        (LOGICAL_HEIGHT as f32 * scale) as u32,
        scale,
    )
    .map_err(|error| error.to_string())?;
    let mut session =
        HtmlBrowserSession::new(source, viewport).map_err(|error| error.to_string())?;
    session
        .dispatch_input(HtmlBrowserInput::Scroll {
            delta_x: 0.0,
            delta_y: scroll_y * scale,
        })
        .map_err(|error| error.to_string())?;
    let (frame, metrics) = session
        .latest_frame_with_metrics()
        .ok_or_else(|| "frame/metrics pair missing".to_owned())?;
    Ok(Capture {
        frame: frame.clone(),
        metrics: *metrics,
    })
}
