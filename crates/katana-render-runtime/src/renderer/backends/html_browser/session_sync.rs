use super::{
    HtmlBrowserError, HtmlBrowserFrame, HtmlBrowserNavigationEvent, HtmlBrowserScrollMetrics,
    HtmlBrowserSession, HtmlInteractiveSession,
};

struct InteractiveUpdates {
    frame: Option<HtmlBrowserFrame>,
    metrics: Option<HtmlBrowserScrollMetrics>,
    navigation: Option<HtmlBrowserNavigationEvent>,
}

struct InteractiveCandidates<'a> {
    frame: Option<&'a HtmlBrowserFrame>,
    metrics: Option<HtmlBrowserScrollMetrics>,
}

impl HtmlBrowserSession {
    pub(super) fn sync_interactive_state(&mut self) -> Result<(), HtmlBrowserError> {
        let updates = self.take_interactive_updates()?;
        self.publish_prepared_interactive_frame(updates.frame, updates.metrics);
        if updates.navigation.is_some() {
            self.pending_navigation = updates.navigation;
        }
        Ok(())
    }

    fn take_interactive_updates(&mut self) -> Result<InteractiveUpdates, HtmlBrowserError> {
        let latest_generation = self.latest_frame.as_ref().map(|frame| frame.generation);
        let navigation = self.take_interactive_navigation()?;
        let candidates =
            Self::candidate_interactive_updates(self.interactive.as_ref(), latest_generation)?;
        let transfer = Self::prepare_interactive_frame(
            self.state,
            &mut self.source,
            &mut self.latest_frame,
            &mut self.frame_update_pending,
            candidates.frame,
        )?;
        let metrics = candidates.metrics;
        let frame = self.take_prepared_interactive_frame(transfer);
        Ok(InteractiveUpdates {
            frame,
            metrics,
            navigation,
        })
    }

    fn candidate_interactive_updates(
        interactive: Option<&HtmlInteractiveSession>,
        latest_generation: Option<u64>,
    ) -> Result<InteractiveCandidates<'_>, HtmlBrowserError> {
        let interactive = interactive.ok_or(HtmlBrowserError::RuntimeNotStarted)?;
        let candidate_metrics = interactive
            .latest_scroll_metrics()
            .filter(|metrics| latest_generation != Some(metrics.generation))
            .copied();
        let candidate_frame = interactive
            .latest_frame()
            .filter(|frame| latest_generation != Some(frame.generation));
        Self::validate_optional_interactive_pair(candidate_frame, candidate_metrics.as_ref())?;
        Ok(InteractiveCandidates {
            frame: candidate_frame,
            metrics: candidate_metrics,
        })
    }

    fn validate_optional_interactive_pair(
        frame: Option<&HtmlBrowserFrame>,
        metrics: Option<&HtmlBrowserScrollMetrics>,
    ) -> Result<(), HtmlBrowserError> {
        match (frame, metrics) {
            (Some(frame), Some(metrics)) => Self::validate_interactive_pair(frame, metrics),
            (None, None) => Ok(()),
            _ => Err(HtmlBrowserError::RuntimeFailure {
                error: "interactive frame and scroll metrics were not published as a pair"
                    .to_owned(),
            }),
        }
    }

    fn take_interactive_navigation(
        &mut self,
    ) -> Result<Option<HtmlBrowserNavigationEvent>, HtmlBrowserError> {
        self.interactive
            .as_mut()
            .ok_or(HtmlBrowserError::RuntimeNotStarted)
            .map(|interactive| interactive.take_navigation())
    }

    #[cfg(test)]
    pub(super) fn sync_interactive_frame(
        &mut self,
        frame: Option<HtmlBrowserFrame>,
    ) -> Result<(), HtmlBrowserError> {
        if let Some(frame) = frame
            && self
                .latest_frame
                .as_ref()
                .is_none_or(|latest| latest.generation != frame.generation)
        {
            self.accept_interactive_frame(frame)?;
        }
        Ok(())
    }

    #[cfg(test)]
    fn sync_interactive_frame_with_metrics(
        &mut self,
        frame: Option<HtmlBrowserFrame>,
        metrics: Option<HtmlBrowserScrollMetrics>,
    ) -> Result<(), HtmlBrowserError> {
        if let Some(frame) = frame
            && self
                .latest_frame
                .as_ref()
                .is_none_or(|latest| latest.generation != frame.generation)
        {
            self.accept_interactive_frame_with_metrics(
                frame,
                metrics.ok_or_else(|| HtmlBrowserError::RuntimeFailure {
                    error: "interactive frame is missing scroll metrics".to_owned(),
                })?,
            )?;
        } else if let Some(metrics) = metrics {
            let frame =
                self.latest_frame
                    .as_ref()
                    .ok_or_else(|| HtmlBrowserError::RuntimeFailure {
                        error: "interactive scroll metrics are missing their frame".to_owned(),
                    })?;
            Self::validate_interactive_pair(frame, &metrics)?;
            self.latest_scroll_metrics = Some(metrics);
        }
        Ok(())
    }

    fn validate_interactive_pair(
        frame: &HtmlBrowserFrame,
        metrics: &HtmlBrowserScrollMetrics,
    ) -> Result<(), HtmlBrowserError> {
        if frame.generation != metrics.generation
            || frame.viewport.logical_width() != metrics.viewport_width
            || frame.viewport.logical_height() != metrics.viewport_height
            || frame.scroll_y != metrics.scroll_y
            || frame.content_height != metrics.content_height
        {
            return Err(HtmlBrowserError::RuntimeFailure {
                error: "interactive frame and scroll metrics generation/viewport mismatch"
                    .to_owned(),
            });
        }
        Ok(())
    }

    #[cfg(test)]
    fn accept_interactive_frame_with_metrics(
        &mut self,
        frame: HtmlBrowserFrame,
        metrics: HtmlBrowserScrollMetrics,
    ) -> Result<(), HtmlBrowserError> {
        Self::validate_interactive_pair(&frame, &metrics)?;
        let previous_origin = self.source.origin.clone();
        if previous_origin.is_same_document_fragment_navigation(&frame.origin) {
            self.source.origin = frame.origin.clone();
        }
        match self.accept_frame(frame) {
            Ok(()) => {
                self.latest_scroll_metrics = Some(metrics);
                Ok(())
            }
            Err(error) => {
                self.source.origin = previous_origin;
                Err(error)
            }
        }
    }

    #[cfg(test)]
    fn accept_interactive_frame(
        &mut self,
        frame: HtmlBrowserFrame,
    ) -> Result<(), HtmlBrowserError> {
        let previous_origin = self.source.origin.clone();
        if previous_origin.is_same_document_fragment_navigation(&frame.origin) {
            self.source.origin = frame.origin.clone();
        }
        match self.accept_frame(frame) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.source.origin = previous_origin;
                Err(error)
            }
        }
    }
}

#[cfg(test)]
#[path = "session_sync_contract_tests.rs"]
mod contract_tests;

#[cfg(test)]
#[path = "session_frame_transfer_tests.rs"]
mod frame_transfer_tests;

#[cfg(test)]
#[path = "session_sync_real_frame_tests.rs"]
mod real_frame_tests;
