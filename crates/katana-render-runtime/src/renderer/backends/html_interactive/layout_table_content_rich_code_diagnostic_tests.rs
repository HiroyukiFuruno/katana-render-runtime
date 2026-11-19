use super::super::super::table_cell_horizontal_inset;
use crate::renderer::backends::html_browser::HtmlBrowserViewport;
use crate::renderer::backends::html_document::{HtmlDocument, HtmlDocumentNode};
use crate::renderer::backends::html_interactive::layout::HtmlLayoutRenderer;
use crate::renderer::backends::html_interactive::layout_inline::InlineMeasurement;
use crate::renderer::backends::html_interactive::style::CssStyle;
use crate::renderer::backends::html_interactive::types::LayoutResult;
use std::collections::HashMap;

const DIAGNOSTIC_ENV: &str = "KRR_HTML_RICH_CODE_DIAGNOSTIC";
const VIEWPORT_WIDTH: u32 = 620;
const VIEWPORT_HEIGHT: u32 = 900;
const URL: &str = "https://documents.example.invalid/preview/";
const PREFIX: &str = "URLの例は";
const SUFFIX: &str = "です。ここに示した";
const LINE_POSITION_EPSILON: f32 = 0.01;
const FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/html/anonymized-requirements.html"
));

struct RichCodeContext {
    cell_node_id: u64,
    cell_style: CssStyle,
    code_style: CssStyle,
}

struct RichCodeMetrics {
    cell_x: f32,
    cell_y: f32,
    cell_width: f32,
    cell_height: f32,
    available_width: f32,
    prefix_width: f32,
    code_width: f32,
    suffix_width: f32,
    code_font_family: String,
    code_font_size: f32,
    prefix_positions: Vec<(f32, f32)>,
    code_positions: Vec<(f32, f32)>,
    suffix_positions: Vec<(f32, f32)>,
    line_count: usize,
}

#[test]
fn reports_anonymized_rich_code_width_metrics_when_requested() -> Result<(), String> {
    if std::env::var_os(DIAGNOSTIC_ENV).is_none() {
        return Ok(());
    }
    let metrics = rich_code_metrics()?;

    println!(
        "public-rich-code-metrics cell=({:.3},{:.3},{:.3},{:.3}) cell_available_width={:.3} prefix_width={:.3} code_width={:.3} suffix_width={:.3} prefix_positions={:?} code_positions={:?} suffix_positions={:?} cell_line_count={}",
        metrics.cell_x,
        metrics.cell_y,
        metrics.cell_width,
        metrics.cell_height,
        metrics.available_width,
        metrics.prefix_width,
        metrics.code_width,
        metrics.suffix_width,
        metrics.prefix_positions,
        metrics.code_positions,
        metrics.suffix_positions,
        metrics.line_count,
    );
    println!(
        "public-rich-code-style code_font_family={:?} code_font_size={:.3}",
        metrics.code_font_family, metrics.code_font_size,
    );
    Ok(())
}

fn rich_code_metrics() -> Result<RichCodeMetrics, String> {
    let (layout, context) = public_fixture_layout()?;
    let cell = layout
        .element_boxes
        .iter()
        .find(|element| element.node_id == context.cell_node_id)
        .ok_or("public rich code cell layout box was not rendered")?;
    let painted_text = svg_text_positions(&layout.svg);

    Ok(RichCodeMetrics {
        cell_x: cell.x,
        cell_y: cell.y,
        cell_width: cell.width,
        cell_height: cell.height,
        available_width: (cell.width - table_cell_horizontal_inset(&context.cell_style)).max(0.0),
        prefix_width: InlineMeasurement::text_width(PREFIX, &context.cell_style),
        code_width: InlineMeasurement::text_width(URL, &context.code_style),
        suffix_width: InlineMeasurement::text_width(SUFFIX, &context.cell_style),
        code_font_family: context.code_style.font_family.clone(),
        code_font_size: context.code_style.font_size,
        prefix_positions: marked_positions(&painted_text, PREFIX),
        code_positions: marked_positions(&painted_text, URL),
        suffix_positions: marked_positions(&painted_text, SUFFIX),
        line_count: cell_line_count(&painted_text, cell.x, cell.y, cell.width, cell.height),
    })
}

fn public_fixture_layout() -> Result<(LayoutResult, RichCodeContext), String> {
    let document = HtmlDocument::parse(FIXTURE);
    let nodes =
        document.interactive_nodes_with_styles_at_width(&HashMap::new(), VIEWPORT_WIDTH as f32);
    let inherited =
        CssStyle::browser_default_for_viewport(VIEWPORT_WIDTH as f32, VIEWPORT_HEIGHT as f32);
    let context = rich_code_context(&nodes, &inherited, None)
        .ok_or("public rich code cell was not projected")?;
    let viewport = HtmlBrowserViewport::new(VIEWPORT_WIDTH, VIEWPORT_HEIGHT, 1.0)
        .map_err(|error| error.to_string())?;
    let layout = HtmlLayoutRenderer::render(&nodes, viewport, 0.0, &HashMap::new(), None)?;
    Ok((layout, context))
}

fn rich_code_context(
    nodes: &[HtmlDocumentNode],
    inherited: &CssStyle,
    cell: Option<(u64, CssStyle)>,
) -> Option<RichCodeContext> {
    nodes
        .iter()
        .find_map(|node| rich_code_context_for_node(node, inherited, cell.clone()))
}

fn rich_code_context_for_node(
    node: &HtmlDocumentNode,
    inherited: &CssStyle,
    cell: Option<(u64, CssStyle)>,
) -> Option<RichCodeContext> {
    let HtmlDocumentNode::Element {
        node_id,
        tag,
        attributes,
        children,
    } = node
    else {
        return None;
    };
    let style = CssStyle::from_element(tag, attributes, inherited);
    let cell = (tag == "td").then(|| (*node_id, style.clone())).or(cell);
    if tag == "code" && contains_url(children) {
        let (cell_node_id, cell_style) = cell?;
        return Some(RichCodeContext {
            cell_node_id,
            cell_style,
            code_style: style,
        });
    }
    rich_code_context(children, &style, cell)
}

fn contains_url(nodes: &[HtmlDocumentNode]) -> bool {
    nodes.iter().any(|node| match node {
        HtmlDocumentNode::Text(text) => text == URL,
        HtmlDocumentNode::Element { children, .. } => contains_url(children),
    })
}

fn marked_positions(text: &[(f32, f32, String)], marker: &str) -> Vec<(f32, f32)> {
    text.iter()
        .filter(|(_, _, content)| content.contains(marker))
        .map(|(x, y, _)| (x, y))
        .collect()
}

fn cell_line_count(text: &[(f32, f32, String)], x: f32, y: f32, width: f32, height: f32) -> usize {
    let mut lines = text
        .iter()
        .filter_map(|(text_x, text_y, _)| {
            (*text_x >= x && *text_x <= x + width && *text_y >= y && *text_y <= y + height)
                .then_some(*text_y)
        })
        .collect::<Vec<_>>();
    lines.sort_by(f32::total_cmp);
    lines.dedup_by(|left, right| (*left - *right).abs() < LINE_POSITION_EPSILON);
    lines.len()
}

fn svg_text_positions(svg: &str) -> Vec<(f32, f32, String)> {
    let document = roxmltree::Document::parse(&format!("<svg>{svg}</svg>"));
    let Ok(document) = document else {
        return Vec::new();
    };
    document
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .filter_map(|node| {
            Some((
                node.attribute("x")?.parse().ok()?,
                node.attribute("y")?.parse().ok()?,
                node.descendants()
                    .filter(|child| child.is_text())
                    .filter_map(|child| child.text())
                    .collect(),
            ))
        })
        .collect()
}
