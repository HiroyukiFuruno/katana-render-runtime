use super::super::super::html_browser::HtmlBrowserViewport;
use super::super::super::html_document::HtmlDocumentNode;
use super::super::document::TableCell;
use super::super::layout::HtmlLayoutRenderer;
use super::super::style::CssStyle;
use super::super::types::DetailsContext;
use super::cell::{table_cell_height, table_cell_vertical_inset};
use super::{table_cell_horizontal_inset, table_cell_style};
use std::rc::Rc;

pub(super) struct TableCellMeasurement {
    pub(super) outer_height: f32,
    minimum_row_height: f32,
}

pub(super) struct MeasuredTableCell<'a> {
    pub(super) cell: &'a TableCell,
    pub(super) measurement: TableCellMeasurement,
}

pub(super) struct MeasuredTableRow<'a> {
    pub(super) height: f32,
    pub(super) cells: Vec<MeasuredTableCell<'a>>,
}

impl HtmlLayoutRenderer {
    #[cfg(test)]
    pub(super) fn measure_table_content_row(
        &mut self,
        row: &[TableCell],
        row_index: usize,
        widths: &[f32],
        inherited: &CssStyle,
        previous_border: f32,
    ) -> f32 {
        self.measure_table_content_row_details(row, row_index, widths, inherited, previous_border)
            .height
    }

    pub(super) fn insert_table_background(
        &mut self,
        index: usize,
        x: f32,
        y: f32,
        width: f32,
        bottom: f32,
        style: &CssStyle,
    ) {
        self.insert_box(index, x, y, width, (bottom - y).max(0.0), style);
    }

    pub(super) fn measure_table_content_row_details<'a>(
        &mut self,
        row: &'a [TableCell],
        row_index: usize,
        widths: &[f32],
        inherited: &CssStyle,
        previous_border: f32,
    ) -> MeasuredTableRow<'a> {
        let cells = self.measure_table_cells(row, row_index, widths, inherited, previous_border);
        let height = cells
            .iter()
            .map(|measured| {
                measured
                    .measurement
                    .outer_height
                    .max(measured.measurement.minimum_row_height)
            })
            .fold(0.0, f32::max);
        MeasuredTableRow { height, cells }
    }

    fn measure_table_cells<'a>(
        &mut self,
        row: &'a [TableCell],
        row_index: usize,
        widths: &[f32],
        inherited: &CssStyle,
        previous_border: f32,
    ) -> Vec<MeasuredTableCell<'a>> {
        row.iter()
            .enumerate()
            .map(|(column, cell)| {
                let width = widths.get(column).copied().unwrap_or(0.0);
                MeasuredTableCell {
                    cell,
                    measurement: self.measure_one_table_cell(
                        cell,
                        row_index,
                        width,
                        inherited,
                        previous_border,
                    ),
                }
            })
            .collect()
    }

    fn measure_one_table_cell(
        &mut self,
        cell: &TableCell,
        row_index: usize,
        width: f32,
        inherited: &CssStyle,
        previous_border: f32,
    ) -> TableCellMeasurement {
        let style = table_cell_style(cell, row_index, inherited);
        let inset = table_cell_vertical_inset(&style, row_index, previous_border);
        let outer_height = if cell
            .children
            .iter()
            .all(|node| matches!(node, HtmlDocumentNode::Text(_)))
        {
            table_cell_height(cell, row_index, width, inherited, previous_border)
        } else {
            let content_width = (width - table_cell_horizontal_inset(&style)).max(0.0);
            let measured = self.measure_table_cell_content(cell, content_width, &style);
            accept_table_content_measurement(&mut self.layout_error, measured, inset)
        };
        /* WHY: 指定高さを内容高へ直して共有境界を足し、縦揃え用の自然内容高は保つ。 */
        TableCellMeasurement {
            outer_height,
            minimum_row_height: style.children_height().map_or(0.0, |height| height + inset),
        }
    }

    fn measure_table_cell_content(
        &self,
        cell: &TableCell,
        width: f32,
        style: &CssStyle,
    ) -> Result<f32, String> {
        let viewport = HtmlBrowserViewport {
            width: self.viewport_width.ceil().max(1.0) as u32,
            height: self.viewport_height.ceil().max(1.0) as u32,
            device_scale_factor: 1.0,
        };
        let mut renderer = Self::new_with_clickable_nodes(
            viewport,
            0.0,
            &self.input_values,
            self.focused_input,
            &self.clickable_nodes,
            Rc::clone(&self.flow_measurements),
        );
        let height =
            renderer.render_nodes(&cell.children, 0.0, 0.0, width, style, DetailsContext::NONE);
        renderer.ensure_layout_succeeded()?;
        Ok(height)
    }
}

fn accept_table_content_measurement(
    layout_error: &mut Option<String>,
    measured: Result<f32, String>,
    inset: f32,
) -> f32 {
    match measured {
        Ok(height) => height + inset,
        Err(error) => {
            layout_error.get_or_insert(error);
            0.0
        }
    }
}

#[cfg(test)]
#[path = "layout_table_bounds_tests.rs"]
mod bounds_tests;
#[cfg(test)]
#[path = "layout_table_content_test_support.rs"]
mod support;
#[cfg(test)]
#[path = "layout_table_content_tests.rs"]
mod tests;
