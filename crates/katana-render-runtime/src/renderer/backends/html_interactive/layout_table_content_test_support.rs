use super::super::super::super::html_document::HtmlDocumentNode;
use super::super::super::document::TableCell;
use super::super::super::layout::HtmlLayoutRenderer;
use super::super::super::types::{ElementBox, LayoutResult};
use crate::renderer::backends::html_browser::HtmlBrowserViewport;
use crate::renderer::backends::html_document::HtmlDocument;
use std::collections::HashMap;

pub(super) const TEST_VIEWPORT_WIDTH: u32 = 320;
pub(super) const TEST_VIEWPORT_HEIGHT: u32 = 240;
pub(super) const TABLE_FIXTURE_IDS: [&str; 17] = [
    "head",
    "head-row",
    "head-cell",
    "head-cell-2",
    "empty",
    "body-a",
    "body-row-a",
    "body-cell-a",
    "body-cell-a-2",
    "body-row-a-2",
    "body-cell-a-3",
    "body-b",
    "body-row-b",
    "body-cell-b",
    "foot",
    "foot-row",
    "foot-cell",
];
pub(super) const NARROW_COLUMN_WIDTH: f32 = 20.0;
pub(super) const RICH_CELL_WIDTH: f32 = 200.0;
pub(super) const ZERO_SCROLL_Y: f32 = 0.0;
pub(super) const CELL_INSET: f32 = 8.0;
pub(super) const SPECIFIED_CELL_HEIGHT: f32 = 40.0;
pub(super) const NATURAL_LINE_HEIGHT: f32 = 20.0;
pub(super) const PADDED_SPECIFIED_HEIGHT: f32 = 114.0;
pub(super) const BORDER_BOX_SPECIFIED_HEIGHT: f32 = 100.0;
pub(super) const COLLAPSED_CONTENT_BOX_ROW_HEIGHT: f32 = 112.0;
pub(super) const COLLAPSED_BORDER_BOX_ROW_HEIGHT: f32 = 98.0;

pub(super) fn renderer() -> HtmlLayoutRenderer {
    HtmlLayoutRenderer::new(
        HtmlBrowserViewport {
            width: TEST_VIEWPORT_WIDTH,
            height: TEST_VIEWPORT_HEIGHT,
            device_scale_factor: 1.0,
        },
        0.0,
        &HashMap::new(),
        None,
    )
}

pub(super) fn render_table_html(html: &str) -> Result<LayoutResult, String> {
    let document = HtmlDocument::parse(html);
    let nodes = document.interactive_nodes_with_styles(&HashMap::new());
    HtmlLayoutRenderer::render(
        &nodes,
        HtmlBrowserViewport {
            width: TEST_VIEWPORT_WIDTH,
            height: TEST_VIEWPORT_HEIGHT,
            device_scale_factor: 1.0,
        },
        0.0,
        &HashMap::new(),
        None,
    )
}

pub(super) fn table_element_id(document: &mut HtmlDocument, id: &str) -> Result<u64, String> {
    document
        .get_element_by_id(id)
        .ok_or_else(|| format!("fixture element #{id} is missing"))
}

pub(super) fn table_bounds(layout: &LayoutResult, node_id: u64) -> Result<&ElementBox, String> {
    layout
        .element_boxes
        .iter()
        .find(|bounds| bounds.node_id == node_id)
        .ok_or_else(|| format!("table element bounds for node {node_id} are missing"))
}

pub(super) fn assert_table_union(
    layout: &LayoutResult,
    union_id: u64,
    members: &[u64],
) -> Result<(), String> {
    let union = table_bounds(layout, union_id)?;
    let first = table_bounds(
        layout,
        *members.first().ok_or("table union has no members")?,
    )?;
    let mut left = first.x;
    let mut top = first.y;
    let mut right = first.x + first.width;
    let mut bottom = first.y + first.height;
    for id in members.iter().skip(1) {
        let bounds = table_bounds(layout, *id)?;
        left = left.min(bounds.x);
        top = top.min(bounds.y);
        right = right.max(bounds.x + bounds.width);
        bottom = bottom.max(bounds.y + bounds.height);
    }
    assert_eq!(
        (union.x, union.y, union.width, union.height),
        (left, top, right - left, bottom - top)
    );
    Ok(())
}

pub(super) fn cell(style: &str, children: Vec<HtmlDocumentNode>) -> TableCell {
    TableCell {
        tag: "td".into(),
        attributes: vec![("style".into(), style.into())],
        row_attributes: Vec::new(),
        row_group_tag: None,
        row_group_attributes: Vec::new(),
        element_ids: None,
        children,
    }
}
