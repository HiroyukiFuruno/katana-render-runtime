use super::super::constants::MIN_LAYOUT_WIDTH;
use super::super::types::TableCellLayout;
use super::COLLAPSED_BORDER_SHARE;
#[cfg(test)]
pub(super) use super::cell::table_cell_height;
pub(super) use super::cell::{table_cell_horizontal_inset, table_cell_style};
use super::{CssStyle, TableCell};

#[path = "layout_table_columns.rs"]
mod columns;
pub(super) use columns::TableColumnWidths;

pub(super) struct TableRowLayout<'a> {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) column_widths: &'a [f32],
    pub(super) style: &'a CssStyle,
    pub(super) previous_bottom_border: f32,
}

pub(super) fn table_row_cell_layout<'a>(
    row_layout: &'a TableRowLayout<'a>,
    row_index: usize,
    height: f32,
) -> TableCellLayout<'a> {
    TableCellLayout {
        previous_bottom_border: row_layout.previous_bottom_border,
        row_index,
        x: row_layout.x,
        y: row_layout.y,
        width: row_layout
            .column_widths
            .first()
            .copied()
            .unwrap_or(MIN_LAYOUT_WIDTH),
        height,
        style: row_layout.style,
    }
}

pub(super) fn collapsed_border_width(width: f32, style: &CssStyle) -> f32 {
    if style.border_collapse {
        width * COLLAPSED_BORDER_SHARE
    } else {
        0.0
    }
}

pub(super) fn table_horizontal_edges(rows: &[Vec<TableCell>], style: &CssStyle) -> (f32, f32) {
    let left = rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            row.first()
                .map(|cell| table_cell_style(cell, index, style).border_left_width())
        })
        .fold(0.0_f32, f32::max);
    let right = rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            row.last()
                .map(|cell| table_cell_style(cell, index, style).border_right_width())
        })
        .fold(0.0_f32, f32::max);
    (
        collapsed_border_width(left, style),
        collapsed_border_width(right, style),
    )
}

pub(super) fn table_geometry(
    rows: &[Vec<TableCell>],
    width: f32,
    style: &CssStyle,
) -> (f32, f32, Vec<f32>) {
    let (left, right) = table_horizontal_edges(rows, style);
    let columns = rows.iter().map(Vec::len).max().unwrap_or(1).max(1);
    let available =
        (width - left - right - super::table_border_spacing(style) * (columns + 1) as f32)
            .max(super::MIN_LAYOUT_WIDTH);
    (
        left,
        right,
        TableColumnWidths::resolve(rows, columns, available, style),
    )
}

pub(super) fn table_cell_paint_bounds(
    layout: &TableCellLayout<'_>,
    style: &CssStyle,
) -> (f32, f32, f32, f32) {
    let left = collapsed_border_width(style.border_left_width(), style);
    let right = collapsed_border_width(style.border_right_width(), style);
    let top = collapsed_border_width(style.border_top_width(), style);
    let bottom = collapsed_border_width(style.border_bottom_width(), style);
    (
        layout.x - left,
        layout.y - top,
        layout.width + left + right,
        layout.height + top + bottom,
    )
}

pub(super) fn collapsed_outer_border(
    row: Option<&Vec<TableCell>>,
    row_index: usize,
    style: &CssStyle,
    border_width: fn(&CssStyle) -> f32,
) -> f32 {
    if !style.border_collapse {
        return 0.0;
    }
    row.map_or(0.0, |cells| {
        cells
            .iter()
            .map(|cell| border_width(&table_cell_style(cell, row_index, style)))
            .fold(0.0_f32, f32::max)
            * COLLAPSED_BORDER_SHARE
    })
}

pub(super) fn row_bottom_border(row: &[TableCell], row_index: usize, style: &CssStyle) -> f32 {
    row.iter()
        .map(|cell| table_cell_style(cell, row_index, style).border_bottom_width())
        .fold(0.0, f32::max)
}
