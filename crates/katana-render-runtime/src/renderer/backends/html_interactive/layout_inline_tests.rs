#[cfg(target_os = "macos")]
#[path = "layout_inline_font_test_support.rs"]
mod font_fixture;
#[cfg(target_os = "macos")]
use font_fixture::{
    CHROME_INLINE_VIEWPORT, STATUS_BADGE_ID, inline_status_fixture, inline_text_height,
    inline_text_height_at_width, strong_text_node, styled_inline_node,
};

use super::super::super::html_browser::HtmlBrowserViewport;
use super::super::types::DetailsContext;
use super::{
    CssFloat, CssStyle, HtmlDocumentNode, HtmlLayoutRenderer, InlineFloat, InlineFlowState,
    InlineMeasurement,
};
use std::collections::HashMap;

#[cfg(target_os = "macos")]
const INLINE_FLEX_BASELINE_AVAILABLE_WIDTH: f32 = 200.0;

#[cfg(target_os = "macos")]
#[test]
fn inline_flex_badge_and_smaller_second_line_preserve_parent_strut() {
    let (style, nodes) = inline_status_fixture();
    let mut renderer = HtmlLayoutRenderer::new(CHROME_INLINE_VIEWPORT, 0.0, &HashMap::new(), None);
    let height = renderer.render_nodes(&nodes, 0.0, 0.0, 376.0, &style, DetailsContext::NONE);
    assert_eq!(height, 41.171875);
    let badge_box = renderer
        .element_boxes
        .iter()
        .find(|element| element.node_id == STATUS_BADGE_ID);
    assert_eq!(badge_box.map(|element| element.y), Some(1.0));
}

#[cfg(target_os = "macos")]
#[test]
fn parsed_inline_flex_safety_preserves_baseline_metrics() -> Result<(), String> {
    let alignments = [
        (None, 13.0),
        (Some("center"), 26.0),
        (Some("unsafe center"), 26.0),
        (Some("safe center"), 13.0),
        (Some("end"), 39.0),
        (Some("unsafe end"), 39.0),
        (Some("safe end"), 13.0),
        (Some("flex-end"), 39.0),
        (Some("unsafe flex-end"), 39.0),
        (Some("safe flex-end"), 13.0),
    ];
    for (alignment, expected_baseline) in alignments {
        assert_inline_flex_baseline_case(alignment, expected_baseline)?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn assert_inline_flex_baseline_case(
    alignment: Option<&str>,
    expected_baseline: f32,
) -> Result<(), String> {
    let alignment_style = match alignment {
        Some(value) => format!(";align-items:{value}"),
        None => String::new(),
    };
    let node = styled_inline_node(
        1,
        &format!(
            "display:inline-flex;height:40px;border:1px solid;padding:2px 8px;font-family:Arial;font-size:10px;line-height:14px{alignment_style}"
        ),
        "Alpha",
    );
    let metrics = super::atomic_metrics::inline_flex_metrics(
        &node,
        &CssStyle::browser_default(),
        INLINE_FLEX_BASELINE_AVAILABLE_WIDTH,
    )
    .ok_or_else(|| format!("missing inline flex metrics for {alignment:?}"))?;
    assert_eq!(metrics.height, 46.0);
    assert_eq!(metrics.baseline, expected_baseline, "{alignment:?}");
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn mixed_weight_inline_line_preserves_chrome_font_extents() {
    let strong = strong_text_node("概要：");
    let text = HtmlDocumentNode::Text("日本語本文".to_string());
    assert_eq!(inline_text_height(&[strong.clone(), text.clone()]), 20.75);
    assert_eq!(inline_text_height(&[text, strong]), 20.75);
    assert_eq!(
        inline_text_height(&[strong_text_node("概要：日本語本文")]),
        19.75
    );
    assert_eq!(
        inline_text_height(&[HtmlDocumentNode::Text("概要：日本語本文".to_string())]),
        19.75
    );
}

#[cfg(target_os = "macos")]
#[test]
fn wrapped_inline_lines_use_only_the_font_extents_present_on_each_line() {
    let text = "長い日本語本文を確認するための文章です。幅に合わせて三行へ折り返します。追加の説明を続けます。";
    let nodes = [
        strong_text_node("概要："),
        HtmlDocumentNode::Text(text.to_string()),
    ];
    assert_eq!(inline_text_height_at_width(&nodes, 240.0), 60.25);
    let nodes = [
        strong_text_node(
            "三行分の長い日本語本文を確認するための文章です。幅に合わせて折り返します。",
        ),
        HtmlDocumentNode::Text("平文の追記です。".to_string()),
    ];
    assert_eq!(inline_text_height_at_width(&nodes, 240.0), 60.25);
}

#[cfg(target_os = "macos")]
#[test]
fn inline_font_size_changes_expand_the_used_line_box() {
    let mut strong = strong_text_node("概要：");
    if let HtmlDocumentNode::Element { attributes, .. } = &mut strong {
        attributes.push(("style".to_string(), "font-size:14px".to_string()));
    }
    let nodes = [strong, HtmlDocumentNode::Text("日本語本文".to_string())];
    assert_eq!(inline_text_height_at_width(&nodes, 240.0), 23.796875);
}

#[test]
fn explicit_break_advances_one_line_and_consecutive_breaks_keep_empty_lines() {
    let viewport = HtmlBrowserViewport {
        width: 400,
        height: 200,
        device_scale_factor: 1.0,
    };
    let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
    let style = CssStyle::browser_default();
    let line_break = explicit_line_break();
    let nodes = vec![
        HtmlDocumentNode::Text("First".to_string()),
        line_break.clone(),
        HtmlDocumentNode::Text("Second".to_string()),
    ];
    assert_eq!(
        renderer.render_nodes(&nodes, 0.0, 0.0, 400.0, &style, DetailsContext::NONE),
        style.line_height * 2.0
    );
    let nodes = vec![
        HtmlDocumentNode::Text("First".to_string()),
        line_break.clone(),
        line_break,
        HtmlDocumentNode::Text("Third".to_string()),
    ];
    assert_eq!(
        renderer.render_nodes(&nodes, 0.0, 0.0, 400.0, &style, DetailsContext::NONE),
        style.line_height * 3.0
    );
}

fn explicit_line_break() -> HtmlDocumentNode {
    HtmlDocumentNode::Element {
        node_id: 1,
        tag: "br".to_string(),
        attributes: vec![],
        children: vec![],
    }
}

#[test]
fn inline_node_width_uses_text_content_and_skips_block_nodes() {
    let inline = HtmlDocumentNode::Element {
        node_id: 1,
        tag: "a".to_string(),
        attributes: vec![(
            "style".to_string(),
            "display:inline-block;padding:6px".to_string(),
        )],
        children: vec![HtmlDocumentNode::Text("Open link".to_string())],
    };
    let block = HtmlDocumentNode::Element {
        node_id: 2,
        tag: "div".to_string(),
        attributes: Vec::new(),
        children: Vec::new(),
    };

    assert!(InlineMeasurement::node_width(&inline, &CssStyle::browser_default(), 300.0).is_some());
    assert_eq!(
        InlineMeasurement::node_width(&block, &CssStyle::browser_default(), 300.0),
        None
    );
}

#[test]
fn inline_flow_wraps_then_flushes_before_a_block() {
    let viewport = HtmlBrowserViewport {
        width: 100,
        height: 100,
        device_scale_factor: 1.0,
    };
    let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
    let style = CssStyle::browser_default();
    let node = HtmlDocumentNode::Text("wrapped".to_string());
    let mut inline = InlineFlowState::new(0.0, 0.0, 20.0);
    inline.has_items = true;
    inline.cursor_x = 15.0;
    inline.bottom = 20.0;

    renderer.render_inline_node(&node, 10.0, &style, DetailsContext::NONE, &mut inline);
    assert_eq!(inline.y, 20.0);
    renderer.render_block_node(&node, &style, DetailsContext::NONE, &mut inline);
    assert!(!inline.has_items);
}

#[test]
fn inline_node_width_respects_explicit_content_width() {
    let node = HtmlDocumentNode::Element {
        node_id: 3,
        tag: "a".to_string(),
        attributes: vec![(
            "style".to_string(),
            "display:inline-block;width:40px;padding:6px".to_string(),
        )],
        children: Vec::new(),
    };

    assert_eq!(
        InlineMeasurement::node_width(&node, &CssStyle::browser_default(), 300.0),
        Some(52.0)
    );
}

#[test]
fn leading_inline_whitespace_becomes_a_horizontal_text_offset() {
    let style = CssStyle::browser_default();
    let (offset, visible) = InlineMeasurement::visible_line("  next", &style);

    assert!(offset > 0.0);
    assert_eq!(visible, "next");
    assert_eq!(
        InlineMeasurement::visible_line("next", &style),
        (0.0, "next")
    );
}

#[test]
fn floated_nodes_shrink_to_content_and_hidden_or_normal_nodes_stay_in_flow() -> Result<(), String> {
    let floated = HtmlDocumentNode::Element {
        node_id: 4,
        tag: "span".to_string(),
        attributes: vec![("style".to_string(), "float:right;margin:0 5px".to_string())],
        children: vec![HtmlDocumentNode::Text("count".to_string())],
    };
    let hidden = HtmlDocumentNode::Element {
        node_id: 5,
        tag: "span".to_string(),
        attributes: vec![("style".to_string(), "float:left;display:none".to_string())],
        children: Vec::new(),
    };
    let normal = HtmlDocumentNode::Text("normal".to_string());

    let (side, width) = InlineFloat::node_geometry(&floated, &CssStyle::browser_default(), 200.0)
        .ok_or("float geometry was missing")?;
    assert_eq!(side, CssFloat::Right);
    assert!(width > 10.0);
    assert!(InlineFloat::node_geometry(&hidden, &CssStyle::browser_default(), 200.0).is_none());
    assert!(InlineFloat::node_geometry(&normal, &CssStyle::browser_default(), 200.0).is_none());
    Ok(())
}
