use crate::{HtmlBrowserFrame, HtmlBrowserSource, HtmlBrowserViewport, HtmlRuntime};

const ORIGIN: &str = "https://example.test/flex-cross-size.html";
const VIEWPORT_WIDTH: u32 = 160;
const VIEWPORT_HEIGHT: u32 = 80;
const DEVICE_SCALE_FACTOR: f32 = 1.0;
const PARENT_WIDTH: usize = 120;
const RGBA_CHANNELS: usize = 4;
const RED: [u8; RGBA_CHANNELS] = [255, 0, 0, 255];
const MIN_RED_PIXELS: usize = 8;
const STRETCH: &str = "stretch";
const FLEX_START: &str = "flex-start";

#[test]
fn flex_column_nonstretch_and_stretch_keep_distinct_public_widths() -> Result<(), String> {
    let stretched = render_red_band_width(STRETCH)?;
    let nonstretched = render_red_band_width(FLEX_START)?;
    assert_eq!(stretched, PARENT_WIDTH, "stretch red band width");
    assert!(nonstretched > 0, "flex-start red band must be visible");
    assert!(
        nonstretched < stretched,
        "flex-start child must keep intrinsic width"
    );
    Ok(())
}

fn render_red_band_width(align_items: &str) -> Result<usize, String> {
    let html = format!(
        "<body style='margin:0'><div style='display:flex;flex-direction:column;align-items:{align_items};width:{PARENT_WIDTH}px;height:40px'><div style='margin:0;height:12px;background:#ff0000;font-family:Arial;font-size:16px;line-height:20px'>short</div></div></body>"
    );
    let source = HtmlBrowserSource::new(html, ORIGIN).map_err(|error| error.to_string())?;
    let viewport = HtmlBrowserViewport::new(VIEWPORT_WIDTH, VIEWPORT_HEIGHT, DEVICE_SCALE_FACTOR)
        .map_err(|error| error.to_string())?;
    let mut session = HtmlRuntime
        .open(source, viewport)
        .map_err(|error| error.to_string())?;
    let width = session
        .latest_frame()
        .ok_or_else(|| "flex session returned no frame".to_owned())
        .and_then(red_band_width);
    let closed = session.close().map_err(|error| error.to_string());
    match (width, closed) {
        (Ok(width), Ok(())) => Ok(width),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(frame_error), Err(close_error)) => Err(format!("{frame_error}; {close_error}")),
    }
}

fn red_band_width(frame: &HtmlBrowserFrame) -> Result<usize, String> {
    let width = frame.viewport.width as usize;
    let expected_pixels = width * frame.viewport.height as usize;
    let pixels = frame.pixels.chunks_exact(RGBA_CHANNELS);
    if width == 0 || !pixels.remainder().is_empty() || pixels.len() != expected_pixels {
        return Err("flex frame pixel dimensions are invalid".to_owned());
    }
    let mut left = usize::MAX;
    let mut right = 0;
    let mut count = 0;
    for (index, pixel) in pixels.enumerate() {
        if pixel == RED {
            let x = index % width;
            left = left.min(x);
            right = right.max(x);
            count += 1;
        }
    }
    if count < MIN_RED_PIXELS {
        return Err(format!(
            "flex red band is not visibly rendered: {count} pixels"
        ));
    }
    Ok(right - left + 1)
}
