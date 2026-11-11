use super::super::super::html_document::HtmlDocumentNode;
use super::super::document::wrap_text_with_initial_width;
use super::super::style::CssStyle;
use super::line_metrics::InlineLineMetrics;
use super::render::whitespace::normalized_inline_text;
use super::{InlineFlowState, InlineMeasurement};
use std::collections::HashSet;

#[cfg(test)]
#[path = "layout_inline_line_plan_tests.rs"]
mod tests;

pub(super) struct InlineLinePlan {
    metrics: Vec<InlineLineMetrics>,
    offsets: Vec<f32>,
}

impl InlineLinePlan {
    pub(super) fn for_run(
        nodes: &[HtmlDocumentNode],
        inherited: &CssStyle,
        clickable: &HashSet<u64>,
        width: f32,
    ) -> Self {
        let mut builder = LinePlanBuilder {
            metrics: vec![InlineLineMetrics::default()],
            flow: InlineFlowState::new(0.0, 0.0, width),
            inherited: inherited.clone(),
        };
        builder.add_nodes(nodes, inherited, clickable);
        let mut offsets = vec![0.0];
        for metrics in &builder.metrics {
            offsets.push(
                offsets.last().copied().unwrap_or(0.0)
                    + metrics.height().unwrap_or(inherited.line_height),
            );
        }
        Self {
            metrics: builder.metrics,
            offsets,
        }
    }

    pub(super) fn metrics(&self, index: usize) -> Option<&InlineLineMetrics> {
        self.metrics.get(index)
    }

    pub(super) fn advance(&self, index: usize, count: usize) -> Option<f32> {
        Some(self.offsets.get(index + count)? - self.offsets.get(index)?)
    }
}

struct LinePlanBuilder {
    metrics: Vec<InlineLineMetrics>,
    flow: InlineFlowState,
    inherited: CssStyle,
}

impl LinePlanBuilder {
    fn add_nodes(
        &mut self,
        nodes: &[HtmlDocumentNode],
        inherited: &CssStyle,
        clickable: &HashSet<u64>,
    ) {
        for node in nodes {
            if !self.add_node(node, inherited, clickable) {
                break;
            }
        }
    }

    fn add_node(
        &mut self,
        node: &HtmlDocumentNode,
        inherited: &CssStyle,
        clickable: &HashSet<u64>,
    ) -> bool {
        if let HtmlDocumentNode::Text(text) = node {
            self.add_text(text, inherited);
            return true;
        }
        let Some(fragment) = super::inline_flow_fragment(node, inherited, clickable) else {
            return self.add_atomic(node, inherited);
        };
        self.add_nodes(fragment.children, &fragment.style, clickable);
        true
    }

    fn add_atomic(&mut self, node: &HtmlDocumentNode, inherited: &CssStyle) -> bool {
        super::render::whitespace::advance_pending_inline_space(&mut self.flow, inherited);
        let Some(atomic) =
            super::atomic_metrics::inline_flex_metrics(node, inherited, self.flow.width)
        else {
            return false;
        };
        if self.flow.has_items && self.flow.cursor_x + atomic.width > self.flow.width {
            self.metrics.push(InlineLineMetrics::default());
            self.flow.cursor_x = 0.0;
        }
        if let Some(metrics) = self.metrics.last_mut() {
            metrics.add_text("strut", &self.inherited);
            metrics.add_box(atomic.baseline, atomic.height);
        }
        self.flow.cursor_x += atomic.width;
        self.flow.has_items = true;
        true
    }

    fn add_text(&mut self, text: &str, style: &CssStyle) {
        let text = normalized_inline_text(text, &mut self.flow, style);
        if text.is_empty() {
            return;
        }
        let remaining =
            (self.flow.width - self.flow.cursor_x).max(super::super::constants::MIN_LAYOUT_WIDTH);
        let lines = wrap_text_with_initial_width(&text, remaining, self.flow.width, style);
        for (index, line) in lines.iter().enumerate() {
            if index != 0 {
                self.metrics.push(InlineLineMetrics::default());
            }
            if let Some(metrics) = self.metrics.last_mut() {
                if style.font_size < self.inherited.font_size {
                    metrics.add_text(line, &self.inherited);
                }
                metrics.add_text(line, style);
            }
        }
        self.advance_text(&text, &lines, remaining, style);
    }

    fn advance_text(&mut self, text: &str, lines: &[String], remaining: f32, style: &CssStyle) {
        if lines.len() == 1 {
            self.flow.cursor_x += InlineMeasurement::text_width(text, style).min(remaining);
        } else if let Some(line) = lines.last() {
            self.flow.cursor_x = InlineMeasurement::text_width(line, style);
        }
        self.flow.has_items = true;
    }
}

#[cfg(test)]
#[path = "layout_inline_line_plan_parsed_dom_regression_tests.rs"]
mod parsed_dom_regression_tests;
