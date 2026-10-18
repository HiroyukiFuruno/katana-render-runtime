use super::super::super::style::CssStyle;
use super::super::{
    TextMeasurementDiagnosticScope, TextMeasurementDiagnosticStats, start_style_timing,
    start_taffy_timing,
};
use crate::renderer::backends::html_browser::HtmlBrowserViewport;
use crate::renderer::backends::html_document::{HtmlDocument, HtmlDocumentNode};
use crate::renderer::backends::html_interactive::layout::HtmlLayoutRenderer;
use std::collections::HashMap;
use std::panic::AssertUnwindSafe;

const TEST_VIEWPORT_WIDTH: u32 = 160;
const TEST_VIEWPORT_HEIGHT: u32 = 80;
const NATURAL_LAYOUT_WIDTH: u32 = 120;
const NATURAL_LAYOUT_HEIGHT: u32 = 160;
const NO_SCROLL_Y: f32 = 0.0;
const DEVICE_SCALE: f32 = 1.0;
const ZERO_LENGTH: f32 = 0.0;

#[path = "text_measurement_column_alignment_tests.rs"]
mod column_alignment;

fn real_style() -> CssStyle {
    CssStyle::from_element(
        "p",
        &[(
            "style".to_string(),
            "color: #123456; font-size: 18px".to_string(),
        )],
        &CssStyle::browser_default(),
    )
}

fn real_flex_nodes() -> Vec<HtmlDocumentNode> {
    let child = HtmlDocumentNode::Element {
        node_id: 2,
        tag: "div".to_string(),
        attributes: vec![("style".to_string(), "width:24px;height:12px".to_string())],
        children: Vec::new(),
    };
    vec![HtmlDocumentNode::Element {
        node_id: 1,
        tag: "div".to_string(),
        attributes: vec![(
            "style".to_string(),
            "display:flex;width:120px;height:40px".to_string(),
        )],
        children: vec![child],
    }]
}

fn find_element_id(nodes: &[HtmlDocumentNode], id: &str) -> Option<u64> {
    nodes.iter().find_map(|node| match node {
        HtmlDocumentNode::Element {
            node_id,
            attributes,
            children,
            ..
        } => {
            if attributes
                .iter()
                .any(|(name, value)| name == "id" && value == id)
            {
                Some(*node_id)
            } else {
                find_element_id(children, id)
            }
        }
        HtmlDocumentNode::Text(_) => None,
    })
}

fn parsed_nodes(html: &str) -> Vec<HtmlDocumentNode> {
    HtmlDocument::parse(html).interactive_nodes_with_styles(&HashMap::new())
}

fn real_flex_layout() -> Result<(), String> {
    let nodes = real_flex_nodes();
    let rendered = HtmlLayoutRenderer::render_with_clickable_nodes(
        &nodes,
        HtmlBrowserViewport {
            width: TEST_VIEWPORT_WIDTH,
            height: TEST_VIEWPORT_HEIGHT,
            device_scale_factor: 1.0,
        },
        0.0,
        &std::collections::HashMap::new(),
        None,
        &std::collections::HashSet::new(),
    )?;
    let root_box = rendered
        .element_boxes
        .iter()
        .find(|element_box| element_box.node_id == 1)
        .ok_or_else(|| "real flex root box should be present".to_string())?;
    assert_eq!((root_box.width, root_box.height), (120.0, 40.0));
    Ok(())
}

#[test]
fn disabled_layout_timing_scope_stays_empty_for_real_calls() -> Result<(), String> {
    let mut scope = TextMeasurementDiagnosticScope::enter(false);
    assert!(start_style_timing().is_none());
    assert!(start_taffy_timing().is_none());
    let styled = real_style();
    assert_eq!(styled.color, "#123456");
    real_flex_layout()?;
    assert_eq!(scope.finish(), TextMeasurementDiagnosticStats::default());
    Ok(())
}

#[test]
fn real_style_call_preserves_values_and_records_one_call() {
    let mut scope = TextMeasurementDiagnosticScope::enter(true);
    let styled = real_style();
    assert_eq!(styled.color, "#123456");
    assert_eq!(styled.font_size, 18.0);
    let stats = scope.finish();
    assert_eq!(stats.style_calls, 1);
}

#[test]
fn real_taffy_call_preserves_dimensions_and_records_calls() -> Result<(), String> {
    let mut scope = TextMeasurementDiagnosticScope::enter(true);
    real_flex_layout()?;
    let stats = scope.finish();
    assert!(stats.taffy_calls > 0);
    Ok(())
}

#[test]
fn nested_enabled_scope_isolates_real_style_and_taffy_counts() -> Result<(), String> {
    let mut outer = TextMeasurementDiagnosticScope::enter(true);
    let _ = real_style();
    let mut inner = TextMeasurementDiagnosticScope::enter(true);
    real_flex_layout()?;
    let inner_stats = inner.finish();
    assert!(inner_stats.style_calls > 0);
    assert!(inner_stats.taffy_calls > 0);
    let outer_stats = outer.finish();
    assert_eq!(outer_stats.style_calls, 1);
    assert_eq!(outer_stats.taffy_calls, 0);
    Ok(())
}

#[test]
fn disabled_nested_scope_suppresses_real_style_and_taffy_timing() -> Result<(), String> {
    let mut outer = TextMeasurementDiagnosticScope::enter(true);
    let _ = real_style();
    let mut disabled = TextMeasurementDiagnosticScope::enter(false);
    let styled = real_style();
    assert_eq!(styled.color, "#123456");
    real_flex_layout()?;
    assert_eq!(disabled.finish(), TextMeasurementDiagnosticStats::default());
    let outer_stats = outer.finish();
    assert_eq!(outer_stats.style_calls, 1);
    assert_eq!(outer_stats.taffy_calls, 0);
    Ok(())
}

#[test]
fn layout_timing_scope_cleanup_survives_nested_unwind() -> Result<(), String> {
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| -> Result<(), String> {
        let _scope = TextMeasurementDiagnosticScope::enter(true);
        let _ = real_style();
        real_flex_layout()?;
        std::panic::resume_unwind(Box::new("layout diagnostic unwind"));
    }));
    assert!(result.is_err());
    assert!(start_style_timing().is_none());
    assert!(start_taffy_timing().is_none());

    let mut outer = TextMeasurementDiagnosticScope::enter(true);
    let nested_result = std::panic::catch_unwind(AssertUnwindSafe(|| -> Result<(), String> {
        let _inner = TextMeasurementDiagnosticScope::enter(true);
        let _ = real_style();
        real_flex_layout()?;
        std::panic::resume_unwind(Box::new("nested layout diagnostic unwind"));
    }));
    assert!(nested_result.is_err());
    let _ = real_style();
    let stats = outer.finish();
    assert_eq!(stats.style_calls, 1);
    assert_eq!(stats.taffy_calls, 0);
    Ok(())
}

#[test]
fn repeated_finish_does_not_consume_restored_real_layout_timing() -> Result<(), String> {
    let mut outer = TextMeasurementDiagnosticScope::enter(true);
    let mut inner = TextMeasurementDiagnosticScope::enter(true);
    let _ = real_style();
    real_flex_layout()?;
    let inner_stats = inner.finish();
    assert!(inner_stats.style_calls > 0);
    assert!(inner_stats.taffy_calls > 0);
    assert_eq!(inner.finish(), TextMeasurementDiagnosticStats::default());

    let _ = real_style();
    let outer_stats = outer.finish();
    assert_eq!(outer_stats.style_calls, 1);
    assert_eq!(outer_stats.taffy_calls, 0);
    Ok(())
}

#[test]
fn parsed_column_reverse_nonstretch_keeps_intrinsic_cross_size() -> Result<(), String> {
    let html = "<style>html,body{margin:0}</style><div id=reverse style='display:flex;flex-direction:column-reverse;align-items:center;width:120px'><span id=child style='white-space:nowrap'>small</span></div>";
    let nodes = parsed_nodes(html);
    let reverse_id = find_element_id(&nodes, "reverse").ok_or("reverse element missing")?;
    let child_id = find_element_id(&nodes, "child").ok_or("child element missing")?;
    let viewport = HtmlBrowserViewport {
        width: NATURAL_LAYOUT_WIDTH,
        height: NATURAL_LAYOUT_HEIGHT,
        device_scale_factor: DEVICE_SCALE,
    };
    let rendered =
        HtmlLayoutRenderer::render(&nodes, viewport, NO_SCROLL_Y, &HashMap::new(), None)?;
    let reverse = rendered
        .element_boxes
        .iter()
        .find(|element| element.node_id == reverse_id)
        .ok_or("reverse box missing")?;
    let child = rendered
        .element_boxes
        .iter()
        .find(|element| element.node_id == child_id)
        .ok_or("child box missing")?;
    assert_eq!(reverse.width, NATURAL_LAYOUT_WIDTH as f32);
    assert!(child.width > ZERO_LENGTH && child.width < reverse.width);
    Ok(())
}

#[test]
fn parsed_sparse_table_renders_all_cells() -> Result<(), String> {
    let html = "<style>html,body{margin:0}</style><table id=table style='width:120px'><tbody><tr><td id=first style='background:#112233'>one</td></tr><tr><td id=second style='width:40px;background:#223344'>two</td><td id=third style='background:#334455'>longer content</td></tr></tbody></table>";
    let nodes = parsed_nodes(html);
    let table_id = find_element_id(&nodes, "table").ok_or("table element missing")?;
    let viewport = HtmlBrowserViewport {
        width: NATURAL_LAYOUT_WIDTH,
        height: NATURAL_LAYOUT_HEIGHT,
        device_scale_factor: DEVICE_SCALE,
    };
    let rendered =
        HtmlLayoutRenderer::render(&nodes, viewport, NO_SCROLL_Y, &HashMap::new(), None)?;
    let table = rendered
        .element_boxes
        .iter()
        .find(|element| element.node_id == table_id)
        .ok_or("table box missing")?;
    assert_eq!(table.width, NATURAL_LAYOUT_WIDTH as f32);
    assert!(rendered.svg.contains("fill=\"#112233\""));
    assert!(rendered.svg.contains("fill=\"#223344\""));
    assert!(rendered.svg.contains("fill=\"#334455\""));
    assert!(rendered.svg.contains("one"));
    assert!(rendered.svg.contains("two"));
    assert!(rendered.svg.contains("longer"));
    assert!(rendered.svg.contains("content"));
    Ok(())
}
