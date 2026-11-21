use super::super::html_document::HtmlDocumentNode;
use super::constants::MIN_LAYOUT_WIDTH;
use super::document::{TableCell, table_rows};
use super::layout::HtmlLayoutRenderer;
use super::style::CssStyle;
#[cfg(test)]
use super::types::TableCellLayout;

const COLLAPSED_BORDER_SHARE: f32 = 0.5;
const TABLE_BORDER_SPACING: f32 = 2.0;

fn table_border_spacing(style: &CssStyle) -> f32 {
    if style.border_collapse {
        0.0
    } else {
        TABLE_BORDER_SPACING
    }
}

#[path = "layout_table_cell.rs"]
mod cell;
#[path = "layout_table_content.rs"]
mod content;
#[path = "layout_table_geometry.rs"]
mod geometry;
#[path = "layout_table_paint.rs"]
mod paint;
#[cfg(test)]
use geometry::table_cell_height;
use geometry::{
    TableRowLayout, collapsed_border_width, collapsed_outer_border, row_bottom_border,
    table_cell_horizontal_inset, table_cell_style, table_geometry, table_row_cell_layout,
};
pub(super) fn table_cell_border_inset(width: f32, style: &CssStyle) -> f32 {
    paint::table_cell_border_inset(width, style)
}

#[cfg(test)]
pub(super) fn table_cell_paint_bounds(
    layout: &TableCellLayout<'_>,
    style: &CssStyle,
) -> (f32, f32, f32, f32) {
    geometry::table_cell_paint_bounds(layout, style)
}

#[cfg(test)]
pub(super) fn table_cell_top_inset(style: &CssStyle, previous_bottom_border: f32) -> f32 {
    paint::table_cell_top_inset(style, previous_bottom_border)
}

impl HtmlLayoutRenderer {
    pub(super) fn render_table(
        &mut self,
        children: &[HtmlDocumentNode],
        x: f32,
        y: f32,
        width: f32,
        style: &CssStyle,
    ) -> f32 {
        let rows = table_rows(children);
        if rows.is_empty() {
            return y;
        }
        let x = x + style.margin_left;
        let width =
            (style.explicit_width(width).unwrap_or(width) - style.margin_left - style.margin_right)
                .max(MIN_LAYOUT_WIDTH);
        let (left_border, _right_border, column_widths) = table_geometry(&rows, width, style);
        let table_x = x;
        let table_y = y + style.margin_top;
        let background_index = self.svg.len();
        let bottom = self.render_table_rows(
            &rows,
            table_x + left_border + table_border_spacing(style),
            table_y + table_border_spacing(style),
            &column_widths,
            style,
        );
        self.paint_table_background(background_index, table_x, table_y, width, bottom, style);
        bottom + style.margin_bottom
    }

    fn paint_table_background(
        &mut self,
        index: usize,
        x: f32,
        y: f32,
        width: f32,
        bottom: f32,
        style: &CssStyle,
    ) {
        self.capture_stacking_context_children();
        let before_background = self.svg.len();
        self.insert_table_background(index, x, y, width, bottom, style);
        self.record_stacking_background(self.svg.len() - before_background);
    }

    fn render_table_rows(
        &mut self,
        rows: &[Vec<TableCell>],
        x: f32,
        mut current_y: f32,
        column_widths: &[f32],
        style: &CssStyle,
    ) -> f32 {
        current_y += collapsed_outer_border(rows.first(), 0, style, CssStyle::border_top_width);
        let (current_y, previous_bottom_border) =
            self.render_table_row_sequence(rows, x, current_y, column_widths, style);
        current_y
            + collapsed_border_width(previous_bottom_border, style)
            + table_border_spacing(style)
    }

    fn render_table_row_sequence(
        &mut self,
        rows: &[Vec<TableCell>],
        x: f32,
        mut current_y: f32,
        column_widths: &[f32],
        style: &CssStyle,
    ) -> (f32, f32) {
        let mut previous_bottom_border = 0.0;
        for (row_index, row) in rows.iter().enumerate() {
            let (height, bottom_border) = self.render_one_table_row(
                row,
                row_index,
                TableRowLayout {
                    x,
                    y: current_y,
                    column_widths,
                    style,
                    previous_bottom_border,
                },
            );
            current_y += height;
            if row_index + 1 < rows.len() {
                current_y += table_border_spacing(style);
            }
            previous_bottom_border = bottom_border;
        }
        (current_y, previous_bottom_border)
    }

    fn render_one_table_row(
        &mut self,
        row: &[TableCell],
        row_index: usize,
        row_layout: TableRowLayout<'_>,
    ) -> (f32, f32) {
        let measured_row = self.measure_table_content_row_details(
            row,
            row_index,
            row_layout.column_widths,
            row_layout.style,
            row_layout.previous_bottom_border,
        );

        let layout = table_row_cell_layout(&row_layout, row_index, measured_row.height);
        self.render_table_row(&measured_row, layout, row_layout.column_widths);
        (
            measured_row.height,
            row_bottom_border(row, row_index, row_layout.style),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::geometry::TableColumnWidths;
    use super::{CssStyle, TableCell, table_cell_height, table_cell_style};
    use crate::renderer::backends::html_document::HtmlDocumentNode;

    fn region_cell_and_style(declarations: &str) -> (TableCell, CssStyle) {
        let cell = TableCell {
            tag: "td".to_string(),
            attributes: vec![("style".to_string(), declarations.to_string())],
            row_attributes: Vec::new(),
            row_group_tag: None,
            row_group_attributes: Vec::new(),
            element_ids: None,
            children: vec![HtmlDocumentNode::Text("領域".to_string())],
        };
        let mut style = CssStyle::browser_default();
        style.font_size = 14.0;
        style.line_height = 22.4;
        (cell, style)
    }

    #[test]
    fn collapsed_cell_paints_outside_half_borders_and_keeps_half_insets() {
        let mut style = CssStyle::from_attributes(
            &[("style".into(), "border:2px solid black;padding:0".into())],
            &CssStyle::browser_default(),
        );
        style.border_collapse = true;
        let layout = super::TableCellLayout {
            previous_bottom_border: 0.0,
            row_index: 0,
            x: 1.0,
            y: 1.0,
            width: 158.0,
            height: 42.0,
            style: &style,
        };
        assert_eq!(
            super::table_cell_paint_bounds(&layout, &style),
            (0.0, 0.0, 160.0, 44.0)
        );
        assert_eq!(super::table_cell_horizontal_inset(&style), 2.0);
        assert_eq!(
            super::table_cell_border_inset(style.border_top_width(), &style),
            1.0
        );
        style.border_collapse = false;
        assert_eq!(super::table_cell_horizontal_inset(&style), 4.0);
    }

    #[test]
    fn collapsed_cell_text_includes_the_previous_rows_shared_border() {
        let mut style = CssStyle::from_attributes(
            &[(
                "style".into(),
                "border-bottom:1px solid black;padding:0".into(),
            )],
            &CssStyle::browser_default(),
        );
        style.border_collapse = true;
        assert_eq!(super::table_cell_top_inset(&style, 0.0), 0.0);
        assert_eq!(super::table_cell_top_inset(&style, 1.0), 0.5);
        assert_eq!(super::table_cell_top_inset(&style, 3.0), 1.5);
        style.border_collapse = false;
        assert_eq!(super::table_cell_top_inset(&style, 3.0), 0.0);
    }

    #[test]
    fn table_cell_uses_cascade_style_before_browser_fallbacks() {
        let header = styled_header_cell();
        let style = table_cell_style(&header, 0, &CssStyle::browser_default());
        assert_explicit_cell_style(&style);
        let fallback = table_cell_style(&empty_header_cell(), 0, &CssStyle::browser_default());
        assert_eq!(fallback.background.as_deref(), Some("#0b74c7"));
        assert_eq!(fallback.color, "#ffffff");
    }

    fn styled_header_cell() -> TableCell {
        TableCell {
            tag: "th".to_string(),
            attributes: vec![(
                "style".to_string(),
                "background:#dbeafe;color:#173f5f;border:2px solid #112233".to_string(),
            )],
            row_attributes: Vec::new(),
            row_group_tag: None,
            row_group_attributes: Vec::new(),
            element_ids: None,
            children: Vec::new(),
        }
    }

    fn empty_header_cell() -> TableCell {
        TableCell {
            tag: "th".to_string(),
            attributes: Vec::new(),
            row_attributes: Vec::new(),
            row_group_tag: None,
            row_group_attributes: Vec::new(),
            element_ids: None,
            children: Vec::new(),
        }
    }

    fn assert_explicit_cell_style(style: &CssStyle) {
        assert_eq!(style.background.as_deref(), Some("#dbeafe"));
        assert_eq!(style.color, "#173f5f");
        assert_eq!(style.border.as_deref(), Some("#112233"));
        assert_eq!(style.border_width, 2.0);
    }

    #[test]
    fn table_column_widths_respect_declared_percentages_and_fill_remaining_space() {
        let row = vec![
            header_cell_with_width(Some("6%")),
            header_cell_with_width(Some("28%")),
            header_cell_with_width(None),
        ];
        let widths = TableColumnWidths::resolve(&[row], 3, 1_000.0, &CssStyle::browser_default());
        assert_eq!(widths, [60.0, 280.0, 660.0]);
    }

    fn header_cell_with_width(width: Option<&str>) -> TableCell {
        let attributes = width.map_or_else(Vec::new, |width| {
            vec![("style".to_string(), format!("width:{width}"))]
        });
        TableCell {
            tag: "th".to_string(),
            attributes,
            row_attributes: Vec::new(),
            row_group_tag: None,
            row_group_attributes: Vec::new(),
            element_ids: None,
            children: Vec::new(),
        }
    }

    #[test]
    fn table_row_background_is_inherited_by_cells_without_an_override() {
        let row_attributes = vec![(
            "style".to_string(),
            "background:#eef2ff;color:#223355".to_string(),
        )];
        let cell = TableCell {
            tag: "td".to_string(),
            attributes: Vec::new(),
            row_attributes,
            row_group_tag: None,
            row_group_attributes: Vec::new(),
            element_ids: None,
            children: Vec::new(),
        };
        let style = table_cell_style(&cell, 1, &CssStyle::browser_default());
        assert_eq!(style.background.as_deref(), Some("#eef2ff"));
        assert_eq!(style.color, "#223355");
        let table = CssStyle::from_attributes(
            &[("style".to_string(), "background:#ffffff".to_string())],
            &CssStyle::browser_default(),
        );
        assert_eq!(
            table_cell_style(&cell, 1, &table).background.as_deref(),
            Some("#eef2ff")
        );
    }

    #[test]
    fn table_background_inherits_without_synthesizing_user_agent_borders() {
        let cell = TableCell {
            tag: "td".to_string(),
            attributes: Vec::new(),
            row_attributes: Vec::new(),
            row_group_tag: None,
            row_group_attributes: Vec::new(),
            element_ids: None,
            children: Vec::new(),
        };
        let table = CssStyle::from_attributes(
            &[("style".to_string(), "background:#ffffff".to_string())],
            &CssStyle::browser_default(),
        );
        let style = table_cell_style(&cell, 1, &table);
        assert_eq!(style.background.as_deref(), Some("#ffffff"));
        assert_eq!(style.border, None);
        assert_eq!(style.border_width, 0.0);
    }

    #[test]
    fn collapsed_tables_do_not_add_separate_border_spacing() {
        let mut style = CssStyle::browser_default();
        assert_eq!(super::table_border_spacing(&style), 2.0);
        style.border_collapse = true;
        assert_eq!(super::table_border_spacing(&style), 0.0);
    }

    #[test]
    fn table_cell_height_matches_browser_padding_and_line_height() {
        let (cell, style) = region_cell_and_style(
            "font-size:14px;line-height:1.6;padding:12px 10px;border:1px solid #dce5ed",
        );

        assert!((table_cell_height(&cell, 0, 300.0, &style, 0.0) - 48.390625).abs() < 0.001);
    }

    #[test]
    fn table_header_style_preserves_bold_weight_and_cell_attributes() {
        let header = TableCell {
            tag: "th".to_string(),
            attributes: vec![
                (
                    "style".to_string(),
                    "font-weight:700;vertical-align:top".to_string(),
                ),
                ("rowspan".to_string(), "2".to_string()),
                ("colspan".to_string(), "2".to_string()),
            ],
            row_attributes: Vec::new(),
            row_group_tag: None,
            row_group_attributes: Vec::new(),
            element_ids: None,
            children: Vec::new(),
        };
        let style = table_cell_style(&header, 0, &CssStyle::browser_default());

        assert_eq!(style.font_weight, 700);
        assert_eq!(header.attributes[1].1, "2");
        assert_eq!(header.attributes[2].1, "2");
    }

    #[test]
    fn collapsed_table_rows_use_half_shared_borders() {
        let (cell, mut style) = region_cell_and_style(
            "font-size:14px;line-height:1.6;padding:12px 10px;border-bottom:1px solid #dce5ed",
        );
        style.border_collapse = true;

        assert!((table_cell_height(&cell, 0, 300.0, &style, 0.0) - 46.890625).abs() < 0.001);
        assert!((table_cell_height(&cell, 1, 300.0, &style, 1.0) - 47.390625).abs() < 0.001);
    }

    #[test]
    fn collapsed_rows_use_the_stronger_adjacent_border() {
        let (cell, mut style) = region_cell_and_style(
            "font-size:14px;line-height:1.6;padding:12px 10px;border-top:1px solid #dce5ed;border-bottom:1px solid #dce5ed",
        );
        style.border_collapse = true;

        assert!((table_cell_height(&cell, 1, 300.0, &style, 3.0) - 48.390625).abs() < 0.001);
    }

    #[test]
    fn collapsed_table_includes_both_exterior_half_borders() {
        let row = vec![TableCell {
            tag: "td".to_string(),
            attributes: vec![("style".to_string(),
                "line-height:40px;padding:0;border-top:2px solid black;border-bottom:2px solid black".to_string())],
            row_attributes: Vec::new(),
            row_group_tag: None,
            row_group_attributes: Vec::new(),
            element_ids: None,
            children: vec![HtmlDocumentNode::Text("A".to_string())],
        }];
        let mut style = CssStyle::browser_default();
        style.border_collapse = true;
        let top = super::collapsed_outer_border(Some(&row), 0, &style, CssStyle::border_top_width);
        let bottom =
            super::collapsed_outer_border(Some(&row), 0, &style, CssStyle::border_bottom_width);
        assert_eq!(top, 1.0);
        assert_eq!(bottom, 1.0);
        assert_eq!(
            top + table_cell_height(&row[0], 0, 200.0, &style, 0.0) + bottom,
            44.0
        );
        style.border_collapse = false;
        assert_eq!(
            super::collapsed_outer_border(Some(&row), 0, &style, CssStyle::border_top_width),
            0.0
        );
    }
}
