use super::{CssStyle, HtmlDocumentNode, child_style, collapsed_anonymous_text, measured_text_width};
use crate::renderer::backends::html_interactive::document::css_auto_linebreak_ends;
use crate::renderer::backends::html_interactive::layout_text::is_css_whitespace;
use crate::renderer::backends::html_interactive::style::{CssBoxSizing, CssLength, CssWhiteSpace};
use taffy::prelude::Display;

impl CssStyle {
    pub(in crate::renderer::backends::html_interactive) fn resolve_min_content_width(
        &mut self,
        children: &[HtmlDocumentNode],
    ) {
        if self.width_min_content {
            let content = children_min_content_width(children, self);
            self.width = Some(CssLength::Px(match self.box_sizing {
                CssBoxSizing::ContentBox => content,
                CssBoxSizing::BorderBox => self.intrinsic_outer_width(content),
            }));
        }
    }
}

fn min_content_node_width(node: &HtmlDocumentNode, style: &CssStyle) -> f32 {
    match node {
        HtmlDocumentNode::Text(text) => longest_unbreakable_width(text, style),
        HtmlDocumentNode::Element { children, .. } => {
            style.intrinsic_outer_width(children_min_content_width(children, style))
        }
    }
}

fn children_min_content_width(children: &[HtmlDocumentNode], style: &CssStyle) -> f32 {
    children
        .iter()
        .map(|child| (child, child_style(child, style)))
        .filter(|(_, child)| child.display != Display::None)
        .map(|(node, child)| min_content_node_width(node, &child))
        .fold(0.0, f32::max)
}

fn longest_unbreakable_width(text: &str, style: &CssStyle) -> f32 {
    let text = collapsed_anonymous_text(text);
    if style.white_space == CssWhiteSpace::NoWrap {
        return measured_text_width(&text, style);
    }
    let mut start = 0;
    let mut maximum = 0.0_f32;
    for end in css_auto_linebreak_ends(&text) {
        let segment = text[start..end].trim_end_matches(is_css_whitespace);
        maximum = maximum.max(measured_text_width(segment, style));
        start = end;
    }
    maximum
}

#[cfg(test)]
#[path = "layout_min_content_tests.rs"]
mod tests;
