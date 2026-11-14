use super::super::document::TableCell;
use super::super::layout::HtmlLayoutRenderer;
use super::super::style::CssStyle;
use super::super::types::{DetailsContext, TableCellLayout};
use super::cell::table_cell_styles;
#[path = "layout_table_paint_geometry.rs"]
mod paint_geometry;
use super::content::{MeasuredTableCell, MeasuredTableRow};
use super::geometry::table_cell_paint_bounds;
use super::{COLLAPSED_BORDER_SHARE, table_border_spacing};

impl HtmlLayoutRenderer {
    pub(super) fn render_table_row(
        &mut self,
        measured_row: &MeasuredTableRow<'_>,
        layout: TableCellLayout<'_>,
        column_widths: &[f32],
    ) {
        for (column_index, measured_cell) in measured_row.cells.iter().enumerate() {
            self.render_table_row_cell(measured_cell, column_index, layout, column_widths);
        }
    }

    fn render_table_row_cell(
        &mut self,
        measured_cell: &MeasuredTableCell<'_>,
        column_index: usize,
        layout: TableCellLayout<'_>,
        column_widths: &[f32],
    ) {
        let (group_style, row_style, style) =
            table_cell_styles(measured_cell.cell, layout.row_index, layout.style);
        let content_offset = paint_geometry::table_cell_content_offset(
            style.vertical_align,
            layout.height,
            measured_cell.measurement.outer_height,
        );
        let cell_layout = table_row_cell_layout(layout, column_index, column_widths);
        self.render_table_cell(
            measured_cell.cell,
            cell_layout,
            content_offset,
            group_style.as_ref(),
            &row_style,
            &style,
        );
    }

    fn render_table_cell(
        &mut self,
        cell: &TableCell,
        layout: TableCellLayout<'_>,
        content_offset: f32,
        group_style: Option<&CssStyle>,
        row_style: &CssStyle,
        style: &CssStyle,
    ) {
        let bounds = paint_table_cell(self, &layout, style);
        self.record_table_element_boxes(cell, group_style, row_style, style, bounds);
        render_table_cell_content(self, cell, &layout, content_offset, style);
    }

    fn record_table_element_boxes(
        &mut self,
        cell: &TableCell,
        group_style: Option<&CssStyle>,
        row_style: &CssStyle,
        cell_style: &CssStyle,
        bounds: (f32, f32, f32, f32),
    ) {
        let Some(ids) = cell.element_ids else {
            return;
        };
        if let (Some(node_id), Some(style)) = (ids.row_group, group_style) {
            self.record_table_element_box(node_id, style, bounds, true);
        }
        self.record_table_element_box(ids.row, row_style, bounds, true);
        self.record_table_element_box(ids.cell, cell_style, bounds, false);
    }

    fn table_element_box_index(
        &mut self,
        existing: Option<usize>,
        node_id: u64,
        style: &CssStyle,
    ) -> usize {
        if let Some(index) = existing {
            return index;
        }
        let position = style.position;
        self.start_element_box(
            node_id,
            self.element_positioning_context(position),
            !matches!(
                position,
                super::super::style::CssPosition::Absolute
                    | super::super::style::CssPosition::Fixed
            ),
            self.positioning_origin_node_id(node_id, position),
        )
    }

    fn record_table_element_box(
        &mut self,
        node_id: u64,
        style: &CssStyle,
        bounds: (f32, f32, f32, f32),
        union: bool,
    ) {
        let existing = self.ownership.element_box_indices.get(&node_id).copied();
        let index = self.table_element_box_index(existing, node_id, style);
        let clip_axes = self.element_overflow_clip_axes(node_id, style);
        paint_geometry::update_table_element_box(
            &mut self.element_boxes[index],
            node_id,
            style,
            bounds,
            union && existing.is_some(),
            clip_axes,
        );
    }
}

fn table_row_cell_layout<'a>(
    layout: TableCellLayout<'a>,
    column_index: usize,
    column_widths: &[f32],
) -> TableCellLayout<'a> {
    let width = column_widths
        .get(column_index)
        .copied()
        .unwrap_or(layout.width);
    let x = layout.x
        + column_widths[..column_index].iter().sum::<f32>()
        + column_index as f32 * table_border_spacing(layout.style);
    TableCellLayout { x, width, ..layout }
}

fn paint_table_cell(
    renderer: &mut HtmlLayoutRenderer,
    layout: &TableCellLayout<'_>,
    style: &CssStyle,
) -> (f32, f32, f32, f32) {
    let bounds = table_cell_paint_bounds(layout, style);
    renderer.paint_box(bounds.0, bounds.1, bounds.2, bounds.3, style);
    bounds
}

fn render_table_cell_content(
    renderer: &mut HtmlLayoutRenderer,
    cell: &TableCell,
    layout: &TableCellLayout<'_>,
    content_offset: f32,
    style: &CssStyle,
) {
    let horizontal_inset = super::table_cell_horizontal_inset(style);
    let x =
        layout.x + table_cell_border_inset(style.border_left_width(), style) + style.padding_left;
    let y = layout.y
        + table_cell_top_inset(style, layout.previous_bottom_border)
        + style.padding_top
        + content_offset;
    renderer.render_nodes(
        &cell.children,
        x,
        y,
        (layout.width - horizontal_inset).max(0.0),
        style,
        DetailsContext::NONE,
    );
}

#[cfg(test)]
#[path = "layout_table_paint_tests.rs"]
mod tests;

pub(super) fn table_cell_border_inset(width: f32, style: &CssStyle) -> f32 {
    if style.border_collapse {
        width * COLLAPSED_BORDER_SHARE
    } else {
        width
    }
}

pub(super) fn table_cell_top_inset(style: &CssStyle, previous_bottom_border: f32) -> f32 {
    let width = if style.border_collapse {
        style.border_top_width().max(previous_bottom_border)
    } else {
        style.border_top_width()
    };
    table_cell_border_inset(width, style)
}
