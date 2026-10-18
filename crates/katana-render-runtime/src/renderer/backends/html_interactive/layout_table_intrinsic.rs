use super::super::super::super::super::html_document::HtmlDocumentNode;
use super::super::super::super::style::{CssStyle, CssWhiteSpace};

#[path = "layout_table_inline_metrics.rs"]
mod inline_metrics;
use inline_metrics::{
    child_style, normal_children_min_content_width, table_min_content_text_width,
    table_no_wrap_text_width,
};

#[cfg(test)]
#[path = "layout_table_intrinsic_coverage_gap_tests.rs"]
mod coverage_gaps;

pub(super) struct TableIntrinsic;

impl TableIntrinsic {
    pub(super) fn min_content_width(node: &HtmlDocumentNode, style: &CssStyle) -> f32 {
        if style.white_space == CssWhiteSpace::NoWrap {
            return match node {
                HtmlDocumentNode::Text(text) => table_no_wrap_text_width(text, style),
                HtmlDocumentNode::Element { children, .. } => {
                    let width = children
                        .iter()
                        .map(|child| Self::min_content_width(child, &child_style(child, style)))
                        .sum();
                    style.intrinsic_outer_width(width)
                }
            };
        }
        match node {
            HtmlDocumentNode::Text(text) => table_min_content_text_width(text, style),
            HtmlDocumentNode::Element { children, .. } => {
                let width = normal_children_min_content_width(children, style);
                style.intrinsic_outer_width(width)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HtmlDocumentNode, TableIntrinsic, table_no_wrap_text_width};
    use crate::renderer::backends::html_document::HtmlDocument;
    use crate::renderer::backends::html_interactive::style::CssStyle;
    use std::collections::HashMap;

    #[test]
    fn nowrap_text_is_shaped_as_one_inline_run() {
        let mut style = CssStyle::browser_default();
        style.white_space = super::CssWhiteSpace::NoWrap;
        let text = "確認中の事項を一覧に表示する。";
        let expected = table_no_wrap_text_width(text, &style);
        assert_eq!(table_no_wrap_text_width(text, &style), expected);
        assert_eq!(
            TableIntrinsic::min_content_width(&HtmlDocumentNode::Text(text.to_string()), &style),
            expected
        );
    }

    fn inline_children_fixture() -> HtmlDocumentNode {
        HtmlDocumentNode::Element {
            node_id: 1,
            tag: "td".to_string(),
            attributes: Vec::new(),
            children: vec![
                HtmlDocumentNode::Text("prefix ".to_string()),
                HtmlDocumentNode::Element {
                    node_id: 2,
                    tag: "strong".to_string(),
                    attributes: Vec::new(),
                    children: vec![HtmlDocumentNode::Text("important".to_string())],
                },
                HtmlDocumentNode::Text(" suffix".to_string()),
            ],
        }
    }

    fn unbroken_inline_children_fixture() -> HtmlDocumentNode {
        HtmlDocumentNode::Element {
            node_id: 3,
            tag: "td".to_string(),
            attributes: Vec::new(),
            children: vec![
                HtmlDocumentNode::Text("prefix".to_string()),
                HtmlDocumentNode::Element {
                    node_id: 4,
                    tag: "strong".to_string(),
                    attributes: Vec::new(),
                    children: vec![HtmlDocumentNode::Text("important".to_string())],
                },
                HtmlDocumentNode::Text("suffix".to_string()),
            ],
        }
    }

    #[test]
    fn normal_inline_children_keep_adjacent_min_content_runs() {
        let style = CssStyle::browser_default();
        let node = inline_children_fixture();
        let width = TableIntrinsic::min_content_width(&node, &style);
        let strong_width = TableIntrinsic::min_content_width(
            &HtmlDocumentNode::Element {
                node_id: 2,
                tag: "strong".to_string(),
                attributes: Vec::new(),
                children: vec![HtmlDocumentNode::Text("important".to_string())],
            },
            &style,
        );
        let prefix_width = TableIntrinsic::min_content_width(
            &HtmlDocumentNode::Text("prefix ".to_string()),
            &style,
        );
        assert!(width > 0.0);
        assert!(width >= strong_width.max(prefix_width));
        assert!(
            width < strong_width + prefix_width,
            "width={width}, strong={strong_width}, prefix={prefix_width}"
        );
    }

    #[test]
    fn normal_inline_children_join_unbroken_styled_runs() {
        let style = CssStyle::browser_default();
        let width = TableIntrinsic::min_content_width(&unbroken_inline_children_fixture(), &style);
        let prefix = TableIntrinsic::min_content_width(
            &HtmlDocumentNode::Text("prefix".to_string()),
            &style,
        );
        let suffix = TableIntrinsic::min_content_width(
            &HtmlDocumentNode::Text("suffix".to_string()),
            &style,
        );
        assert!(width > prefix + suffix);
    }

    #[test]
    fn inline_boundary_uses_trailing_segment_after_internal_break() {
        let style = CssStyle::browser_default();
        let node = HtmlDocumentNode::Element {
            node_id: 5,
            tag: "td".to_string(),
            attributes: Vec::new(),
            children: vec![
                HtmlDocumentNode::Text("reallylongword x".to_string()),
                HtmlDocumentNode::Element {
                    node_id: 6,
                    tag: "strong".to_string(),
                    attributes: Vec::new(),
                    children: vec![HtmlDocumentNode::Text("bar".to_string())],
                },
            ],
        };
        let width = TableIntrinsic::min_content_width(&node, &style);
        let text_width = TableIntrinsic::min_content_width(
            &HtmlDocumentNode::Text("reallylongword x".to_string()),
            &style,
        );
        let strong_width =
            TableIntrinsic::min_content_width(&HtmlDocumentNode::Text("bar".to_string()), &style);
        assert!(width < text_width + strong_width);
    }

    #[test]
    fn empty_inline_text_does_not_break_a_styled_run() {
        let style = CssStyle::browser_default();
        let node = HtmlDocumentNode::Element {
            node_id: 7,
            tag: "td".to_string(),
            attributes: Vec::new(),
            children: vec![
                HtmlDocumentNode::Text("prefix".to_string()),
                HtmlDocumentNode::Text(String::new()),
                HtmlDocumentNode::Element {
                    node_id: 8,
                    tag: "strong".to_string(),
                    attributes: Vec::new(),
                    children: vec![HtmlDocumentNode::Text("bar".to_string())],
                },
            ],
        };
        let width = TableIntrinsic::min_content_width(&node, &style);
        let prefix = TableIntrinsic::min_content_width(
            &HtmlDocumentNode::Text("prefix".to_string()),
            &style,
        );
        let strong =
            TableIntrinsic::min_content_width(&HtmlDocumentNode::Text("bar".to_string()), &style);
        assert!(width >= prefix + strong);
    }

    #[test]
    fn code_child_uses_user_agent_monospace_for_intrinsic_measurement() {
        let inherited = CssStyle::browser_default();
        let code = HtmlDocumentNode::Element {
            node_id: 10,
            tag: "code".to_string(),
            attributes: Vec::new(),
            children: vec![HtmlDocumentNode::Text("documents.example".to_string())],
        };
        let cell = HtmlDocumentNode::Element {
            node_id: 9,
            tag: "td".to_string(),
            attributes: Vec::new(),
            children: vec![code.clone()],
        };
        assert_eq!(
            super::child_style(&code, &inherited).font_family,
            "monospace"
        );
        assert!(TableIntrinsic::min_content_width(&cell, &inherited) > 0.0);
    }

    #[test]
    fn empty_inline_element_has_zero_intrinsic_width() {
        let node = HtmlDocumentNode::Element {
            node_id: 11,
            tag: "td".to_string(),
            attributes: Vec::new(),
            children: vec![HtmlDocumentNode::Element {
                node_id: 12,
                tag: "span".to_string(),
                attributes: Vec::new(),
                children: Vec::new(),
            }],
        };
        assert_eq!(
            TableIntrinsic::min_content_width(&node, &CssStyle::browser_default()),
            0.0
        );
    }

    #[test]
    fn parsed_table_cell_uses_no_wrap_element_intrinsic_width() -> Result<(), String> {
        let document = HtmlDocument::parse(
            "<table><caption>caption prefix</caption><tr><td>unrelated cell</td><td id=target style=\"white-space:nowrap\">prefix <strong>important phrase</strong> suffix</td></tr></table>",
        );
        let nodes = document.interactive_nodes_with_styles(&HashMap::new());
        let target_width = projected_target_cell_width(&nodes)?;
        let control = HtmlDocument::parse(
            "<table><tr><td id=target style=\"white-space:nowrap\">prefix <strong>important phrase</strong> suffix</td></tr></table>",
        );
        let control_nodes = control.interactive_nodes_with_styles(&HashMap::new());
        let control_width = projected_target_cell_width(&control_nodes)?;
        assert!(target_width > 0.0);
        assert_eq!(target_width, control_width);
        Ok(())
    }

    pub(super) fn projected_target_cell_width(nodes: &[HtmlDocumentNode]) -> Result<f32, String> {
        let cell = nodes
            .iter()
            .find_map(find_target_cell)
            .ok_or_else(|| "parsed table cell must be projected".to_string())?;
        let HtmlDocumentNode::Element {
            tag, attributes, ..
        } = cell
        else {
            return Err("target cell projection must be an element".to_string());
        };
        let style = CssStyle::from_element(tag, attributes, &CssStyle::browser_default());
        if style.white_space != super::CssWhiteSpace::NoWrap {
            return Err("target cell must retain nowrap style".to_string());
        }
        Ok(TableIntrinsic::min_content_width(cell, &style))
    }

    fn find_target_cell(node: &HtmlDocumentNode) -> Option<&HtmlDocumentNode> {
        match node {
            HtmlDocumentNode::Element {
                tag,
                attributes,
                children,
                ..
            } => {
                if tag == "td"
                    && attributes
                        .iter()
                        .any(|(name, value)| name == "id" && value == "target")
                {
                    return Some(node);
                }
                children.iter().find_map(find_target_cell)
            }
            HtmlDocumentNode::Text(_) => None,
        }
    }
}
