use super::super::super::html_document::HtmlDocumentNode;
use super::super::document::wrap_text_with_initial_width;
use super::super::layout::HtmlLayoutRenderer;
use super::super::style::{CssPosition, CssStyle};
use super::super::types::DetailsContext;
use super::InlineMeasurement;
use super::floats::InlineFloat;
#[path = "layout_inline_margin.rs"]
mod margin;
#[path = "layout_inline_whitespace.rs"]
pub(super) mod whitespace;
use super::state::InlineFlowState;
use super::{advance_inline_text, inline_flow_style};
use whitespace::normalized_inline_text;

impl HtmlLayoutRenderer {
    fn render_inline_or_block_node(
        &mut self,
        nodes: &[HtmlDocumentNode],
        index: usize,
        inherited: &CssStyle,
        details: DetailsContext,
        inline: &mut InlineFlowState,
    ) {
        let node = &nodes[index];
        if matches!(node, HtmlDocumentNode::Text(text) if text.chars().all(super::super::layout_text::is_css_whitespace))
        {
            whitespace::remember_pending_inline_space(inline, inherited);
            return;
        }
        if matches!(node, HtmlDocumentNode::Element { tag, .. } if tag == "br") {
            self.render_inline_break(inherited.line_height, inline);
            return;
        }
        if self.render_inline_float(node, inherited, details, inline) {
            return;
        }
        if !inline.has_items {
            inline.cursor_x =
                InlineMeasurement::run_start_x(&nodes[index..], inline.x, inline.width, inherited);
        }
        self.render_inline_node_content(node, inherited, details, inline);
    }

    fn render_inline_node_content(
        &mut self,
        node: &HtmlDocumentNode,
        inherited: &CssStyle,
        details: DetailsContext,
        inline: &mut InlineFlowState,
    ) {
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

    fn render_inline_break(&mut self, line_height: f32, inline: &mut InlineFlowState) {
        self.record_inline_fragment(inline.cursor_x, inline.y, 0.0, line_height);
        inline.y = inline.bottom.max(inline.y + line_height);
        inline.cursor_x = inline.x;
        inline.bottom = inline.y;
        inline.has_items = false;
        inline.pending_space_width = None;
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
        let popped_owner = self.ownership.inline_fragment_owners.pop();
        debug_assert_eq!(popped_owner, Some(*node_id));
        self.finish_inline_fragment_box(element_box_index, *node_id, insertion_x, insertion_y);
    }

    fn render_inline_text(&mut self, text: &str, style: &CssStyle, inline: &mut InlineFlowState) {
        let normalized = normalized_inline_text(text, inline, style);
        if normalized.is_empty() {
            return;
        }
        let text = normalized.as_str();
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
        inline.pending_space_width = None;
        inline.y = self.render_node(node, inline.x, inline.y, inline.width, inherited, details);
    }
}
