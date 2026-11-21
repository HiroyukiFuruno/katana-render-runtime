use super::{CssStyle, HtmlDocumentNode};
use crate::renderer::backends::html_browser::HtmlBrowserViewport;
use crate::renderer::backends::html_document::HtmlDocument;
use crate::renderer::backends::html_interactive::{
    layout::HtmlLayoutRenderer, types::DetailsContext,
};
use std::collections::HashMap;

const NEXT_NODE_ID: u64 = 3;
const VIEWPORT_WIDTH: u32 = 300;
const VIEWPORT_HEIGHT: u32 = 200;

fn element(id: u64, style: &str, children: Vec<HtmlDocumentNode>) -> HtmlDocumentNode {
    HtmlDocumentNode::Element {
        node_id: id,
        tag: "div".to_string(),
        attributes: vec![("style".to_string(), style.to_string())],
        children,
    }
}

fn render_margin_boxes(parent_style: &str, child_margin: f32) -> HtmlLayoutRenderer {
    let child = element(
        2,
        &format!("height:20px;margin-bottom:{child_margin}px"),
        vec![],
    );
    let nodes = [
        element(1, parent_style, vec![child]),
        element(NEXT_NODE_ID, "height:20px", vec![]),
    ];
    let viewport = HtmlBrowserViewport {
        width: VIEWPORT_WIDTH,
        height: VIEWPORT_HEIGHT,
        device_scale_factor: 1.0,
    };
    let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
    renderer.render_nodes(
        &nodes,
        0.0,
        0.0,
        VIEWPORT_WIDTH as f32,
        &CssStyle::browser_default(),
        DetailsContext::NONE,
    );
    renderer
}

#[test]
fn parent_and_last_child_margins_collapse_without_inflating_the_parent_box() {
    let renderer = render_margin_boxes("margin-bottom:26px", 16.0);
    assert_eq!(renderer.element_boxes[0].height, 20.0);
    assert_eq!(renderer.element_boxes[2].y, 46.0);
    let renderer = render_margin_boxes("margin-bottom:26px", -5.0);
    assert_eq!(renderer.element_boxes[0].height, 20.0);
    assert_eq!(renderer.element_boxes[2].y, 41.0);
}

#[test]
fn padding_border_and_formatting_contexts_keep_child_margins_inside_the_parent() {
    for (style, height) in [
        ("margin-bottom:26px;padding-bottom:4px", 40.0),
        ("margin-bottom:26px;border-bottom:1px solid black", 37.0),
        ("margin-bottom:26px;overflow:hidden", 36.0),
        ("margin-bottom:26px;display:flow-root", 36.0),
    ] {
        let renderer = render_margin_boxes(style, 16.0);
        assert_eq!(renderer.element_boxes[0].height, height, "{style}");
        assert_eq!(renderer.element_boxes[2].y, height + 26.0, "{style}");
    }
}

#[test]
fn flex_and_grid_item_trailing_margins_do_not_collapse_into_the_container() {
    for display in ["flex", "grid"] {
        let renderer = render_margin_boxes(&format!("display:{display};margin-bottom:26px"), 16.0);
        assert_eq!(renderer.element_boxes[0].height, 36.0, "{display}");
        assert_eq!(renderer.element_boxes[2].y, 62.0, "{display}");
    }
}

fn render_leading_margin_boxes(parent_style: &str) -> HtmlLayoutRenderer {
    let child = element(2, "height:20px;margin-top:16px", vec![]);
    let viewport = HtmlBrowserViewport {
        width: VIEWPORT_WIDTH,
        height: VIEWPORT_HEIGHT,
        device_scale_factor: 1.0,
    };
    let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
    renderer.render_nodes(
        &[element(1, parent_style, vec![child])],
        0.0,
        0.0,
        VIEWPORT_WIDTH as f32,
        &CssStyle::browser_default(),
        DetailsContext::NONE,
    );
    renderer
}

#[test]
fn parsed_flex_and_grid_items_keep_descendant_bottom_margin_inside() {
    for display in ["flex", "grid"] {
        assert_parsed_nested_margin_geometry(display);
    }
}

fn assert_parsed_nested_margin_geometry(display: &str) {
    let html = format!(
        "<style>html,body{{margin:0}}</style><main style='display:{display};flex-direction:column;grid-template-columns:120px;width:120px'><section id=item><div style='height:20px;margin-bottom:16px'></div></section></main><div id=after style='height:8px'></div>"
    );
    let mut document = HtmlDocument::parse(&html);
    let ids = document
        .get_element_by_id("item")
        .zip(document.get_element_by_id("after"));
    let nodes = document.interactive_nodes_with_styles(&HashMap::new());
    let viewport = HtmlBrowserViewport {
        width: VIEWPORT_WIDTH,
        height: VIEWPORT_HEIGHT,
        device_scale_factor: 1.0,
    };
    let rendered = HtmlLayoutRenderer::render(&nodes, viewport, 0.0, &HashMap::new(), None);
    assert!(
        rendered.is_ok_and(|layout| {
            ids.is_some_and(|(item_id, after_id)| {
                let geometry = |id| layout.element_boxes.iter().find(|item| item.node_id == id);
                geometry(item_id)
                    .zip(geometry(after_id))
                    .is_some_and(|(item, after)| item.height == 36.0 && after.y == 36.0)
            })
        }),
        "parsed {display} layout collapsed a flex/grid item's descendant margin"
    );
}

#[test]
fn first_child_margin_collapses_with_parent_without_double_vertical_offset() {
    let renderer = render_leading_margin_boxes("margin-top:8px");
    assert_eq!(renderer.element_boxes[0].y, 16.0);
    assert_eq!(renderer.element_boxes[1].y, 16.0);
    assert_eq!(renderer.element_boxes[0].height, 20.0);
    let renderer = render_leading_margin_boxes("margin-top:-5px");
    assert_eq!(renderer.element_boxes[0].y, 11.0);
    assert_eq!(renderer.element_boxes[1].y, 11.0);
    assert_eq!(renderer.element_boxes[0].height, 20.0);
}

#[test]
fn top_padding_and_formatting_context_keep_first_child_margin_inside() {
    for (style, child_y) in [
        ("margin-top:8px;padding-top:4px", 28.0),
        ("margin-top:8px;border-top:1px solid black", 25.0),
        ("margin-top:8px;overflow:hidden", 24.0),
        ("margin-top:8px;display:flow-root", 24.0),
    ] {
        let renderer = render_leading_margin_boxes(style);
        assert_eq!(renderer.element_boxes[0].y, 8.0, "{style}");
        assert_eq!(renderer.element_boxes[1].y, child_y, "{style}");
    }
}
