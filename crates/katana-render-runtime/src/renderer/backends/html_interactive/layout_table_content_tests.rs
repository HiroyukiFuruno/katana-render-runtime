use super::super::super::style::CssStyle;
use super::accept_table_content_measurement;
use super::support::*;
use crate::renderer::backends::html_document::HtmlDocumentNode;

#[test]
fn rich_cell_with_narrow_width_keeps_content_measurement_non_negative() {
    let mut renderer = renderer();
    let row = [cell(
        "padding:40px",
        vec![HtmlDocumentNode::Element {
            node_id: 1,
            tag: "span".to_string(),
            attributes: vec![("style".to_string(), "font-size:18px".to_string())],
            children: vec![HtmlDocumentNode::Text("狭い列の内容".to_string())],
        }],
    )];

    let height = renderer.measure_table_content_row(
        &row,
        0,
        &[NARROW_COLUMN_WIDTH],
        &CssStyle::browser_default(),
        ZERO_SCROLL_Y,
    );

    assert!(height.is_finite());
    assert!(height >= 0.0);
}

#[test]
fn rich_cell_preserves_inline_child_font_and_line_metrics() {
    let mut renderer = renderer();
    let row = [cell(
        "padding:4px;line-height:24px",
        vec![HtmlDocumentNode::Element {
            node_id: 2,
            tag: "span".to_string(),
            attributes: vec![(
                "style".to_string(),
                "display:inline-block;font-size:18px;line-height:24px".to_string(),
            )],
            children: vec![HtmlDocumentNode::Text("予約状況".to_string())],
        }],
    )];

    let height = renderer.measure_table_content_row(
        &row,
        0,
        &[RICH_CELL_WIDTH],
        &CssStyle::browser_default(),
        ZERO_SCROLL_Y,
    );

    assert!(height >= 24.0);
    assert!(renderer.layout_error.is_none());
}

#[test]
fn rich_cell_measurement_preserves_inset_and_propagates_errors() {
    let mut layout_error = None;
    assert_eq!(
        accept_table_content_measurement(&mut layout_error, Ok(24.0), CELL_INSET),
        32.0
    );
    assert!(layout_error.is_none());

    assert_eq!(
        accept_table_content_measurement(
            &mut layout_error,
            Err("nested table layout failed".to_string()),
            CELL_INSET,
        ),
        0.0
    );
    assert_eq!(layout_error.as_deref(), Some("nested table layout failed"));
}

#[test]
fn specified_cell_height_sets_row_minimum_without_inflating_content() {
    let row = [cell(
        "height:40px;padding:0;border:0;line-height:20px",
        vec![HtmlDocumentNode::Text("内容".to_string())],
    )];
    let measured = renderer().measure_table_content_row_details(
        &row,
        0,
        &[RICH_CELL_WIDTH],
        &CssStyle::browser_default(),
        ZERO_SCROLL_Y,
    );
    assert_eq!(measured.height, SPECIFIED_CELL_HEIGHT);
    assert_eq!(
        measured.cells[0].measurement.outer_height,
        NATURAL_LINE_HEIGHT
    );
}

#[test]
fn specified_cell_height_does_not_clip_taller_content() {
    let row = [cell(
        "height:10px;padding:0;border:0;line-height:20px",
        vec![HtmlDocumentNode::Text("内容".to_string())],
    )];
    let measured = renderer().measure_table_content_row_details(
        &row,
        0,
        &[RICH_CELL_WIDTH],
        &CssStyle::browser_default(),
        ZERO_SCROLL_Y,
    );
    assert_eq!(measured.height, NATURAL_LINE_HEIGHT);
    assert_eq!(
        measured.cells[0].measurement.outer_height,
        NATURAL_LINE_HEIGHT
    );
}

#[test]
fn specified_cell_height_respects_box_sizing_and_border_model() {
    for (collapsed, sizing, expected) in [
        (false, "content-box", PADDED_SPECIFIED_HEIGHT),
        (false, "border-box", BORDER_BOX_SPECIFIED_HEIGHT),
        (true, "content-box", COLLAPSED_CONTENT_BOX_ROW_HEIGHT),
        (true, "border-box", COLLAPSED_BORDER_BOX_ROW_HEIGHT),
    ] {
        assert_specified_height_with_insets(collapsed, sizing, expected);
    }
}

fn assert_specified_height_with_insets(collapsed: bool, sizing: &str, expected: f32) {
    let row = [cell(
        &format!("height:100px;padding:5px;border:2px solid black;box-sizing:{sizing}"),
        Vec::new(),
    )];
    let mut inherited = CssStyle::browser_default();
    inherited.border_collapse = collapsed;
    let measured = renderer().measure_table_content_row_details(
        &row,
        0,
        &[RICH_CELL_WIDTH],
        &inherited,
        ZERO_SCROLL_Y,
    );
    assert_eq!(
        measured.height, expected,
        "collapsed={collapsed}, sizing={sizing}"
    );
    assert!(measured.cells[0].measurement.outer_height < NATURAL_LINE_HEIGHT);
}
