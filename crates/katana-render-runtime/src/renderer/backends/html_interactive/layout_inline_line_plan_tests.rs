use super::{CssStyle, HashSet, HtmlDocumentNode, InlineLinePlan};

const PARENT_FONT_SIZE: f32 = 16.0;
const PARENT_LINE_HEIGHT: f32 = 24.0;

fn span(style: &str, text: &str) -> HtmlDocumentNode {
    HtmlDocumentNode::Element {
        node_id: 1,
        tag: "span".to_string(),
        attributes: vec![("style".to_string(), style.to_string())],
        children: vec![HtmlDocumentNode::Text(text.to_string())],
    }
}

fn inherited() -> CssStyle {
    CssStyle {
        font_family: "Noto Sans".to_string(),
        font_size: PARENT_FONT_SIZE,
        line_height: PARENT_LINE_HEIGHT,
        normal_line_height: false,
        ..CssStyle::browser_default()
    }
}

#[test]
fn smaller_inline_font_preserves_the_parent_line_strut() -> Result<(), String> {
    let plan = InlineLinePlan::for_run(
        &[span("font-size:8px;line-height:12px", "small")],
        &inherited(),
        &HashSet::new(),
        200.0,
    );
    assert_eq!(plan.advance(0, 1).ok_or("line advance missing")?, 24.0);
    assert!(plan.advance(1, 1).is_none());
    Ok(())
}

#[test]
fn atomic_inline_flex_wraps_as_one_box_after_existing_text() -> Result<(), String> {
    let plan = InlineLinePlan::for_run(
        &[
            HtmlDocumentNode::Text("prefix".to_string()),
            span("display:inline-flex;width:30px;height:28px", "AV"),
        ],
        &inherited(),
        &HashSet::new(),
        50.0,
    );
    assert_eq!(plan.metrics.len(), 2);
    assert_eq!(plan.advance(0, 1).ok_or("first line missing")?, 24.0);
    assert!(plan.advance(1, 1).ok_or("atomic line missing")? >= 28.0);
    Ok(())
}

#[test]
fn empty_and_block_nodes_keep_a_single_parent_line() -> Result<(), String> {
    let block = HtmlDocumentNode::Element {
        node_id: 2,
        tag: "div".to_string(),
        attributes: Vec::new(),
        children: vec![HtmlDocumentNode::Text("block".to_string())],
    };
    let plan = InlineLinePlan::for_run(
        &[HtmlDocumentNode::Text(String::new()), block],
        &inherited(),
        &HashSet::new(),
        200.0,
    );
    assert_eq!(plan.metrics.len(), 1);
    assert_eq!(plan.advance(0, 1).ok_or("parent line missing")?, 24.0);
    Ok(())
}
