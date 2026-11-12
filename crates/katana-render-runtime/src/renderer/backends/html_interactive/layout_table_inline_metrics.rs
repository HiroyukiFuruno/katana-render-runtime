use super::super::super::super::super::super::html_document::HtmlDocumentNode;
use super::super::super::super::super::document::css_auto_linebreak_ends;
use super::super::super::super::super::layout_flow_measure::intrinsic_text_width;
use super::super::super::super::super::style::CssStyle;
use super::TableIntrinsic;

pub(super) fn child_style(child: &HtmlDocumentNode, parent: &CssStyle) -> CssStyle {
    match child {
        HtmlDocumentNode::Element {
            tag, attributes, ..
        } => CssStyle::from_element(tag, attributes, parent),
        HtmlDocumentNode::Text(_) => parent.inherited_text_style(),
    }
}

pub(super) fn table_no_wrap_text_width(text: &str, style: &CssStyle) -> f32 {
    intrinsic_text_width(&HtmlDocumentNode::Text(text.to_string()), style)
}

pub(super) fn normal_children_min_content_width(
    children: &[HtmlDocumentNode],
    inherited: &CssStyle,
) -> f32 {
    let mut maximum = 0.0;
    let mut inline_run = 0.0;
    let mut previous_last = None;
    for child in children {
        let style = child_style(child, inherited);
        let width = TableIntrinsic::min_content_width(child, &style);
        if is_flow_inline(child, &style) {
            if width <= 0.0 && text_edge(child, true).is_none() {
                continue;
            }
            let metrics = inline_metrics(child, &style);
            (maximum, inline_run, previous_last) =
                append_flow_inline(child, metrics, maximum, inline_run, previous_last);
        } else if style.inline_block {
            maximum = maximum.max(inline_run);
            inline_run = inline_run.max(width);
            previous_last = None;
        } else {
            maximum = maximum.max(inline_run).max(width);
            inline_run = 0.0;
            previous_last = None;
        }
    }
    maximum.max(inline_run)
}

fn is_flow_inline(child: &HtmlDocumentNode, style: &CssStyle) -> bool {
    matches!(child, HtmlDocumentNode::Text(_)) || (style.inline_block && !style.inline_atomic)
}

fn append_flow_inline(
    child: &HtmlDocumentNode,
    metrics: InlineMetrics,
    mut maximum: f32,
    mut inline_run: f32,
    previous_last: Option<char>,
) -> (f32, f32, Option<char>) {
    let first = text_edge(child, true);
    if let (Some(last), Some(first)) = (previous_last, first)
        && !inline_boundary_allows_break(last, first)
    {
        inline_run += metrics.leading;
    } else {
        maximum = maximum.max(inline_run);
        inline_run = metrics.trailing;
    }
    (
        maximum.max(metrics.minimum),
        inline_run,
        text_edge(child, false),
    )
}

#[derive(Clone, Copy)]
struct InlineMetrics {
    minimum: f32,
    leading: f32,
    trailing: f32,
}

fn inline_metrics(node: &HtmlDocumentNode, style: &CssStyle) -> InlineMetrics {
    match node {
        HtmlDocumentNode::Text(text) => text_inline_metrics(text, style),
        HtmlDocumentNode::Element { children, .. } => {
            let minimum = normal_children_min_content_width(children, style);
            let leading = children
                .iter()
                .find(|child| text_edge(child, true).is_some())
                .map(|child| inline_metrics(child, &child_style(child, style)).leading)
                .unwrap_or(0.0);
            let trailing = children
                .iter()
                .rev()
                .find(|child| text_edge(child, false).is_some())
                .map(|child| inline_metrics(child, &child_style(child, style)).trailing)
                .unwrap_or(0.0);
            let outer = style.intrinsic_outer_width(0.0);
            InlineMetrics {
                minimum: style.intrinsic_outer_width(minimum),
                leading: leading + outer,
                trailing: trailing + outer,
            }
        }
    }
}

fn text_inline_metrics(text: &str, style: &CssStyle) -> InlineMetrics {
    let segments = css_auto_linebreak_ends(text)
        .into_iter()
        .scan(0, |start, end| {
            let segment =
                text[*start..end].trim_matches(|character: char| character.is_ascii_whitespace());
            *start = end;
            Some(intrinsic_text_width(
                &HtmlDocumentNode::Text(segment.to_string()),
                style,
            ))
        })
        .collect::<Vec<_>>();
    let minimum = segments.iter().copied().fold(0.0, f32::max);
    InlineMetrics {
        minimum,
        leading: segments.first().copied().unwrap_or(0.0),
        trailing: segments.last().copied().unwrap_or(0.0),
    }
}

fn text_edge(node: &HtmlDocumentNode, first: bool) -> Option<char> {
    match node {
        HtmlDocumentNode::Text(text) => {
            if first {
                text.chars().next()
            } else {
                text.chars().next_back()
            }
        }
        HtmlDocumentNode::Element { children, .. } => {
            if first {
                children
                    .iter()
                    .filter_map(|child| text_edge(child, true))
                    .next()
            } else {
                children
                    .iter()
                    .rev()
                    .filter_map(|child| text_edge(child, false))
                    .next()
            }
        }
    }
}

fn inline_boundary_allows_break(left: char, right: char) -> bool {
    if left.is_ascii_whitespace() || right.is_ascii_whitespace() {
        return true;
    }
    let text = format!("{left}{right}");
    css_auto_linebreak_ends(&text).contains(&left.len_utf8())
}

pub(super) fn table_min_content_text_width(text: &str, style: &CssStyle) -> f32 {
    let mut start = 0;
    let mut maximum = 0.0_f32;
    /* WHY: EOTのmandatory breakが必ず最終segmentを返すため、loop後の補完処理は不要。 */
    for end in css_auto_linebreak_ends(text) {
        let segment =
            text[start..end].trim_matches(|character: char| character.is_ascii_whitespace());
        maximum = maximum.max(intrinsic_text_width(
            &HtmlDocumentNode::Text(segment.to_string()),
            style,
        ));
        start = end;
    }
    maximum
}

#[cfg(test)]
mod tests {
    use super::{
        CssStyle, HtmlDocumentNode, normal_children_min_content_width, table_min_content_text_width,
    };
    use crate::renderer::backends::html_browser::HtmlBrowserViewport;
    use crate::renderer::backends::html_document::HtmlDocument;
    use crate::renderer::backends::html_interactive::layout::HtmlLayoutRenderer;
    use std::collections::HashMap;

    fn boundary_fixture() -> Vec<HtmlDocumentNode> {
        vec![
            HtmlDocumentNode::Element {
                node_id: 2,
                tag: "span".to_string(),
                attributes: Vec::new(),
                children: Vec::new(),
            },
            HtmlDocumentNode::Element {
                node_id: 3,
                tag: "span".to_string(),
                attributes: vec![("style".to_string(), "display:inline-block".to_string())],
                children: vec![HtmlDocumentNode::Text("inline".to_string())],
            },
            HtmlDocumentNode::Element {
                node_id: 4,
                tag: "div".to_string(),
                attributes: Vec::new(),
                children: vec![HtmlDocumentNode::Text("block".to_string())],
            },
        ]
    }

    #[test]
    fn normal_children_cover_empty_inline_block_and_block_boundaries() {
        let style = CssStyle::browser_default();
        assert!(normal_children_min_content_width(&boundary_fixture(), &style) > 0.0);
    }

    #[test]
    fn text_width_uses_the_last_segment_after_a_trailing_break() {
        let style = CssStyle::browser_default();
        let width = table_min_content_text_width("longword short", &style);
        assert!(width > 0.0);
    }

    #[test]
    fn text_width_handles_unicode_final_segment_and_empty_text() {
        let style = CssStyle::browser_default();
        assert!(table_min_content_text_width("資料一覧", &style) > 0.0);
        assert_eq!(table_min_content_text_width("", &style), 0.0);
    }

    #[test]
    fn public_table_trailing_text_has_rendered_geometry() {
        let document = HtmlDocument::parse(
            "<table style=\"width:106px\"><tr><td style=\"font-family:monospace;font-size:16px;line-height:20px;padding:5px 0;border:1px solid #123456;background:#fedcba\">alpha beta gamma</td></tr></table>",
        );
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());
        let rendered = HtmlBrowserViewport::new(320, 160, 1.0).map(|viewport| {
            HtmlLayoutRenderer::render(&nodes, viewport, 0.0, &HashMap::new(), None)
                .map(|layout| layout.svg)
        });
        assert!(
            matches!(&rendered, Ok(Ok(svg)) if svg
                .contains(r##"<rect x="10" y="10" width="102" height="52" fill="#fedcba"/>"##)
                && svg.contains("alpha") && svg.contains("gamma")),
            "{rendered:?}"
        );
    }
}
