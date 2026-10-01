use super::{
    HtmlBrowserError, HtmlBrowserFrame, HtmlBrowserInput, HtmlBrowserNavigation,
    HtmlBrowserNavigationEvent, HtmlBrowserSource, HtmlBrowserViewport,
};
use crate::renderer::backends::html_interactive::HtmlInteractiveSession;

#[path = "session_shutdown.rs"]
mod shutdown;
#[path = "session_sync.rs"]
mod sync;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HtmlBrowserSessionState {
    Active,
    Closed,
}

pub struct HtmlBrowserSession {
    source: HtmlBrowserSource,
    viewport: HtmlBrowserViewport,
    state: HtmlBrowserSessionState,
    latest_frame: Option<HtmlBrowserFrame>,
    frame_update_pending: bool,
    pending_navigation: Option<HtmlBrowserNavigationEvent>,
    interactive: Option<HtmlInteractiveSession>,
}

impl std::fmt::Debug for HtmlBrowserSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HtmlBrowserSession")
            .field("source", &self.source)
            .field("viewport", &self.viewport)
            .field("state", &self.state)
            .field("has_in_process_runtime", &self.interactive.is_some())
            .finish()
    }
}

impl HtmlBrowserSession {
    pub fn new(
        source: HtmlBrowserSource,
        viewport: HtmlBrowserViewport,
    ) -> Result<Self, HtmlBrowserError> {
        source.validate()?;
        viewport.validate()?;
        let interactive = HtmlInteractiveSession::start(source.clone(), viewport)?;
        let mut session = Self {
            source,
            viewport,
            state: HtmlBrowserSessionState::Active,
            latest_frame: None,
            frame_update_pending: false,
            pending_navigation: None,
            interactive: Some(interactive),
        };
        session.sync_interactive_state()?;
        Ok(session)
    }

    pub(crate) fn start_in_process(
        source: HtmlBrowserSource,
        viewport: HtmlBrowserViewport,
    ) -> Result<Self, HtmlBrowserError> {
        Self::new(source, viewport)
    }

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
    pub fn take_frame_update(&mut self) -> Option<&HtmlBrowserFrame> {
        if !self.frame_update_pending {
            return None;
        }
        self.frame_update_pending = false;
        self.latest_frame.as_ref()
    }
    pub fn take_navigation(&mut self) -> Option<HtmlBrowserNavigationEvent> {
        self.pending_navigation.take()
    }
    pub fn has_in_process_runtime(&self) -> bool {
        self.interactive.is_some()
    }

    pub fn navigate(&mut self, navigation: HtmlBrowserNavigation) -> Result<(), HtmlBrowserError> {
        self.ensure_active()?;
        navigation.source.validate()?;
        self.source = navigation.source;
        self.latest_frame = None;
        self.frame_update_pending = false;
        self.pending_navigation = None;
        /* WHY: A replacement session must be the sole owner of its V8 isolate. */
        self.interactive = None;
        self.interactive = Some(HtmlInteractiveSession::start(
            self.source.clone(),
            self.viewport,
        )?);
        self.sync_interactive_state()
    }

    pub fn resize(&mut self, viewport: HtmlBrowserViewport) -> Result<(), HtmlBrowserError> {
        self.ensure_active()?;
        viewport.validate()?;
        let interactive = self
            .interactive
            .as_mut()
            .ok_or(HtmlBrowserError::RuntimeNotStarted)?;
        interactive.resize(viewport)?;
        self.viewport = viewport;
        self.sync_interactive_state()
    }

    pub fn refresh_frame(&mut self) -> Result<(), HtmlBrowserError> {
        self.ensure_active()?;
        let interactive = self
            .interactive
            .as_mut()
            .ok_or(HtmlBrowserError::RuntimeNotStarted)?;
        interactive.refresh_frame()?;
        self.sync_interactive_state()
    }

    pub fn dispatch_input(&mut self, input: HtmlBrowserInput) -> Result<(), HtmlBrowserError> {
        self.ensure_active()?;
        input.validate()?;
        let interactive = self
            .interactive
            .as_mut()
            .ok_or(HtmlBrowserError::RuntimeNotStarted)?;
        interactive.dispatch_input(input)?;
        self.sync_interactive_state()
    }

    fn accept_frame(&mut self, frame: HtmlBrowserFrame) -> Result<(), HtmlBrowserError> {
        self.ensure_active()?;
        if frame.origin != self.source.origin {
            return Err(HtmlBrowserError::FrameOriginMismatch {
                expected: self.source.origin.as_str().to_owned(),
                actual: frame.origin.as_str().to_owned(),
            });
        }
        if let Some(latest) = &self.latest_frame
            && frame.generation <= latest.generation
        {
            return Err(HtmlBrowserError::StaleFrameGeneration {
                latest: latest.generation,
                received: frame.generation,
            });
        }
        self.latest_frame = Some(frame);
        self.frame_update_pending = true;
        Ok(())
    }

    fn ensure_active(&self) -> Result<(), HtmlBrowserError> {
        (self.state == HtmlBrowserSessionState::Active)
            .then_some(())
            .ok_or(HtmlBrowserError::SessionClosed)
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "session_requirements_tests.rs"]
mod requirements_tests;

#[cfg(test)]
#[path = "session_fragment_tests.rs"]
mod fragment_tests;

#[cfg(test)]
#[path = "session_state_tests.rs"]
mod state_tests;
