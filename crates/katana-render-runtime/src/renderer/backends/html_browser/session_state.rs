use super::{
    HtmlBrowserFrame, HtmlBrowserNavigationEvent, HtmlBrowserScrollMetrics, HtmlBrowserSession,
    HtmlBrowserSessionState, HtmlBrowserSource, HtmlBrowserViewport,
};

impl HtmlBrowserSession {
    pub fn source(&self) -> &HtmlBrowserSource {
        &self.source
    }

    pub fn viewport(&self) -> HtmlBrowserViewport {
        self.viewport
    }

    pub fn state(&self) -> HtmlBrowserSessionState {
        self.state
    }

    pub fn latest_frame(&self) -> Option<&HtmlBrowserFrame> {
        self.latest_frame.as_ref()
    }

    /// 最後に公開したframeと同じ世代の論理寸法・スクロール量を返す。
    pub fn latest_scroll_metrics(&self) -> Option<&HtmlBrowserScrollMetrics> {
        self.latest_scroll_metrics.as_ref()
    }

    /// 画素と同じ世代のmetricsを借用し、更新の未取得状態は変更しない。
    pub fn latest_frame_with_metrics(
        &self,
    ) -> Option<(&HtmlBrowserFrame, &HtmlBrowserScrollMetrics)> {
        let (frame, metrics) = (
            self.latest_frame.as_ref()?,
            self.latest_scroll_metrics.as_ref()?,
        );
        (frame.generation == metrics.generation).then_some((frame, metrics))
    }

    pub fn take_frame_update(&mut self) -> Option<&HtmlBrowserFrame> {
        if !self.frame_update_pending {
            return None;
        }
        self.frame_update_pending = false;
        self.latest_frame.as_ref()
    }

    /// 未取得のframe/metricsを一緒に取得する。frame単独の取得と消費状態を共有する。
    pub fn take_frame_update_with_metrics(
        &mut self,
    ) -> Option<(&HtmlBrowserFrame, &HtmlBrowserScrollMetrics)> {
        if !self.frame_update_pending || self.latest_frame_with_metrics().is_none() {
            return None;
        }
        self.frame_update_pending = false;
        self.latest_frame_with_metrics()
    }

    pub fn take_navigation(&mut self) -> Option<HtmlBrowserNavigationEvent> {
        self.pending_navigation.take()
    }

    pub fn has_in_process_runtime(&self) -> bool {
        self.interactive.is_some()
    }
}
