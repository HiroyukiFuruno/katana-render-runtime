use super::super::{HtmlBrowserError, HtmlInteractiveSession, runtime_failure};

impl HtmlInteractiveSession {
    pub(super) fn scroll(&mut self, delta_x: f32, delta_y: f32) -> Result<(), HtmlBrowserError> {
        self.resize_anchor = None;
        let (next_x, next_scroll) = self.user_scroll_offset(delta_x, delta_y);
        if next_scroll == self.scroll_geometry.scroll_y && next_x == self.scroll_geometry.scroll_x {
            return Ok(());
        }
        self.scroll_geometry.scroll_y = next_scroll;
        self.scroll_geometry.scroll_x = next_x;
        self.refresh_scroll_layout()?;
        if self.sync_intersection_geometry_from_current_layout()? {
            /* WHY: Observer callbacks can mutate the DOM. The scroll listener
             * must observe the resulting geometry, not the preceding layout. */
            self.refresh_scroll_layout()?;
            self.scroll_geometry.scroll_y = self.scroll_geometry.scroll_y.min(self.max_scroll());
            self.scroll_geometry.scroll_x = self.scroll_geometry.scroll_x.min(self.max_scroll_x());
            self.sync_intersection_geometry_from_current_layout()?;
        }
        self.runtime
            .dispatch_window_scroll()
            .map_err(runtime_failure)?;
        self.render_frame()
    }

    fn user_scroll_offset(&self, delta_x: f32, delta_y: f32) -> (f32, f32) {
        let logical_delta = delta_y / self.viewport.device_scale_factor;
        use super::super::super::html_browser::HtmlBrowserOverflowPolicy;
        let allows_user_scroll = |policy| {
            matches!(
                policy,
                HtmlBrowserOverflowPolicy::Auto | HtmlBrowserOverflowPolicy::Scroll
            )
        };
        let next_x = if allows_user_scroll(self.scroll_geometry.viewport_overflow.0) {
            (self.scroll_geometry.scroll_x + delta_x / self.viewport.device_scale_factor)
                .clamp(0.0, self.max_scroll_x())
        } else {
            self.scroll_geometry.scroll_x
        };
        let next_scroll = if allows_user_scroll(self.scroll_geometry.viewport_overflow.1) {
            (self.scroll_geometry.scroll_y + logical_delta).clamp(0.0, self.max_scroll())
        } else {
            self.scroll_geometry.scroll_y
        };
        (next_x, next_scroll)
    }

    fn refresh_scroll_layout(&mut self) -> Result<(), HtmlBrowserError> {
        let layout = self.layout()?;
        self.store_layout_geometry(layout);
        Ok(())
    }
}
