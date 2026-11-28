use super::{HtmlBrowserError, HtmlBrowserSession, HtmlBrowserSessionState};

impl HtmlBrowserSession {
    pub(super) fn close_on_failure<T>(
        &mut self,
        result: Result<T, HtmlBrowserError>,
    ) -> Result<T, HtmlBrowserError> {
        match result {
            Err(error) => {
                let _ = self.close();
                Err(error)
            }
            Ok(value) => Ok(value),
        }
    }

    pub(super) fn resize_or_close_on_failure(
        &mut self,
        viewport: super::HtmlBrowserViewport,
    ) -> Result<(), HtmlBrowserError> {
        let result = (|| {
            let interactive = self
                .interactive
                .as_mut()
                .ok_or(HtmlBrowserError::RuntimeNotStarted)?;
            interactive.resize(viewport)?;
            self.viewport = viewport;
            self.sync_interactive_state()
        })();
        self.close_on_failure(result)
    }

    pub fn close(&mut self) -> Result<(), HtmlBrowserError> {
        if self.state == HtmlBrowserSessionState::Closed {
            return Ok(());
        }
        self.state = HtmlBrowserSessionState::Closed;
        self.interactive = None;
        let raw_html = std::mem::take(&mut self.source.raw_html);
        drop(raw_html);
        self.latest_frame = None;
        self.latest_scroll_metrics = None;
        self.frame_update_pending = false;
        self.pending_navigation = None;
        Ok(())
    }
}

impl Drop for HtmlBrowserSession {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
