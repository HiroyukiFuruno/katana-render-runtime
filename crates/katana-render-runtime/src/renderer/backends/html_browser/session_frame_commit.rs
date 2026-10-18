use super::super::HtmlBrowserOrigin;
use super::{
    HtmlBrowserError, HtmlBrowserFrame, HtmlBrowserSession, HtmlBrowserSessionState,
    HtmlBrowserSource,
};

impl HtmlBrowserSession {
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
    ) -> Result<Option<HtmlBrowserFrame>, HtmlBrowserError> {
        let Some(frame) = frame else {
            return Ok(None);
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
            return Ok(None);
        }
        Ok(Some(frame.clone()))
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
        let fragment = source
            .origin
            .is_same_document_fragment_navigation(&frame.origin);
        let expected_origin = if fragment {
            &frame.origin
        } else {
            &source.origin
        };
        Self::validate_frame(state, expected_origin, Some(current), frame)?;
        let next_origin = fragment.then(|| frame.origin.clone());
        Self::copy_same_length_frame(current, frame);
        if let Some(origin) = next_origin {
            source.origin = origin;
        }
        *frame_update_pending = true;
        Ok(())
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
