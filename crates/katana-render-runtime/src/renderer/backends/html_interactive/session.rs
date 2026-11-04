use super::super::html_browser::{
    HtmlBrowserFrame, HtmlBrowserNavigationEvent, HtmlBrowserScrollMetrics, HtmlBrowserSource,
    HtmlBrowserViewport,
};
use super::super::html_debug_trace::HtmlDebugTrace;
use super::super::html_runtime::{StaticHtmlRuntime, StaticHtmlRuntimeSession};
use super::super::html_subresources::HtmlSubresourcePolicy;
use super::layout_measurement_cache::FlowMeasurementCache;
use super::session_scroll_geometry::HtmlInteractiveScrollGeometry;
use super::types::{ElementBox, HitTarget};
use super::{HtmlBrowserError, runtime_failure};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    ops::{Deref, DerefMut},
    rc::Rc,
};

/// In-process Rust/V8 HTML session. It owns the DOM, layout, hit-test and
/// raster frame so downstream crates only exchange frames and input events.
pub(in crate::renderer::backends) struct HtmlInteractiveSession {
    pub(super) source: HtmlBrowserSource,
    pub(super) viewport: HtmlBrowserViewport,
    pub(super) runtime: StaticHtmlRuntimeSession,
    pub(super) state: HtmlInteractiveSessionState,
    pub(super) resource_policy: HtmlSubresourcePolicy,
    pub(super) trace: HtmlDebugTrace,
    pub(super) flow_measurements: Rc<RefCell<FlowMeasurementCache>>,
    pub(super) flow_projection_fingerprint: Option<u64>,
}

#[derive(Default)]
pub(in crate::renderer::backends) struct HtmlInteractiveSessionState {
    pub(super) generation: u64,
    pub(super) latest_frame: Option<HtmlBrowserFrame>,
    pub(super) latest_scroll_metrics: Option<HtmlBrowserScrollMetrics>,
    pub(super) hit_targets: Vec<HitTarget>,
    pub(super) element_boxes: Vec<ElementBox>,
    pub(super) hovered_nodes: HashSet<u64>,
    pub(super) input_values: HashMap<u64, String>,
    pub(super) pressed_target: Option<u64>,
    pub(super) focused_input: Option<u64>,
    pub(super) focused_node: Option<u64>,
    pub(super) tab_order: Vec<u64>,
    pub(super) pointer_focusable_nodes: HashSet<u64>,
    pub(super) keyboard_modality: bool,
    pub(super) shift_pressed: bool,
    pub(super) focus_visible: bool,
    pub(super) dirty_inputs: HashSet<u64>,
    pub(super) scroll_geometry: HtmlInteractiveScrollGeometry,
    pub(super) resize_anchor: Option<String>,
    pub(super) pending_navigation: Option<HtmlBrowserNavigationEvent>,
}

impl Deref for HtmlInteractiveSession {
    type Target = HtmlInteractiveSessionState;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl DerefMut for HtmlInteractiveSession {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}

impl HtmlInteractiveSession {
    pub(in crate::renderer::backends) fn start(
        source: HtmlBrowserSource,
        viewport: HtmlBrowserViewport,
    ) -> Result<Self, HtmlBrowserError> {
        source.validate()?;
        viewport.validate()?;
        let trace = HtmlDebugTrace::from_env();
        let runtime = StaticHtmlRuntime
            .start_interactive_traced(&source, &trace)
            .map_err(runtime_failure)?;
        let mut session = Self::new(source, viewport, runtime, trace);
        session.render_frame()?;
        if session.source.origin.url().fragment().is_some() {
            let origin = session.source.origin.clone();
            session.apply_fragment_navigation(origin)?;
            session.render_frame()?;
        }
        Ok(session)
    }

    fn new(
        source: HtmlBrowserSource,
        viewport: HtmlBrowserViewport,
        runtime: StaticHtmlRuntimeSession,
        trace: HtmlDebugTrace,
    ) -> Self {
        let resource_policy = HtmlSubresourcePolicy::from_source(&source);
        Self {
            source,
            viewport,
            runtime,
            state: HtmlInteractiveSessionState::default(),
            resource_policy,
            trace,
            flow_measurements: Rc::new(RefCell::new(FlowMeasurementCache::default())),
            flow_projection_fingerprint: None,
        }
    }

    pub(in crate::renderer::backends) fn latest_frame(&self) -> Option<&HtmlBrowserFrame> {
        self.latest_frame.as_ref()
    }

    pub(in crate::renderer::backends) fn take_frame(&mut self) -> Option<HtmlBrowserFrame> {
        self.latest_frame.take()
    }

    pub(in crate::renderer::backends) fn release_frame_if_generation(&mut self, generation: u64) {
        if self
            .latest_frame
            .as_ref()
            .is_some_and(|frame| frame.generation == generation)
        {
            self.latest_frame = None;
        }
    }

    pub(in crate::renderer::backends) fn latest_scroll_metrics(
        &self,
    ) -> Option<&HtmlBrowserScrollMetrics> {
        self.latest_scroll_metrics.as_ref()
    }

    pub(in crate::renderer::backends) fn take_navigation(
        &mut self,
    ) -> Option<HtmlBrowserNavigationEvent> {
        self.pending_navigation.take()
    }

    pub(in crate::renderer::backends) fn refresh_frame(&mut self) -> Result<(), HtmlBrowserError> {
        self.render_frame()
    }

    pub(in crate::renderer::backends) fn advance_time(
        &mut self,
        elapsed_ms: f64,
    ) -> Result<(), HtmlBrowserError> {
        if self
            .runtime
            .advance_timers(elapsed_ms)
            .map_err(runtime_failure)?
        {
            self.render_frame()?;
        }
        Ok(())
    }

    pub(in crate::renderer::backends) fn resize(
        &mut self,
        viewport: HtmlBrowserViewport,
    ) -> Result<(), HtmlBrowserError> {
        viewport.validate()?;
        self.viewport = viewport;
        self.flow_projection_fingerprint = None;
        self.render_resize_frame()
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "session_resource_reuse_tests.rs"]
mod resource_reuse_tests;
