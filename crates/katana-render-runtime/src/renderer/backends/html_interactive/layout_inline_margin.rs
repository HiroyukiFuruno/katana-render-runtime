use super::super::super::super::html_document::HtmlDocumentNode;
use super::super::super::layout::HtmlLayoutRenderer;
use super::super::super::style::{CssFloat, CssPosition, CssStyle};
use super::super::super::types::DetailsContext;
use super::super::state::InlineFlowState;
use taffy::prelude::Display;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BlockMargins {
    pub(super) top: f32,
    pub(super) bottom: f32,
}

struct MarginRenderContext<'a> {
    nodes: &'a [HtmlDocumentNode],
    index: usize,
    inherited: &'a CssStyle,
    details: DetailsContext,
    inline: &'a mut InlineFlowState,
    previous_block_bottom_margin: &'a mut f32,
    previous_block_collapsible: &'a mut bool,
}

pub(super) fn block_margins(node: &HtmlDocumentNode, inherited: &CssStyle) -> Option<BlockMargins> {
    let HtmlDocumentNode::Element {
        tag,
        attributes,
        children,
        ..
    } = node
    else {
        return None;
    };
    let style = CssStyle::from_element(tag, attributes, inherited);
    block_margins_for_style(children, &style)
}

fn block_margins_for_style(
    children: &[HtmlDocumentNode],
    style: &CssStyle,
) -> Option<BlockMargins> {
    (matches!(
        style.display,
        Display::Block | Display::FlowRoot | Display::Flex | Display::Grid
    ) && !style.inline_block
        && style.position != CssPosition::Absolute
        && style.position != CssPosition::Fixed
        && style.float == CssFloat::None)
        .then_some(BlockMargins {
            top: super::super::super::layout_block_margin::collapsed_margin(
                style.margin_top,
                super::super::super::layout_block_margin::leading_child_margin(children, style),
            ),
            bottom: super::super::super::layout_block_margin::collapsed_margin(
                style.margin_bottom,
                super::super::super::layout_block_margin::trailing_child_margin(children, style),
            ),
        })
}

pub(super) fn collapsed_margin_overlap(previous_bottom: f32, current_top: f32) -> f32 {
    previous_bottom + current_top - collapsed_margin(previous_bottom, current_top)
}

pub(super) fn parent_allows_collapse(style: &CssStyle) -> bool {
    matches!(style.display, Display::Block | Display::FlowRoot)
}

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
        let mut previous_block_bottom_margin = 0.0;
        let mut previous_block_collapsible = false;
        for index in 0..nodes.len() {
            render_margin_node(
                self,
                &mut MarginRenderContext {
                    nodes,
                    index,
                    inherited,
                    details,
                    inline: &mut inline,
                    previous_block_bottom_margin: &mut previous_block_bottom_margin,
                    previous_block_collapsible: &mut previous_block_collapsible,
                },
            );
        }
        inline.bottom()
    }
}

fn render_margin_node(renderer: &mut HtmlLayoutRenderer, context: &mut MarginRenderContext<'_>) {
    prepare_inline_line(renderer, context);
    let had_inline_content = context.inline.has_items;
    adjust_margin_start(
        context.nodes,
        context.index,
        context.inherited,
        had_inline_content,
        *context.previous_block_collapsible,
        *context.previous_block_bottom_margin,
        context.inline,
    );
    renderer.render_inline_or_block_node(
        context.nodes,
        context.index,
        context.inherited,
        context.details,
        context.inline,
    );
    remember_margin_state(
        context.nodes,
        context.index,
        context.inherited,
        had_inline_content,
        context.previous_block_bottom_margin,
        context.previous_block_collapsible,
    );
}

fn prepare_inline_line(renderer: &HtmlLayoutRenderer, context: &mut MarginRenderContext<'_>) {
    if !context.inline.has_items {
        context.inline.resolve_line_metrics(
            &context.nodes[context.index..],
            context.inherited,
            &renderer.clickable_nodes,
        );
    }
}

fn adjust_margin_start(
    nodes: &[HtmlDocumentNode],
    index: usize,
    inherited: &CssStyle,
    had_inline_content: bool,
    previous_block_collapsible: bool,
    previous_block_bottom_margin: f32,
    inline: &mut InlineFlowState,
) {
    if !had_inline_content
        && previous_block_collapsible
        && parent_allows_collapse(inherited)
        && let Some(current) = block_margins(&nodes[index], inherited)
    {
        inline.y -= collapsed_margin_overlap(previous_block_bottom_margin, current.top);
    }
}

fn remember_margin_state(
    nodes: &[HtmlDocumentNode],
    index: usize,
    inherited: &CssStyle,
    had_inline_content: bool,
    previous_block_bottom_margin: &mut f32,
    previous_block_collapsible: &mut bool,
) {
    if matches!(&nodes[index], HtmlDocumentNode::Text(text) if text.chars().all(super::super::super::layout_text::is_css_whitespace))
    {
        return;
    }
    if let HtmlDocumentNode::Element {
        tag,
        attributes,
        children,
        ..
    } = &nodes[index]
    {
        let style = CssStyle::from_element(tag, attributes, inherited);
        if style.display == Display::None {
            return;
        }
        if !had_inline_content && let Some(current) = block_margins_for_style(children, &style) {
            (*previous_block_bottom_margin, *previous_block_collapsible) = (current.bottom, true);
            return;
        }
    }
    (*previous_block_bottom_margin, *previous_block_collapsible) = (0.0, false);
}

fn collapsed_margin(previous_bottom: f32, current_top: f32) -> f32 {
    if previous_bottom >= 0.0 && current_top >= 0.0 {
        previous_bottom.max(current_top)
    } else if previous_bottom <= 0.0 && current_top <= 0.0 {
        previous_bottom.min(current_top)
    } else {
        previous_bottom + current_top
    }
}

#[cfg(test)]
#[path = "layout_inline_margin_tests.rs"]
mod tests;
