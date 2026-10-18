use super::super::super::html_document::HtmlDocumentNode;
use super::super::document::attribute;
use super::super::style::{CssPosition, CssStyle};
use super::InlineFlowState;
use super::measure::InlineMeasurement;

pub(super) fn inline_flow_style(
    node: &HtmlDocumentNode,
    inherited: &CssStyle,
    clickable_nodes: &std::collections::HashSet<u64>,
) -> Option<CssStyle> {
    let HtmlDocumentNode::Element {
        node_id,
        tag,
        attributes,
        children,
    } = node
    else {
        return None;
    };
    if !is_fragmentable_phrasing_tag(tag)
        || clickable_nodes.contains(node_id)
        || attribute(attributes, "onclick").is_some()
    {
        return None;
    }
    let mut style = CssStyle::from_element(tag, attributes, inherited);
    if style.differs_in_inline_metrics(inherited) || style.has_inline_metric_change(children) {
        style.resolve_text_line_height(children);
    }
    (style.inline_block && !style.inline_atomic && unboxed_inline_style(&style)).then_some(style)
}

fn is_fragmentable_phrasing_tag(tag: &str) -> bool {
    matches!(
        tag,
        "abbr"
            | "b"
            | "bdi"
            | "bdo"
            | "cite"
            | "code"
            | "dfn"
            | "em"
            | "i"
            | "kbd"
            | "mark"
            | "q"
            | "s"
            | "samp"
            | "small"
            | "span"
            | "strong"
            | "sub"
            | "sup"
            | "time"
            | "u"
            | "var"
    )
}

fn unboxed_inline_style(style: &CssStyle) -> bool {
    style.background.is_none()
        && style.box_shadow.is_none()
        && style.border_width == 0.0
        && style.border_top_width.is_none()
        && style.border_right_width.is_none()
        && style.border_bottom_width.is_none()
        && style.border_left_width.is_none()
        && style.padding_top == 0.0
        && style.padding_right == 0.0
        && style.padding_bottom == 0.0
        && style.padding_left == 0.0
        && style.margin_top == 0.0
        && style.margin_right == 0.0
        && style.margin_bottom == 0.0
        && style.margin_left == 0.0
        && style.width.is_none()
        && style.min_width.is_none()
        && style.max_width.is_none()
        && style.height.is_none()
        && style.position == CssPosition::Static
        && style.opacity == 1.0
}

pub(super) fn advance_inline_text(
    inline: &mut InlineFlowState,
    text: &str,
    lines: &[String],
    remaining_width: f32,
    initial_y: f32,
    style: &CssStyle,
) {
    inline.bottom = inline
        .bottom
        .max(initial_y + inline.line_advance(lines.len(), style));
    if lines.len() == 1 {
        inline.cursor_x += InlineMeasurement::text_width(text, style).min(remaining_width);
    } else {
        inline.y = initial_y + inline.line_advance(lines.len() - 1, style);
        inline.cursor_x = inline.x
            + lines
                .last()
                .map(|line| InlineMeasurement::text_width(line, style))
                .unwrap_or(0.0);
        inline.advance_line_index(lines.len() - 1);
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::super::html_document::HtmlDocumentNode;
    use super::super::super::style::CssStyle;
    use super::inline_flow_style;
    use std::collections::HashSet;

    #[test]
    fn text_nodes_are_not_fragmentable_element_runs() {
        let node = HtmlDocumentNode::Text("text".to_string());

        assert!(inline_flow_style(&node, &CssStyle::browser_default(), &HashSet::new()).is_none());
    }
}
