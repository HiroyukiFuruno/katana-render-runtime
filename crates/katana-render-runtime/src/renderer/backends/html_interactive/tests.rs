mod chrome_interactions;
mod feature_diagnostic;
mod full_scroll_diagnostic;
mod inline_atomic_fragment;
mod input_focus;
mod input_pointer_frame;
mod interaction;
mod intersection_observer;
mod layout_diagnostic;
mod rendering;
mod support;
mod toc_diagnostic;

const INVALID_VIEWPORT_TEST_HEIGHT: u32 = 120;
const TEXT_X_ATTRIBUTE_PREFIX: &str = " x=\"";

pub(super) fn test_error(
    error: crate::renderer::backends::html_browser::HtmlBrowserError,
) -> String {
    support::to_string(error)
}

pub(super) fn assert_invalid_viewport_error() -> Result<(), String> {
    let error = crate::renderer::backends::html_browser::HtmlBrowserViewport::new(
        0,
        INVALID_VIEWPORT_TEST_HEIGHT,
        1.0,
    )
    .map_err(test_error)
    .err()
    .ok_or("invalid viewport error missing")?;
    assert_eq!(error, "browser viewport dimensions must be non-zero");
    Ok(())
}
pub(super) fn first_text_x(svg: &str) -> Option<f32> {
    svg.lines()
        .find(|line| line.contains("<text"))
        .and_then(|line| {
            let start = line.find(TEXT_X_ATTRIBUTE_PREFIX)? + TEXT_X_ATTRIBUTE_PREFIX.len();
            let suffix = line[start..].find('"')?;
            line[start..start + suffix].parse::<f32>().ok()
        })
}
