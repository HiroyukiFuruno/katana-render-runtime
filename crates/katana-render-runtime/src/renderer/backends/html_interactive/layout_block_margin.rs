use super::super::html_document::HtmlDocumentNode;
use super::style::{CssFloat, CssPosition, CssStyle};
use taffy::prelude::Display;

#[cfg(test)]
#[path = "layout_block_margin_tests.rs"]
mod tests;

pub(super) fn collapsed_margin(first: f32, second: f32) -> f32 {
    first.max(0.0).max(second.max(0.0)) + first.min(0.0).min(second.min(0.0))
}

pub(super) fn trailing_child_margin(children: &[HtmlDocumentNode], style: &CssStyle) -> f32 {
    if !allows_trailing_collapse(style) {
        return 0.0;
    }
    children
        .iter()
        .rev()
        .find_map(|child| child_trailing_margin(child, style))
        .unwrap_or(0.0)
}

pub(super) fn leading_child_margin(children: &[HtmlDocumentNode], style: &CssStyle) -> f32 {
    if !allows_leading_collapse(style) {
        return 0.0;
    }
    children
        .iter()
        .find_map(|child| child_leading_margin(child, style))
        .unwrap_or(0.0)
}

fn child_leading_margin(child: &HtmlDocumentNode, style: &CssStyle) -> Option<f32> {
    child_margin(child, style, |child_style, children| {
        collapsed_margin(
            child_style.margin_top,
            leading_child_margin(children, child_style),
        )
    })
}

fn child_margin(
    child: &HtmlDocumentNode,
    style: &CssStyle,
    collapse: impl FnOnce(&CssStyle, &[HtmlDocumentNode]) -> f32,
) -> Option<f32> {
    match child {
        HtmlDocumentNode::Text(text) if text.chars().all(super::layout_text::is_css_whitespace) => {
            None
        }
        HtmlDocumentNode::Text(_) => Some(0.0),
        HtmlDocumentNode::Element {
            tag,
            attributes,
            children,
            ..
        } => {
            let child_style = CssStyle::from_element(tag, attributes, style);
            if is_outside_flow(&child_style) {
                return None;
            }
            if child_style.inline_block {
                return Some(0.0);
            }
            Some(collapse(&child_style, children))
        }
    }
}

fn allows_leading_collapse(style: &CssStyle) -> bool {
    style.display == Display::Block
        && !style.inline_block
        && style.padding_top == 0.0
        && style.border_top_width() == 0.0
        && !style.clips_overflow()
        && style.float == CssFloat::None
        && !matches!(style.position, CssPosition::Absolute | CssPosition::Fixed)
}

fn child_trailing_margin(child: &HtmlDocumentNode, style: &CssStyle) -> Option<f32> {
    child_margin(child, style, |child_style, children| {
        collapsed_margin(
            child_style.margin_bottom,
            trailing_child_margin(children, child_style),
        )
    })
}

fn allows_trailing_collapse(style: &CssStyle) -> bool {
    style.display == Display::Block
        && !style.inline_block
        && style.height.is_none()
        && style.min_height == 0.0
        && style.padding_bottom == 0.0
        && style.border_bottom_width() == 0.0
        && !style.clips_overflow()
        && style.float == CssFloat::None
        && !matches!(style.position, CssPosition::Absolute | CssPosition::Fixed)
}

fn is_outside_flow(style: &CssStyle) -> bool {
    style.display == Display::None
        || style.float != CssFloat::None
        || matches!(style.position, CssPosition::Absolute | CssPosition::Fixed)
}
