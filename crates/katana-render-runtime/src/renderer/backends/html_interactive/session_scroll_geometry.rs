use super::super::html_browser::HtmlBrowserOverflowPolicy;

#[derive(Clone, Copy)]
pub(super) struct HtmlInteractiveScrollGeometry {
    pub(super) scroll_x: f32,
    pub(super) scroll_y: f32,
    pub(super) content_width: f32,
    pub(super) content_height: f32,
    pub(super) viewport_overflow: (HtmlBrowserOverflowPolicy, HtmlBrowserOverflowPolicy),
}

impl Default for HtmlInteractiveScrollGeometry {
    fn default() -> Self {
        Self {
            scroll_x: 0.0,
            scroll_y: 0.0,
            content_width: 0.0,
            content_height: 0.0,
            viewport_overflow: (
                HtmlBrowserOverflowPolicy::Auto,
                HtmlBrowserOverflowPolicy::Auto,
            ),
        }
    }
}
