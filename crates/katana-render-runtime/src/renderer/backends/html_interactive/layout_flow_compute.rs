use super::super::super::html_document::HtmlDocumentNode;
use super::super::layout_flow_measure::is_layout_item;
use super::super::layout_grid_track::taffy_grid_track;
use super::super::style::CssStyle;
use super::super::text_measurement_diagnostic::{record_taffy_timing, start_taffy_timing};
use taffy::geometry::Size;
use taffy::prelude::{AvailableSpace, Display, Style, TaffyTree};
use taffy::style_helpers::{auto, length};

const FLOW_ROW_Y_EPSILON: f32 = 0.01;

pub(super) fn flow_stretch_heights(
    tree: &TaffyTree<()>,
    parent: &CssStyle,
    measurements: &[(taffy::tree::NodeId, CssStyle, f32)],
) -> Result<Option<Vec<f32>>, String> {
    if !stretches_children(parent) {
        return Ok(None);
    }
    if parent.display == Display::Flex && is_flex_row(parent) {
        return row_heights(tree, measurements, true).map(Some);
    }
    if parent.display != Display::Grid {
        return Ok(None);
    }
    row_heights(tree, measurements, false).map(Some)
}

fn row_heights(
    tree: &TaffyTree<()>,
    measurements: &[(taffy::tree::NodeId, CssStyle, f32)],
    use_computed_auto_height: bool,
) -> Result<Vec<f32>, String> {
    let items = measurements
        .iter()
        .map(|(node, _, height)| row_item(tree, *node, *height, use_computed_auto_height))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items
        .iter()
        .map(|(row_y, _)| row_height(&items, *row_y))
        .collect())
}

fn row_item(
    tree: &TaffyTree<()>,
    node: taffy::tree::NodeId,
    measured_height: f32,
    use_computed_auto_height: bool,
) -> Result<(f32, f32), String> {
    let layout = tree.layout(node).map_err(layout_error)?;
    let height = if use_computed_auto_height
        && tree
            .style(node)
            .map_err(layout_error)?
            .size
            .height
            .is_auto()
    {
        layout.size.height.max(measured_height)
    } else {
        measured_height
    };
    Ok((layout.location.y, height))
}

fn row_height(items: &[(f32, f32)], row_y: f32) -> f32 {
    items
        .iter()
        .filter(|(candidate_y, _)| (*candidate_y - row_y).abs() <= FLOW_ROW_Y_EPSILON)
        .fold(0.0, |height, (_, candidate)| height.max(*candidate))
}

fn stretches_children(parent: &CssStyle) -> bool {
    use taffy::style::AlignItems;

    let alignment = parent.align_items.unwrap_or(AlignItems::NORMAL);
    alignment == AlignItems::NORMAL || alignment == AlignItems::STRETCH
}

fn is_flex_row(parent: &CssStyle) -> bool {
    matches!(
        parent.flex_direction,
        taffy::style::FlexDirection::Row | taffy::style::FlexDirection::RowReverse
    )
}

pub(super) fn is_visible_layout_item(node: &HtmlDocumentNode, inherited: &CssStyle) -> bool {
    if !is_layout_item(node) {
        return false;
    }
    match node {
        HtmlDocumentNode::Text(_) => true,
        HtmlDocumentNode::Element {
            tag, attributes, ..
        } => CssStyle::from_element(tag, attributes, inherited).display != Display::None,
    }
}

pub(super) fn compute_flow_layout(
    tree: &mut TaffyTree<()>,
    nodes: &[taffy::tree::NodeId],
    style: &CssStyle,
    width: f32,
    height: Option<f32>,
) -> Result<taffy::tree::NodeId, String> {
    let root = tree
        .new_with_children(flow_style(style, width, height), nodes)
        .map_err(layout_error)?;
    compute_root_layout(tree, root, width, height)?;
    Ok(root)
}

pub(super) fn compute_root_layout(
    tree: &mut TaffyTree<()>,
    root: taffy::tree::NodeId,
    width: f32,
    height: Option<f32>,
) -> Result<(), String> {
    let started = start_taffy_timing();
    let result = compute_root_layout_body(tree, root, width, height);
    record_taffy_timing(started);
    result
}

fn compute_root_layout_body(
    tree: &mut TaffyTree<()>,
    root: taffy::tree::NodeId,
    width: f32,
    height: Option<f32>,
) -> Result<(), String> {
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(width),
            height: height.map_or(AvailableSpace::MaxContent, AvailableSpace::Definite),
        },
    )
    .map_err(layout_error)
}

fn flow_style(style: &CssStyle, width: f32, height: Option<f32>) -> Style {
    let mut layout = Style {
        display: style.display,
        size: Size {
            width: length(width),
            height: height.map_or_else(auto, length),
        },
        gap: Size {
            width: length(style.flow_column_gap.unwrap_or(style.gap)),
            height: length(style.row_gap.unwrap_or(style.gap)),
        },
        flex_direction: style.flex_direction,
        flex_wrap: style.flex_wrap,
        align_items: style
            .align_items
            .unwrap_or(taffy::style::AlignItems::NORMAL),
        justify_content: style
            .justify_content
            .unwrap_or(taffy::style::JustifyContent::NORMAL),
        ..Style::default()
    };
    apply_flow_grid_columns(&mut layout, style);
    layout
}

fn apply_flow_grid_columns(layout: &mut Style, style: &CssStyle) {
    if style.display != Display::Grid {
        return;
    }
    layout.grid_template_columns = style
        .grid_template_columns
        .iter()
        .copied()
        .map(taffy_grid_track)
        .map(Into::into)
        .collect();
}

pub(super) fn layout_error(error: impl ToString) -> String {
    format!("CSS flow layout failed: {}", error.to_string())
}

#[cfg(test)]
#[path = "layout_flow_compute_tests.rs"]
mod tests;
