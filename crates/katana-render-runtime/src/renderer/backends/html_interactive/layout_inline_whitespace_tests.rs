use super::super::super::line_plan::InlineLinePlan;
use crate::renderer::backends::html_browser::HtmlBrowserViewport;
use crate::renderer::backends::html_document::{HtmlDocument, HtmlDocumentNode};
use crate::renderer::backends::html_interactive::{
    layout::HtmlLayoutRenderer, style::CssStyle, types::DetailsContext,
};
use std::collections::{HashMap, HashSet};

const VIEWPORT_WIDTH: u32 = 400;
const VIEWPORT_HEIGHT: u32 = 200;

fn paint_text(text: &str) -> (String, f32, f32) {
    let viewport = HtmlBrowserViewport {
        width: VIEWPORT_WIDTH,
        height: VIEWPORT_HEIGHT,
        device_scale_factor: 1.0,
    };
    let mut renderer = HtmlLayoutRenderer::new(viewport, 0.0, &HashMap::new(), None);
    let style = CssStyle::browser_default();
    let bottom = renderer.render_nodes(
        &[HtmlDocumentNode::Text(text.to_string())],
        0.0,
        0.0,
        VIEWPORT_WIDTH as f32,
        &style,
        DetailsContext::NONE,
    );
    (renderer.svg, bottom, style.line_height)
}

fn find_class<'a>(nodes: &'a [HtmlDocumentNode], class_name: &str) -> Option<&'a HtmlDocumentNode> {
    nodes.iter().find_map(|node| match node {
        HtmlDocumentNode::Element {
            attributes,
            children,
            ..
        } => {
            if attributes
                .iter()
                .any(|(name, value)| name == "class" && value == class_name)
            {
                Some(node)
            } else {
                find_class(children, class_name)
            }
        }
        HtmlDocumentNode::Text(_) => None,
    })
}

fn probe_parts(html: &str) -> Result<(CssStyle, Vec<HtmlDocumentNode>), String> {
    let document = HtmlDocument::parse(html);
    let nodes = document.interactive_nodes_with_styles(&HashMap::new());
    let probe = find_class(&nodes, "probe").ok_or("probe projection missing")?;
    let HtmlDocumentNode::Element {
        tag,
        attributes,
        children,
        ..
    } = probe
    else {
        return Err("probe projection was text".to_string());
    };
    Ok((
        CssStyle::from_element(tag, attributes, &CssStyle::browser_default()),
        children.clone(),
    ))
}

fn render_probe(
    children: &[HtmlDocumentNode],
    style: &CssStyle,
    width: f32,
) -> (HtmlLayoutRenderer, f32) {
    let mut renderer = HtmlLayoutRenderer::new(
        HtmlBrowserViewport {
            width: width.ceil() as u32,
            height: VIEWPORT_HEIGHT,
            device_scale_factor: 1.0,
        },
        0.0,
        &HashMap::new(),
        None,
    );
    let bottom = renderer.render_nodes(children, 0.0, 0.0, width, style, DetailsContext::NONE);
    (renderer, bottom)
}

fn svg_text_element<'a>(svg: &'a str, text: &str) -> Result<&'a str, String> {
    let end = svg
        .find(&format!(">{text}<"))
        .ok_or_else(|| format!("{text} text missing"))?;
    let start = svg[..end]
        .rfind("<text ")
        .ok_or_else(|| format!("{text} text element missing"))?;
    Ok(&svg[start..end])
}

fn svg_attribute(element: &str, name: &str) -> Result<f32, String> {
    element
        .split(&format!("{name}=\""))
        .nth(1)
        .and_then(|value| value.split('"').next())
        .ok_or_else(|| format!("{name} missing"))?
        .parse::<f32>()
        .map_err(|error| format!("{name} parse failed: {error}"))
}

#[test]
fn source_indentation_and_tabs_do_not_create_visible_line_breaks() {
    let (svg, bottom, line_height) = paint_text("\n  First\t second\n  ");
    assert_eq!(bottom, line_height);
    assert!(svg.contains(">First second</text>"), "{svg}");
    assert_eq!(svg.matches("<text ").count(), 1);
}

#[test]
fn nonbreaking_spaces_remain_in_the_rendered_text() {
    let (svg, bottom, line_height) = paint_text("A\u{a0}B\u{a0}");
    assert_eq!(bottom, line_height);
    assert!(svg.contains(">A\u{a0}B\u{a0}</text>"), "{svg}");
}

#[test]
fn collapsible_whitespace_alone_does_not_create_a_text_box() {
    let (svg, bottom, _) = paint_text("\n\t \r");
    assert_eq!(bottom, 0.0);
    assert!(!svg.contains("<text "), "{svg}");
}

#[test]
fn parsed_mixed_font_whitespace_uses_preceding_text_owner_for_geometry() -> Result<(), String> {
    let html = "<div class=probe style=\"font-family:Helvetica Neue;font-size:14px;line-height:24px\">履歴表の <code style=\"font-family:Menlo\">&lt;tbody&gt;</code></div>";
    let (style, children) = probe_parts(html)?;
    let code = children
        .iter()
        .find(|node| matches!(node, HtmlDocumentNode::Element { tag, .. } if tag == "code"))
        .ok_or("code projection missing")?;
    let code_id = match code {
        HtmlDocumentNode::Element { node_id, .. } => *node_id,
        HtmlDocumentNode::Text(_) => return Err("code projection was text".to_string()),
    };
    let (renderer, _) = render_probe(&children, &style, VIEWPORT_WIDTH as f32);
    let code_box = renderer
        .element_boxes
        .iter()
        .find(|element| element.node_id == code_id)
        .ok_or("code box missing")?;
    let expected = super::super::super::InlineMeasurement::text_width("履歴表の", &style)
        + super::super::super::InlineMeasurement::text_width(" ", &style);
    assert!(
        (code_box.x - expected).abs() <= 1.0 / 64.0,
        "code_x={}, expected={expected}",
        code_box.x
    );
    Ok(())
}

#[test]
fn parsed_empty_boundaries_keep_renderer_and_planner_in_sync() -> Result<(), String> {
    let html = "<div class=probe style=\"font-size:16px;line-height:24px\">prefix<span></span> <span></span> <span></span> next</div>";
    let (style, children) = probe_parts(html)?;
    let text_width = super::super::super::InlineMeasurement::text_width;
    let width =
        text_width("prefix", &style) + text_width("next", &style) + text_width(" ", &style) * 1.5;
    let plan = InlineLinePlan::for_run(&children, &style, &HashSet::new(), width);
    assert!(
        plan.advance(0, 2).is_none(),
        "collapsed spaces must fit one line"
    );
    let (renderer, bottom) = render_probe(&children, &style, width);
    let planned = plan.advance(0, 1).ok_or("planned line missing")?;
    assert!((bottom - planned).abs() <= 1.0 / 64.0);
    let next_x = svg_attribute(svg_text_element(&renderer.svg, "next")?, "x")?;
    let expected = text_width("prefix", &style) + text_width(" ", &style);
    assert!((next_x - expected).abs() <= 1.0 / 64.0);
    Ok(())
}

#[test]
fn parsed_atomic_after_empty_boundaries_starts_on_the_wrapped_line() -> Result<(), String> {
    let html = "<div class=probe style=\"font-size:16px;line-height:24px\">AB<span></span> <span></span><span style=\"display:inline-flex;width:20px;height:24px\">AV</span></div>";
    let (style, children) = probe_parts(html)?;
    let atomic = children.iter().find(|node| matches!(node, HtmlDocumentNode::Element { attributes, .. } if attributes.iter().any(|(_, value)| value.contains("inline-flex")))).ok_or("atomic projection missing")?;
    let atomic_id = match atomic {
        HtmlDocumentNode::Element { node_id, .. } => *node_id,
        HtmlDocumentNode::Text(_) => return Err("atomic projection was text".to_string()),
    };
    let measure = super::super::super::InlineMeasurement::text_width;
    let atomic_width = super::super::super::InlineMeasurement::node_width(atomic, &style, 200.0)
        .ok_or("atomic width missing")?;
    let width = measure("AB", &style) + atomic_width + measure(" ", &style) * 0.5;
    let (renderer, bottom) = render_probe(&children, &style, width);
    let plan = InlineLinePlan::for_run(&children, &style, &HashSet::new(), width);
    let planned = plan.advance(0, 2).ok_or("atomic wrap was not planned")?;
    assert!((bottom - planned).abs() <= 1.0 / 64.0);
    let atomic_box = renderer
        .element_boxes
        .iter()
        .find(|element| element.node_id == atomic_id)
        .ok_or("atomic box missing")?;
    assert_eq!(atomic_box.x, 0.0);
    assert!(atomic_box.y >= style.line_height);
    Ok(())
}

#[test]
fn parsed_break_drops_pending_space_before_next_line() -> Result<(), String> {
    let (style, children) = probe_parts("<div class=probe>prefix <br>next</div>")?;
    let (renderer, _) = render_probe(&children, &style, VIEWPORT_WIDTH as f32);
    let next = svg_text_element(&renderer.svg, "next")?;
    assert_eq!(svg_attribute(next, "x")?, 0.0);
    assert!(svg_attribute(next, "y")? > 0.0);
    Ok(())
}
