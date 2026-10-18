use super::HtmlInteractiveSession;
use super::{
    HtmlBrowserFrame, HtmlBrowserNavigationEvent, HtmlBrowserScrollMetrics, HtmlBrowserSource,
    HtmlBrowserViewport,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HtmlBrowserSessionState {
    Active,
    Closed,
}

pub struct HtmlBrowserSession {
    pub(super) source: HtmlBrowserSource,
    pub(super) viewport: HtmlBrowserViewport,
    pub(super) state: HtmlBrowserSessionState,
    pub(super) latest_frame: Option<HtmlBrowserFrame>,
    pub(super) latest_scroll_metrics: Option<HtmlBrowserScrollMetrics>,
    pub(super) frame_update_pending: bool,
    pub(super) pending_navigation: Option<HtmlBrowserNavigationEvent>,
    pub(super) interactive: Option<HtmlInteractiveSession>,
}
