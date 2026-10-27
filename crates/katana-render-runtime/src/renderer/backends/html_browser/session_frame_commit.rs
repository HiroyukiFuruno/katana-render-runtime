use super::super::HtmlBrowserOrigin;
use super::{
    HtmlBrowserError, HtmlBrowserFrame, HtmlBrowserScrollMetrics, HtmlBrowserSession,
    HtmlBrowserSessionState, HtmlBrowserSource, HtmlInteractiveSession,
};

impl HtmlBrowserSession {
    pub(super) fn take_prepared_interactive_frame(
        &mut self,
        transfer: bool,
    ) -> Option<HtmlBrowserFrame> {
        if !transfer {
            return None;
        }
        self.interactive
            .as_mut()
            .and_then(HtmlInteractiveSession::take_frame)
    }

    pub(super) fn publish_prepared_interactive_frame(
        &mut self,
        frame: Option<HtmlBrowserFrame>,
        metrics: Option<HtmlBrowserScrollMetrics>,
    ) {
        /* WHY: 全検証を所有権移動前に完了し、受理後は再試行用frameを失う失敗点を作らない。 */
        if let Some(frame) = frame {
            self.source.origin = frame.origin.clone();
            self.latest_frame = Some(frame);
            self.frame_update_pending = true;
        }
        if let Some(metrics) = metrics {
            self.latest_scroll_metrics = Some(metrics);
        }
        /* WHY: 公開側で受理した世代だけinner画素を解放し、未受理frameは再試行に残す。 */
        if let (Some(interactive), Some(generation)) = (
            self.interactive.as_mut(),
            self.latest_frame.as_ref().map(|frame| frame.generation),
        ) {
            interactive.release_frame_if_generation(generation);
        }
    }

    pub(super) fn validate_frame(
        state: HtmlBrowserSessionState,
        origin: &HtmlBrowserOrigin,
        latest: Option<&HtmlBrowserFrame>,
        frame: &HtmlBrowserFrame,
    ) -> Result<(), HtmlBrowserError> {
        (state == HtmlBrowserSessionState::Active)
            .then_some(())
            .ok_or(HtmlBrowserError::SessionClosed)?;
        if frame.origin != *origin {
            return Err(HtmlBrowserError::FrameOriginMismatch {
                expected: origin.as_str().to_owned(),
                actual: frame.origin.as_str().to_owned(),
            });
        }
        if let Some(latest) = latest
            && frame.generation <= latest.generation
        {
            return Err(HtmlBrowserError::StaleFrameGeneration {
                latest: latest.generation,
                received: frame.generation,
            });
        }
        Ok(())
    }

    pub(super) fn prepare_interactive_frame(
        state: HtmlBrowserSessionState,
        source: &mut HtmlBrowserSource,
        latest: &mut Option<HtmlBrowserFrame>,
        frame_update_pending: &mut bool,
        frame: Option<&HtmlBrowserFrame>,
    ) -> Result<bool, HtmlBrowserError> {
        let Some(frame) = frame else {
            return Ok(false);
        };
        if let Some(current) = latest.as_mut()
            && Self::can_reuse_pixels(current, frame)
        {
            Self::accept_borrowed_interactive_frame(
                state,
                source,
                current,
                frame_update_pending,
                frame,
            )?;
            return Ok(false);
        }
        Self::validate_interactive_frame(state, &source.origin, latest.as_ref(), frame)?;
        Ok(true)
    }

    fn can_reuse_pixels(current: &HtmlBrowserFrame, frame: &HtmlBrowserFrame) -> bool {
        current.viewport == frame.viewport
            && current.pixels.len() == frame.pixels.len()
            && current.pixels.capacity() >= frame.pixels.len()
    }

    pub(super) fn accept_borrowed_interactive_frame(
        state: HtmlBrowserSessionState,
        source: &mut HtmlBrowserSource,
        current: &mut HtmlBrowserFrame,
        frame_update_pending: &mut bool,
        frame: &HtmlBrowserFrame,
    ) -> Result<(), HtmlBrowserError> {
        let fragment =
            Self::validate_interactive_frame(state, &source.origin, Some(current), frame)?;
        let next_origin = fragment.then(|| frame.origin.clone());
        Self::copy_same_length_frame(current, frame);
        if let Some(origin) = next_origin {
            source.origin = origin;
        }
        *frame_update_pending = true;
        Ok(())
    }

    fn validate_interactive_frame(
        state: HtmlBrowserSessionState,
        origin: &HtmlBrowserOrigin,
        latest: Option<&HtmlBrowserFrame>,
        frame: &HtmlBrowserFrame,
    ) -> Result<bool, HtmlBrowserError> {
        let fragment = origin.is_same_document_fragment_navigation(&frame.origin);
        let expected_origin = if fragment { &frame.origin } else { origin };
        Self::validate_frame(state, expected_origin, latest, frame)?;
        Ok(fragment)
    }

    fn copy_same_length_frame(current: &mut HtmlBrowserFrame, frame: &HtmlBrowserFrame) {
        let origin = frame.origin.clone();
        /* WHY: 所有済みスナップショットは独立Vecなので、検証後の同長コピーで利用者の画素を変えない。 */
        current.pixels.clone_from(&frame.pixels);
        current.generation = frame.generation;
        current.origin = origin;
        current.viewport = frame.viewport;
        current.scroll_y = frame.scroll_y;
        current.content_height = frame.content_height;
        current.pixel_format = frame.pixel_format;
    }
}
