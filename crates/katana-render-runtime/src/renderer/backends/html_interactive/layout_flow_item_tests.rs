use super::super::types::DetailsContext;
use super::HtmlLayoutRenderer;
use super::{CssStyle, HtmlDocumentNode};
use crate::renderer::backends::html_browser::HtmlBrowserViewport;
use std::collections::HashMap;

const LIST_ITEM_NODE_ID: u64 = 3;
const TEST_VIEWPORT: HtmlBrowserViewport = HtmlBrowserViewport {
    width: 320,
    height: 240,
    device_scale_factor: 1.0,
};

#[test]
fn nested_flow_measurements_are_reused_within_one_frame() -> Result<(), String> {
    let root = nested_flow_node();
    let renderer = test_renderer();
    let style = CssStyle::browser_default();
    let first = renderer.measure_flow_node_height(&root, 300.0, &style, DetailsContext::NONE)?;
    let first_stats = renderer.flow_measurements.borrow().stats();
    let second = renderer.measure_flow_node_height(&root, 300.0, &style, DetailsContext::NONE)?;
    let second_stats = renderer.flow_measurements.borrow().stats();

    assert_eq!(first, second);
    assert!(first_stats.0 > 0, "nested remeasurement did not hit cache");
    assert_eq!(second_stats.0, first_stats.0 + 1);
    assert_eq!(second_stats.1, first_stats.1);
    Ok(())
}

#[test]
fn list_item_container_measurement_uses_a_distinct_cache_entry() -> Result<(), String> {
    let node = list_item_node();
    let renderer = test_renderer();
    let style = CssStyle::browser_default();
    let ordinary = renderer.measure_flow_node_height(&node, 144.0, &style, DetailsContext::NONE)?;
    let ordinary_stats = renderer.flow_measurements.borrow().stats();
    let list_item = renderer.measure_flow_node_height_with_list_mode(
        &node,
        144.0,
        &style,
        DetailsContext::NONE,
        true,
    )?;
    let list_stats = renderer.flow_measurements.borrow().stats();

    assert_ne!(ordinary, list_item);
    assert!(
        list_stats.1 > ordinary_stats.1,
        "list mode reused ordinary measurement"
    );
    Ok(())
}

fn list_item_node() -> HtmlDocumentNode {
    HtmlDocumentNode::Element {
        node_id: LIST_ITEM_NODE_ID,
        tag: "li".to_string(),
        attributes: vec![(
            "style".to_string(),
            "min-height:42px;padding:4px".to_string(),
        )],
        children: vec![HtmlDocumentNode::Text("Short".to_string())],
    }
}

fn nested_flow_node() -> HtmlDocumentNode {
    let nested = HtmlDocumentNode::Element {
        node_id: 2,
        tag: "section".to_string(),
        attributes: vec![("style".to_string(), "display:flex".to_string())],
        children: vec![HtmlDocumentNode::Text("Nested content".to_string())],
    };
    HtmlDocumentNode::Element {
        node_id: 1,
        tag: "main".to_string(),
        attributes: vec![(
            "style".to_string(),
            "display:flex;flex-direction:column".to_string(),
        )],
        children: vec![nested],
    }
}

fn test_renderer() -> HtmlLayoutRenderer {
    HtmlLayoutRenderer::new(TEST_VIEWPORT, 0.0, &HashMap::new(), None)
}
