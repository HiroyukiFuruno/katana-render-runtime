use super::{CssStyle, HashSet, HtmlDocumentNode, InlineLinePlan};
use crate::renderer::backends::html_document::HtmlDocument;
use std::collections::HashMap;

const AVAILABLE_LINE_WIDTH: f32 = 200.0;

#[test]
fn parsed_smaller_inline_font_keeps_parent_line_strut() -> Result<(), String> {
    let document = HtmlDocument::parse(
        "prefix <em>unrelated</em><span style=\"font-size:8px;line-height:12px\">small</span>",
    );
    let nodes = document.interactive_nodes_with_styles(&HashMap::new());
    let span = find_span(&nodes).ok_or("span projection missing")?;
    let mut inherited = CssStyle::browser_default();
    inherited.font_size = 16.0;
    inherited.line_height = 24.0;
    let HtmlDocumentNode::Element {
        tag, attributes, ..
    } = span
    else {
        unreachable!("find_span returns an element");
    };
    let child_style = CssStyle::from_element(tag, attributes, &inherited);
    assert_eq!(
        (child_style.font_size, child_style.line_height),
        (8.0, 12.0)
    );
    assert_line_advance(span, &child_style, 12.0)?;
    assert_line_advance(span, &inherited, 24.0)?;
    Ok(())
}

fn assert_line_advance(
    span: &HtmlDocumentNode,
    style: &CssStyle,
    expected: f32,
) -> Result<(), String> {
    let plan = InlineLinePlan::for_run(
        std::slice::from_ref(span),
        style,
        &HashSet::new(),
        AVAILABLE_LINE_WIDTH,
    );
    assert_eq!(plan.advance(0, 1).ok_or("line advance missing")?, expected);
    Ok(())
}

fn find_span(nodes: &[HtmlDocumentNode]) -> Option<&HtmlDocumentNode> {
    nodes.iter().find_map(|node| match node {
        HtmlDocumentNode::Element { tag, .. } if tag == "span" => Some(node),
        HtmlDocumentNode::Element { children, .. } => find_span(children),
        HtmlDocumentNode::Text(_) => None,
    })
}
