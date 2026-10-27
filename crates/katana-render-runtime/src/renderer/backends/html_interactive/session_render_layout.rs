#[path = "session_render_layout_projection_metrics.rs"]
mod projection_metrics;

use super::super::html_document::HtmlDocumentNode;
use super::document::seed_input_values;
use super::layout::HtmlLayoutRenderer;
use super::session::HtmlInteractiveSession;
use super::text_measurement_diagnostic::{
    TextMeasurementDiagnosticScope, finish_layout_svg_trace, finish_layout_total_trace,
};
use super::types::LayoutResult;
use super::{HtmlBrowserError, runtime_failure};
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use projection_metrics::{projection_fingerprint, traced_node_count};
use std::collections::HashSet;

impl HtmlInteractiveSession {
    pub(super) fn traced_layout(
        &mut self,
        generation: u64,
    ) -> Result<LayoutResult, HtmlBrowserError> {
        let total_started = self.trace.start();
        let started = self.trace.start();
        let nodes = self.project_nodes()?;
        (self.tab_order, self.pointer_focusable_nodes) =
            super::input_focus::focus_candidates(&nodes);
        #[cfg(all(test, target_os = "macos"))]
        font_work::projection(&nodes, self, generation);
        self.invalidate_flow_measurements_if_projection_changed(&nodes);
        let node_count = traced_node_count(&self.trace, &nodes);
        self.trace.finish(
            generation,
            "dom_css_projection",
            started,
            &[("nodes", node_count)],
        );
        let clickable_nodes = self.clickable_nodes(generation)?;
        let rendered = self.render_layout_svg(generation, &nodes, &clickable_nodes)?;
        finish_layout_total_trace(
            &self.trace,
            generation,
            total_started,
            &rendered,
            (self.viewport.width, self.viewport.height),
        );
        Ok(rendered)
    }

    fn invalidate_flow_measurements_if_projection_changed(&mut self, nodes: &[HtmlDocumentNode]) {
        let fingerprint = projection_fingerprint(nodes);
        if self.flow_projection_fingerprint != Some(fingerprint) {
            self.flow_measurements.borrow_mut().clear();
            self.flow_projection_fingerprint = Some(fingerprint);
        }
    }

    fn render_layout_svg(
        &mut self,
        generation: u64,
        nodes: &[HtmlDocumentNode],
        clickable_nodes: &HashSet<u64>,
    ) -> Result<LayoutResult, HtmlBrowserError> {
        self.input_values.clear();
        seed_input_values(nodes, &mut self.input_values);
        let started = self.trace.start();
        let flow_before = self.flow_stats();
        let mut text_diagnostic = TextMeasurementDiagnosticScope::enter(self.trace.enabled());
        let mut svg_diagnostic =
            SvgRasterizeOps::begin_html_measurement_diagnostic(self.trace.enabled());
        let rendered = self.render_svg_content(nodes, clickable_nodes)?;
        let flow_after = self.flow_stats();
        let text_stats = text_diagnostic.finish();
        let svg_stats = svg_diagnostic.finish();
        finish_layout_svg_trace(
            &self.trace,
            generation,
            started,
            &rendered,
            flow_before,
            flow_after,
            (text_stats, svg_stats),
        );
        Ok(rendered)
    }

    fn flow_stats(&self) -> (usize, usize) {
        if self.trace.enabled() {
            self.flow_measurements.borrow().stats()
        } else {
            (0, 0)
        }
    }

    fn render_svg_content(
        &self,
        nodes: &[HtmlDocumentNode],
        clickable_nodes: &HashSet<u64>,
    ) -> Result<LayoutResult, HtmlBrowserError> {
        SvgRasterizeOps::with_html_font_render_scope(|| {
            HtmlLayoutRenderer::render_with_clickable_nodes_and_cache(
                nodes,
                self.viewport,
                (self.scroll_geometry.scroll_x, self.scroll_geometry.scroll_y),
                &self.input_values,
                self.focused_input,
                clickable_nodes,
                std::rc::Rc::clone(&self.flow_measurements),
            )
        })
        .map_err(runtime_failure)
    }

    pub(super) fn project_nodes(&self) -> Result<Vec<HtmlDocumentNode>, HtmlBrowserError> {
        let focused_nodes = self.focused_node.into_iter().collect::<HashSet<_>>();
        let focus_visible_nodes = self
            .focus_visible
            .then_some(self.focused_node)
            .flatten()
            .into_iter()
            .collect::<HashSet<_>>();
        self.runtime
            .interactive_nodes_at_width_with_focus(
                self.viewport.logical_width(),
                &self.hovered_nodes,
                &focused_nodes,
                &focus_visible_nodes,
            )
            .map_err(runtime_failure)
    }

    fn clickable_nodes(&self, generation: u64) -> Result<HashSet<u64>, HtmlBrowserError> {
        let started = self.trace.start();
        let nodes = self
            .runtime
            .event_target_ids("click")
            .map_err(runtime_failure)?;
        self.trace.finish(
            generation,
            "event_targets",
            started,
            &[("clickable_nodes", nodes.len())],
        );
        Ok(nodes)
    }

    pub(super) fn traced_rasterize(
        &self,
        generation: u64,
        svg: &str,
        generated_clip_provenance: &crate::markdown::svg_rasterize::GeneratedClipProvenance,
        generated_html_text_provenance: &crate::markdown::svg_rasterize::GeneratedHtmlTextProvenance,
    ) -> Result<Vec<u8>, HtmlBrowserError> {
        let started = self.trace.start();
        let raster = SvgRasterizeOps::rasterize_html_svg_with_text_provenance(
            svg,
            1.0,
            generated_clip_provenance,
            generated_html_text_provenance,
        )
        .map_err(|error| runtime_failure(error.to_string()))?;
        self.validate_raster_dimensions(raster.width, raster.height)?;
        self.trace.finish(
            generation,
            "svg_rasterize",
            started,
            &[("rgba_bytes", raster.rgba.len())],
        );
        Ok(raster.rgba)
    }

    #[cfg(test)]
    pub(super) fn rasterize(&self, svg: &str) -> Result<Vec<u8>, HtmlBrowserError> {
        self.traced_rasterize(
            self.generation + 1,
            svg,
            &crate::markdown::svg_rasterize::GeneratedClipProvenance::default(),
            &crate::markdown::svg_rasterize::GeneratedHtmlTextProvenance::default(),
        )
    }
}

#[cfg(all(test, target_os = "macos"))]
#[path = "session_render_font_work_observer.rs"]
mod font_work;

#[cfg(test)]
#[path = "session_render_layout_contract_tests.rs"]
mod contract_tests;
