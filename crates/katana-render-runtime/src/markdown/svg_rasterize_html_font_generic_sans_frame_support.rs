const INVALID_FRAME_VARIANTS: usize = 4;

pub(super) fn invalid_frames(
    frame: &crate::HtmlBrowserFrame,
) -> [crate::HtmlBrowserFrame; INVALID_FRAME_VARIANTS] {
    let mut width = frame.clone();
    width.viewport.width = 0;
    let mut height = frame.clone();
    height.viewport.height = 0;
    let mut pixels = frame.clone();
    pixels.pixels.clear();
    let mut overflow = frame.clone();
    overflow.viewport.width = u32::MAX;
    overflow.viewport.height = u32::MAX;
    [width, height, pixels, overflow]
}
