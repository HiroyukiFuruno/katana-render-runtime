use super::{
    HtmlBrowserError, HtmlBrowserFrame, HtmlBrowserInput, HtmlBrowserNavigation,
    HtmlBrowserNavigationEvent, HtmlBrowserScrollMetrics, HtmlBrowserSource, HtmlBrowserViewport,
};
use crate::renderer::backends::html_interactive::HtmlInteractiveSession;
use std::time::Duration;

#[path = "session_types.rs"]
mod types;
pub use types::{HtmlBrowserSession, HtmlBrowserSessionState};

const MILLIS_PER_SECOND: f64 = 1_000.0;

#[path = "session_frame_commit.rs"]
mod frame_commit;
#[path = "session_shutdown.rs"]
mod shutdown;
#[path = "session_state.rs"]
mod state;
#[path = "session_sync.rs"]
mod sync;

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
            latest_scroll_metrics: None,
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

    pub fn navigate(&mut self, navigation: HtmlBrowserNavigation) -> Result<(), HtmlBrowserError> {
        self.ensure_active()?;
        navigation.source.validate()?;
        self.source = navigation.source;
        self.latest_frame = None;
        self.latest_scroll_metrics = None;
        self.frame_update_pending = false;
        self.pending_navigation = None;
        /* WHY: A replacement session must be the sole owner of its V8 isolate. */
        self.interactive = None;
        let result = (|| {
            self.interactive = Some(HtmlInteractiveSession::start(
                self.source.clone(),
                self.viewport,
            )?);
            self.sync_interactive_state()
        })();
        self.close_on_failure(result)
    }

    /// ビューポートを変更して、新しいフレーム状態を公開する。
    ///
    /// 有効なビューポートで描画または同期が失敗した場合、古いフレームと入力状態の
    /// 不一致を避けるためセッションを閉じる。無効なビューポートは変更せず、Activeを保つ。
    pub fn resize(&mut self, viewport: HtmlBrowserViewport) -> Result<(), HtmlBrowserError> {
        self.ensure_active()?;
        viewport.validate()?;
        self.resize_or_close_on_failure(viewport)
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

    /// Advances the deterministic HTML timer clock and publishes the resulting frame.
    ///
    /// The host owns wall-clock policy; this method never starts a background timer thread.
    pub fn advance_time(&mut self, elapsed: Duration) -> Result<(), HtmlBrowserError> {
        self.ensure_active()?;
        let interactive = self
            .interactive
            .as_mut()
            .ok_or(HtmlBrowserError::RuntimeNotStarted)?;
        interactive.advance_time(elapsed.as_secs_f64() * MILLIS_PER_SECOND)?;
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

    #[cfg(test)]
    fn accept_frame(&mut self, frame: HtmlBrowserFrame) -> Result<(), HtmlBrowserError> {
        Self::validate_frame(
            self.state,
            &self.source.origin,
            self.latest_frame.as_ref(),
            &frame,
        )?;
        self.latest_frame = Some(frame);
        self.latest_scroll_metrics = None;
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
mod shared_input_test_support {
    use super::{HtmlBrowserInput, HtmlBrowserSession};

    pub(super) fn dispatch_primary_click(
        session: &mut HtmlBrowserSession,
        coordinate: f32,
    ) -> Result<(), String> {
        for input in [
            HtmlBrowserInput::PointerDown {
                x: coordinate,
                y: coordinate,
                button: 0,
            },
            HtmlBrowserInput::PointerUp {
                x: coordinate,
                y: coordinate,
                button: 0,
            },
        ] {
            session
                .dispatch_input(input)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "session_scroll_metrics_tests.rs"]
mod scroll_metrics_tests;

#[cfg(test)]
#[path = "session_scroll_axis_clip_tests.rs"]
mod scroll_axis_clip_tests;

#[cfg(all(test, target_os = "macos"))]
#[path = "session_input_font_tests.rs"]
mod input_font_tests;

#[cfg(test)]
#[path = "session_timer_tests.rs"]
mod timer_tests;

#[cfg(test)]
#[path = "session_requirements_tests.rs"]
mod requirements_tests;

#[cfg(test)]
#[path = "session_fragment_tests.rs"]
mod fragment_tests;

#[cfg(test)]
#[path = "session_state_tests.rs"]
mod state_tests;

#[cfg(test)]
#[path = "session_table_layout_tests.rs"]
mod table_layout_tests;

#[cfg(test)]
#[path = "session_flex_cross_size_tests.rs"]
mod flex_cross_size_tests;

#[cfg(test)]
#[path = "session_svg_style_tests.rs"]
mod svg_style_tests;

#[cfg(test)]
#[path = "session_frame_clone_tests.rs"]
mod frame_clone_tests;

#[cfg(test)]
#[path = "session_frame_reuse_tests.rs"]
mod frame_reuse_tests;

#[cfg(test)]
#[path = "session_resize_failure_tests.rs"]
mod resize_failure_tests;
