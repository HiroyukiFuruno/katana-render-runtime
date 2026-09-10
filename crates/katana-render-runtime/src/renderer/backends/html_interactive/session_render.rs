use super::super::html_browser::{HtmlBrowserFrame, HtmlBrowserPixelFormat};
use super::super::html_document::HtmlDocumentNode;
use super::document::seed_input_values;
use super::layout::HtmlLayoutRenderer;
use super::session::HtmlInteractiveSession;
use super::types::LayoutResult;
use super::{HtmlBrowserError, runtime_failure};
use crate::markdown::svg_rasterize::SvgRasterizeOps;
use std::collections::HashSet;

impl HtmlInteractiveSession {
    pub(super) fn render_frame(&mut self) -> Result<(), HtmlBrowserError> {
        let generation = self.generation + 1;
        let total_started = self.trace.start();
        let mut render = self.traced_layout(generation)?;
        for _ in 0..2 {
            self.update_scroll(&render);
            if !self.sync_intersection_geometry(&render)? {
                break;
            }
            render = self.traced_layout(generation)?;
        }
        let pixels = self.traced_rasterize(generation, &render.svg)?;
        let started = self.trace.start();
        let result = self.store_frame(render, pixels);
        self.trace.finish(generation, "frame_store", started, &[]);
        self.trace
            .finish(generation, "frame_total", total_started, &[]);
        result
    }

    pub(super) fn layout(&mut self) -> Result<LayoutResult, HtmlBrowserError> {
        self.traced_layout(self.generation + 1)
    }

    fn traced_layout(&mut self, generation: u64) -> Result<LayoutResult, HtmlBrowserError> {
        let total_started = self.trace.start();
        let started = self.trace.start();
        let nodes = self.project_nodes()?;
        let node_count = traced_node_count(&self.trace, &nodes);
        self.trace.finish(
            generation,
            "dom_css_projection",
            started,
            &[("nodes", node_count)],
        );
        let clickable_nodes = self.clickable_nodes(generation)?;
        let rendered = self.render_layout_svg(generation, &nodes, &clickable_nodes)?;
        self.trace_layout_total(generation, total_started, &rendered);
        Ok(rendered)
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
        let rendered = HtmlLayoutRenderer::render_with_clickable_nodes(
            nodes,
            self.viewport,
            self.scroll_y,
            &self.input_values,
            self.focused_input,
            clickable_nodes,
        )
        .map_err(runtime_failure)?;
        self.trace.finish(
            generation,
            "layout_svg",
            started,
            &[("svg_bytes", rendered.svg.len())],
        );
        Ok(rendered)
    }

    fn project_nodes(&self) -> Result<Vec<HtmlDocumentNode>, HtmlBrowserError> {
        self.runtime
            .interactive_nodes_at_width_with_hover(
                self.viewport.logical_width(),
                &self.hovered_nodes,
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

    fn trace_layout_total(
        &self,
        generation: u64,
        started: Option<std::time::Instant>,
        render: &LayoutResult,
    ) {
        self.trace.finish(
            generation,
            "layout_total",
            started,
            &[
                ("svg_bytes", render.svg.len()),
                ("hit_targets", render.hit_targets.len()),
                ("element_boxes", render.element_boxes.len()),
                ("viewport_width", self.viewport.width as usize),
                ("viewport_height", self.viewport.height as usize),
            ],
        );
    }

    fn traced_rasterize(&self, generation: u64, svg: &str) -> Result<Vec<u8>, HtmlBrowserError> {
        let started = self.trace.start();
        let raster = SvgRasterizeOps::rasterize_html_svg(svg, 1.0)
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
        self.traced_rasterize(self.generation + 1, svg)
    }

    pub(super) fn validate_raster_dimensions(
        &self,
        width: u32,
        height: u32,
    ) -> Result<(), HtmlBrowserError> {
        if width == self.viewport.width && height == self.viewport.height {
            return Ok(());
        }
        Err(runtime_failure(format!(
            "interactive raster dimensions are {width}x{height}, expected {}x{}",
            self.viewport.width, self.viewport.height
        )))
    }

    pub(super) fn store_frame(
        &mut self,
        render: LayoutResult,
        pixels: Vec<u8>,
    ) -> Result<(), HtmlBrowserError> {
        self.generation += 1;
        self.latest_frame = Some(
            HtmlBrowserFrame::new(
                self.generation,
                self.source.origin.clone(),
                self.viewport,
                HtmlBrowserPixelFormat::Rgba8,
                pixels,
            )
            .map_err(|error| runtime_failure(error.to_string()))?
            .with_layout_metrics(self.scroll_y, render.content_height),
        );
        self.hit_targets = render.hit_targets;
        self.element_boxes = render.element_boxes;
        self.content_height = render.content_height;
        Ok(())
    }
}

fn node_count(nodes: &[HtmlDocumentNode]) -> usize {
    nodes
        .iter()
        .map(|node| match node {
            HtmlDocumentNode::Element { children, .. } => 1 + node_count(children),
            HtmlDocumentNode::Text(_) => 1,
        })
        .sum()
}

fn traced_node_count(
    trace: &super::super::html_debug_trace::HtmlDebugTrace,
    nodes: &[HtmlDocumentNode],
) -> usize {
    if trace.enabled() {
        return node_count(nodes);
    }
    0
}

#[cfg(test)]
mod tests {
    use super::{HtmlDocumentNode, traced_node_count};
    use crate::renderer::backends::html_debug_trace::HtmlDebugTrace;

    #[test]
    fn debug_node_metric_counts_elements_and_text_recursively() {
        let nodes = vec![HtmlDocumentNode::Element {
            node_id: 1,
            tag: "p".to_string(),
            attributes: Vec::new(),
            children: vec![HtmlDocumentNode::Text("visible".to_string())],
        }];

        assert_eq!(
            traced_node_count(&HtmlDebugTrace::enabled_for_test(), &nodes),
            2
        );
        assert_eq!(traced_node_count(&HtmlDebugTrace::disabled(), &nodes), 0);
    }
}
