#[cfg(test)]
use super::super::super::super::html_document::HtmlDocumentNode;
use super::super::super::document::TableCell;
use super::super::super::style::CssStyle;
use super::{collapsed_border_width, table_cell_style};

#[path = "layout_table_columns_distribution.rs"]
mod distribution;
use distribution::{
    all_widths_declared, declared_width_values, declared_widths_need_remainder,
    distribute_percentage_guess, enforce_minimum_widths, fill_undersubscribed_declared_widths,
    preferred_widths, resolve_flexible_widths, specified_width,
};

#[path = "layout_table_intrinsic.rs"]
mod intrinsic;
use intrinsic::TableIntrinsic;

pub(crate) struct TableColumnWidths;

impl TableColumnWidths {
    pub(crate) fn resolve(
        rows: &[Vec<TableCell>],
        column_count: usize,
        available: f32,
        inherited: &CssStyle,
    ) -> Vec<f32> {
        let widths = declared_table_column_widths(rows, column_count, available, inherited);
        let minimums = intrinsic_table_column_widths(rows, column_count, inherited);
        if minimums.iter().sum::<f32>() >= available && all_widths_declared(&widths) {
            return declared_width_values(widths);
        }
        if minimums.iter().sum::<f32>() >= available {
            return minimums;
        }
        let preferred = preferred_widths(&widths, &minimums);
        if preferred.iter().sum::<f32>() > available {
            if all_widths_declared(&widths) {
                return distribute_percentage_guess(&widths, &minimums, available);
            }
            return enforce_minimum_widths(preferred, minimums, available);
        }
        if declared_widths_need_remainder(&widths, &preferred, available) {
            return fill_undersubscribed_declared_widths(&widths, &minimums, available);
        }
        let specified = specified_width(&widths, &minimums, available);
        resolve_flexible_widths(widths, minimums, available, specified)
    }
}

fn intrinsic_table_column_widths(
    rows: &[Vec<TableCell>],
    column_count: usize,
    inherited: &CssStyle,
) -> Vec<f32> {
    let mut widths = vec![0.0_f32; column_count];
    for row in rows {
        for (column_index, cell) in row.iter().enumerate().take(column_count) {
            let style = table_cell_style(cell, 0, inherited);
            let node = super::super::super::super::html_document::HtmlDocumentNode::Element {
                node_id: 0,
                tag: cell.tag.clone(),
                attributes: cell.attributes.clone(),
                children: cell.children.clone(),
            };
            let intrinsic = TableIntrinsic::min_content_width(&node, &style);
            let border_adjustment = if style.border_collapse {
                collapsed_border_width(style.border_left_width(), &style)
                    + collapsed_border_width(style.border_right_width(), &style)
            } else {
                0.0
            };
            widths[column_index] =
                widths[column_index].max((intrinsic - border_adjustment).max(0.0));
        }
    }
    widths
}

fn declared_table_column_widths(
    rows: &[Vec<TableCell>],
    column_count: usize,
    available: f32,
    inherited: &CssStyle,
) -> Vec<Option<f32>> {
    let mut widths = vec![None; column_count];
    for (column_index, slot) in widths.iter_mut().enumerate() {
        let Some(cell) = rows.iter().find_map(|row| row.get(column_index)) else {
            continue;
        };
        let cell_style = table_cell_style(cell, 0, inherited);
        let declared_width = cell_style
            .width
            .as_ref()
            .map(|width| width.resolve(available));
        if let Some(width) = declared_width
            && width.is_finite()
            && width > 0.0
        {
            *slot = Some(width);
        }
    }
    widths
}

#[cfg(test)]
mod column_width_tests {
    use super::{
        CssStyle, HtmlDocumentNode, TableCell, TableColumnWidths, distribute_percentage_guess,
    };

    #[path = "layout_table_columns_boundary_tests.rs"]
    mod boundary_tests;

    const TABLE_WIDTH_960: f32 = 606.0;
    const DECLARED_NO_WIDTH: f32 = 36.36;
    const AUTHOR_TABLE_STYLE: &str = "font-family:'Helvetica Neue',Arial,'Hiragino Kaku Gothic ProN',Meiryo,sans-serif;font-size:0.84em;line-height:1.7";

    fn header(text: &str, width: Option<&str>) -> TableCell {
        TableCell {
            tag: "th".to_string(),
            attributes: width.map_or_else(Vec::new, |value| {
                vec![(
                    "style".to_string(),
                    format!("width:{value};padding:8px 12px"),
                )]
            }),
            row_attributes: Vec::new(),
            row_group_tag: None,
            row_group_attributes: Vec::new(),
            element_ids: None,
            children: vec![HtmlDocumentNode::Text(text.to_string())],
        }
    }

    #[test]
    fn declared_percent_column_grows_to_min_content_width() {
        let row = vec![
            header("No.", Some("6%")),
            header("機能名", Some("28%")),
            header("概要・要件", None),
        ];
        let widths = TableColumnWidths::resolve(
            &[row],
            3,
            TABLE_WIDTH_960,
            &CssStyle::from_attributes(
                &[("style".to_string(), AUTHOR_TABLE_STYLE.to_string())],
                &CssStyle::browser_default(),
            ),
        );

        assert!(widths[0] > DECLARED_NO_WIDTH);
        assert!((widths.iter().sum::<f32>() - TABLE_WIDTH_960).abs() < 0.01);
    }

    #[test]
    fn fully_declared_columns_keep_percentages_when_minimums_overflow() {
        const AVAILABLE: f32 = 640.0;
        let row = vec![
            header("項目", Some("22%")),
            header("要件内容", Some("65%")),
            header(&"W".repeat(128), Some("13%")),
        ];
        let inherited = CssStyle::from_attributes(
            &[("style".to_string(), AUTHOR_TABLE_STYLE.to_string())],
            &CssStyle::browser_default(),
        );
        let rows = [row];
        assert!(
            super::intrinsic_table_column_widths(&rows, 3, &inherited)
                .iter()
                .sum::<f32>()
                > AVAILABLE
        );
        let widths = TableColumnWidths::resolve(&rows, 3, AVAILABLE, &inherited);

        assert_eq!(widths, [140.8, 416.0, 83.2]);
    }

    #[test]
    fn fully_declared_columns_fill_unused_table_width() {
        const AVAILABLE: f32 = 594.0;
        const EXPECTED_COLUMN: f32 = 198.0;
        const DECLARED_WIDTHS: [&str; 2] = ["23%", "1px"];
        for declared_width in DECLARED_WIDTHS {
            let row = ["A"; 3]
                .into_iter()
                .map(|label| header(label, Some(declared_width)))
                .collect::<Vec<_>>();
            let widths =
                TableColumnWidths::resolve(&[row], 3, AVAILABLE, &CssStyle::browser_default());

            assert!(
                widths
                    .iter()
                    .all(|width| (*width - EXPECTED_COLUMN).abs() < 0.01)
            );
            assert!((widths.iter().sum::<f32>() - AVAILABLE).abs() < 0.01);
        }
    }

    #[test]
    fn percentage_guess_distributes_space_above_minimums() {
        let widths = distribute_percentage_guess(
            &[Some(100.0), Some(200.0), Some(100.0)],
            &[30.0, 50.0, 80.0],
            200.0,
        );
        assert!((widths.iter().sum::<f32>() - 200.0).abs() < 0.01);
        assert!(
            widths
                .iter()
                .zip([30.0, 50.0, 80.0])
                .all(|(actual, minimum)| *actual >= minimum)
        );
        assert_eq!(widths, [41.666668, 75.0, 83.333336]);
        assert!(widths[1] > widths[0]);
    }

    #[test]
    fn oversized_declared_columns_shrink_above_their_intrinsic_minimums() {
        let rows = [vec![header("a", Some("150px")), header("a", Some("150px"))]];
        let style = CssStyle::browser_default();
        let minimums = super::intrinsic_table_column_widths(&rows, 2, &style);
        assert!(minimums.iter().sum::<f32>() < 200.0);
        let widths = TableColumnWidths::resolve(&rows, 2, 200.0, &style);
        assert_eq!(widths, [100.0, 100.0]);
        assert!(
            widths
                .iter()
                .zip(minimums)
                .all(|(width, minimum)| *width >= minimum)
        );
    }

    #[test]
    fn percentage_guess_clamps_declared_width_below_minimum() {
        let widths = distribute_percentage_guess(&[Some(100.0), Some(20.0)], &[30.0, 40.0], 100.0);
        assert_eq!(widths, [60.0, 40.0]);
    }

    #[test]
    fn mixed_declared_and_intrinsic_columns_fill_available_width() {
        let row = vec![
            header("識別子", Some("20%")),
            header("折り返し可能な説明文を含む列", None),
        ];
        let widths = TableColumnWidths::resolve(
            &[row],
            2,
            400.0,
            &CssStyle::from_attributes(
                &[("style".to_string(), AUTHOR_TABLE_STYLE.to_string())],
                &CssStyle::browser_default(),
            ),
        );
        assert_eq!(widths.len(), 2);
        assert!((widths.iter().sum::<f32>() - 400.0).abs() < 0.01);
        assert!(widths.iter().all(|width| *width > 0.0));
    }

    #[test]
    fn empty_auto_columns_share_available_width() {
        let row = vec![header("", None), header("", None)];
        let widths = TableColumnWidths::resolve(&[row], 2, 200.0, &CssStyle::browser_default());
        assert_eq!(widths, [100.0, 100.0]);
    }

    #[test]
    fn empty_rows_allocate_requested_columns_with_finite_widths() {
        let widths = TableColumnWidths::resolve(&[], 2, 200.0, &CssStyle::browser_default());
        assert_eq!(widths.len(), 2);
        assert!(
            widths
                .iter()
                .all(|width| width.is_finite() && *width >= 0.0)
        );
        assert!((widths.iter().sum::<f32>() - 200.0).abs() < 0.01);
    }

    #[test]
    fn resolve_uses_intrinsic_widths_when_declared_minimums_overflow() {
        let row = vec![header("長い識別子", None), header("別の長い列", None)];
        let widths = TableColumnWidths::resolve(&[row], 2, 10.0, &CssStyle::browser_default());
        assert!(widths.iter().all(|width| *width > 0.0));
    }

    #[test]
    fn resolve_reduces_mixed_preferred_widths_to_available_space() {
        let row = vec![header("a", Some("100px")), header("MMMMM", None)];
        let widths = TableColumnWidths::resolve(&[row], 2, 140.0, &CssStyle::browser_default());
        assert!((widths.iter().sum::<f32>() - 140.0).abs() < 0.01);
        assert!(widths[0] < 100.0);
        assert!(widths[1] > 0.0);
    }

    #[test]
    fn percentage_guess_handles_zero_weight_and_minimum_only_columns() {
        let widths = distribute_percentage_guess(&[None, Some(20.0)], &[0.0, 20.0], 50.0);
        assert_eq!(widths, [0.0, 20.0]);
        let widths = distribute_percentage_guess(&[None, None], &[0.0, 0.0], 50.0);
        assert_eq!(widths, [0.0, 0.0]);
    }

    #[test]
    fn flexible_widths_cover_declared_only_and_intrinsic_auto_columns() {
        let inherited = CssStyle::browser_default();
        let declared = vec![header("", Some("30%")), header("", Some("70%"))];
        let widths = TableColumnWidths::resolve(&[declared], 2, 100.0, &inherited);
        assert!((widths[0] - 30.0).abs() < 0.01);
        assert!((widths[1] - 70.0).abs() < 0.01);
        let auto = vec![header("", None), header("", None)];
        assert_eq!(
            TableColumnWidths::resolve(&[auto], 2, 100.0, &inherited),
            [50.0, 50.0]
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{CssStyle, HtmlDocumentNode, TableIntrinsic};

    fn width(text: &str, white_space: &str) -> f32 {
        let style = CssStyle::from_attributes(
            &[(
                "style".to_string(),
                format!("font-family:Arial;font-size:12px;white-space:{white_space}"),
            )],
            &CssStyle::browser_default(),
        );
        TableIntrinsic::min_content_width(&HtmlDocumentNode::Text(text.to_string()), &style)
    }

    #[test]
    fn table_min_content_uses_css_breaks_for_mixed_text_cases() {
        let japanese = width("日本語本文", "normal");
        let punctuation = width("日本語、本文。", "normal");
        let latin = width("alpha beta", "normal");
        let mixed = width("ABC日本語DEF", "normal");
        let nowrap = width("日本語本文", "nowrap");
        let emoji = width("👩‍💻資料", "normal");
        assert!(japanese > 0.0 && japanese < nowrap);
        assert!(punctuation >= japanese);
        assert!(latin > 0.0 && latin < nowrap);
        assert!(mixed > 0.0 && mixed < nowrap);
        assert!(emoji > 0.0 && emoji <= nowrap);
    }
}
