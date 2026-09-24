use super::super::super::html_document::HtmlDocumentNode;
use super::super::constants::LAYOUT_FLOAT_EPSILON;
use super::super::document::wrap_text_with_initial_width;
use super::super::layout::HtmlLayoutRenderer;
use super::super::style::{CssPosition, CssStyle};
use super::super::types::DetailsContext;
use super::InlineMeasurement;
use super::floats::InlineFloat;
use super::state::InlineFlowState;
use super::{advance_inline_text, inline_flow_style};

impl HtmlLayoutRenderer {
    pub(crate) fn render_nodes(
        &mut self,
        nodes: &[HtmlDocumentNode],
        x: f32,
        y: f32,
        width: f32,
        inherited: &CssStyle,
        details: DetailsContext,
    ) -> f32 {
        let mut inline = InlineFlowState::new(x, y, width);
        for index in 0..nodes.len() {
            self.render_inline_or_block_node(nodes, index, inherited, details, &mut inline);
        }
        inline.bottom()
    }

    fn render_inline_or_block_node(
        &mut self,
        nodes: &[HtmlDocumentNode],
        index: usize,
        inherited: &CssStyle,
        details: DetailsContext,
        inline: &mut InlineFlowState,
    ) {
        let node = &nodes[index];
        if matches!(node, HtmlDocumentNode::Text(text) if text.trim().is_empty()) {
            return;
        }
        if self.render_inline_float(node, inherited, details, inline) {
            return;
        }
        if !inline.has_items {
            inline.cursor_x =
                InlineMeasurement::run_start_x(&nodes[index..], inline.x, inline.width, inherited);
        }
        if let HtmlDocumentNode::Text(text) = node {
            self.render_inline_text(text, inherited, inline);
        } else if let Some(style) = inline_flow_style(node, inherited, &self.clickable_nodes) {
            self.render_inline_flow_children(node, &style, details, inline);
        } else if let Some(width) = InlineMeasurement::node_width(node, inherited, inline.width) {
            self.render_inline_node(node, width, inherited, details, inline);
        } else {
            self.render_block_node(node, inherited, details, inline);
        }
    }

    fn render_inline_float(
        &mut self,
        node: &HtmlDocumentNode,
        inherited: &CssStyle,
        details: DetailsContext,
        inline: &InlineFlowState,
    ) -> bool {
        let Some((side, float_width)) = InlineFloat::node_geometry(node, inherited, inline.width)
        else {
            return false;
        };
        self.render_floated_node(node, side, float_width, inherited, details, inline);
        true
    }

    pub(super) fn render_inline_flow_children(
        &mut self,
        node: &HtmlDocumentNode,
        style: &CssStyle,
        details: DetailsContext,
        inline: &mut InlineFlowState,
    ) {
        let HtmlDocumentNode::Element {
            node_id, children, ..
        } = node
        else {
            return;
        };
        let element_box_index = self.start_element_box(
            *node_id,
            self.in_flow_positioning_context(),
            true,
            self.positioning_origin_node_id(*node_id, CssPosition::Static),
        );
        let (insertion_x, insertion_y) = (inline.cursor_x, inline.y);
        self.ownership.inline_fragment_owners.push(*node_id);
        for index in 0..children.len() {
            self.render_inline_or_block_node(children, index, style, details, inline);
        }
        debug_assert_eq!(self.ownership.inline_fragment_owners.pop(), Some(*node_id));
        self.finish_inline_fragment_box(element_box_index, *node_id, insertion_x, insertion_y);
    }

    pub(super) fn render_inline_node(
        &mut self,
        node: &HtmlDocumentNode,
        inline_width: f32,
        inherited: &CssStyle,
        details: DetailsContext,
        inline: &mut InlineFlowState,
    ) {
        if inline.has_items
            && inline.cursor_x + inline_width > inline.x + inline.width + LAYOUT_FLOAT_EPSILON
        {
            inline.y = inline.bottom;
            inline.cursor_x = inline.x;
            inline.bottom = inline.y;
        }
        let element_box_start = self.element_boxes.len();
        inline.bottom = inline.bottom.max(self.render_node(
            node,
            inline.cursor_x,
            inline.y,
            inline_width,
            inherited,
            details,
        ));
        self.record_inline_atomic_fragment(element_box_start);
        inline.cursor_x += inline_width;
        inline.has_items = true;
    }

    fn record_inline_atomic_fragment(&mut self, element_box_start: usize) {
        let Some(element_box) = self.element_boxes.get(element_box_start) else {
            return;
        };
        if !element_box.participates_in_flow {
            return;
        }
        if !element_box.inline_fragments.is_empty() {
            return;
        }
        let (x, y, width, height) = (
            element_box.x,
            element_box.y,
            element_box.width,
            element_box.height,
        );
        self.record_inline_fragment(x, y, width, height);
    }

    fn render_inline_text(&mut self, text: &str, style: &CssStyle, inline: &mut InlineFlowState) {
        let initial_x = inline.cursor_x;
        let initial_y = inline.y;
        let remaining_width =
            (inline.x + inline.width - initial_x).max(super::super::constants::MIN_LAYOUT_WIDTH);
        let lines = wrap_text_with_initial_width(text, remaining_width, inline.width, style);
        self.paint_inline_lines(&lines, initial_x, initial_y, style, inline);
        advance_inline_text(inline, text, &lines, remaining_width, initial_y, style);
        inline.has_items = true;
    }

    pub(super) fn render_block_node(
        &mut self,
        node: &HtmlDocumentNode,
        inherited: &CssStyle,
        details: DetailsContext,
        inline: &mut InlineFlowState,
    ) {
        if inline.has_items {
            inline.y = inline.bottom;
            inline.cursor_x = inline.x;
            inline.has_items = false;
        }
        inline.y = self.render_node(node, inline.x, inline.y, inline.width, inherited, details);
    }
}
