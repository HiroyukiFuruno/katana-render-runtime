use super::page_crop_geometry::{
    SvgBox, normal_crop_is_smaller_than_special, sketch_stack_layout_crop_matches, svg_canvas_box,
    svg_canvas_contains_cells, svg_cell_rect_box, svg_cell_stroked_path_box, svg_group_translate,
};

#[test]
fn svg_canvas_rejects_missing_and_wrong_arity_view_boxes() {
    assert_eq!(svg_canvas_box("<svg></svg>"), None);
    assert_eq!(svg_canvas_box(r#"<svg viewBox="0 0 8"></svg>"#), None);
}

#[test]
fn svg_canvas_rejects_missing_tag_delimiter_and_non_numeric_view_box() {
    assert_eq!(svg_canvas_box("<root></root>"), None);
    assert_eq!(svg_canvas_box("<svg"), None);
    assert_eq!(svg_canvas_box(r#"<svg viewBox="0 nope 1 1"></svg>"#), None);
}

#[test]
fn svg_cell_parsers_reject_unsupported_paths_and_unterminated_attributes() {
    let invalid_path =
        r##"<svg><g data-cell-id="board"><path stroke="#000" d="M 0 0 L 1 1"></path></g></svg>"##;
    let unterminated_attribute = r#"<svg><g data-cell-id="cell"><rect x="1></rect></g></svg>"#;

    assert_eq!(svg_cell_stroked_path_box(invalid_path, "board"), None);
    assert_eq!(svg_cell_rect_box(unterminated_attribute, "cell"), None);
}

#[test]
fn svg_cell_rect_applies_valid_translation_and_uses_identity_for_malformed_values() {
    let translated = r#"<svg><g data-cell-id="cell"><rect transform="translate(2, 3)" x="1" y="4" width="5" height="6"></rect></g></svg>"#;
    let bad_number = r#"<svg><g data-cell-id="cell"><rect transform="translate(nope, 3)" x="1" y="4" width="5" height="6"></rect></g></svg>"#;
    let one_value = r#"<svg><g data-cell-id="cell"><rect transform="translate(2)" x="1" y="4" width="5" height="6"></rect></g></svg>"#;

    assert_eq!(
        svg_cell_rect_box(translated, "cell"),
        Some(SvgBox::new(3, 7, 5, 6))
    );
    assert_eq!(
        svg_cell_rect_box(bad_number, "cell"),
        Some(SvgBox::new(1, 4, 5, 6))
    );
    assert_eq!(
        svg_cell_rect_box(one_value, "cell"),
        Some(SvgBox::new(1, 4, 5, 6))
    );
    assert_eq!(svg_group_translate("<rect>"), (0.0, 0.0));
}

#[test]
fn svg_cell_rect_rejects_missing_structure_and_non_numeric_coordinates() {
    let missing_group_close =
        r#"<svg><g data-cell-id="cell"><rect x="1" y="2" width="3" height="4">"#;
    let missing_rect = r#"<svg><g data-cell-id="cell"></g></svg>"#;
    let missing_rect_delimiter =
        r#"<svg><g data-cell-id="cell"><rect x="1" y="2" width="3" height="4"</g></svg>"#;
    let non_numeric_coordinates = [
        r#"<svg><g data-cell-id="cell"><rect x="1" y="nope" width="3" height="4"></rect></g></svg>"#,
        r#"<svg><g data-cell-id="cell"><rect x="1" y="2" width="nope" height="4"></rect></g></svg>"#,
        r#"<svg><g data-cell-id="cell"><rect x="1" y="2" width="3" height="nope"></rect></g></svg>"#,
    ];

    assert_eq!(svg_cell_rect_box(missing_group_close, "cell"), None);
    assert_eq!(svg_cell_rect_box(missing_rect, "cell"), None);
    assert_eq!(svg_cell_rect_box(missing_rect_delimiter, "cell"), None);
    for svg in non_numeric_coordinates {
        assert_eq!(svg_cell_rect_box(svg, "cell"), None);
    }
}

#[test]
fn svg_cell_stroked_path_rejects_missing_and_non_numeric_paint_data() {
    let missing_group_close =
        r#"<svg><g data-cell-id="board"><path stroke="black" d="M 1 2 H 3 V 4">"#;
    let no_paint = r#"<svg><g data-cell-id="board"></g></svg>"#;
    let missing_path_group = "<svg></svg>";
    let none_stroke =
        r#"<svg><g data-cell-id="board"><path stroke="none" d="M 1 2 H 3 V 4"></path></g></svg>"#;
    let invalid_coordinates = [
        r#"<svg><g data-cell-id="board"><path stroke="black" d="M nope 2 H 3 V 4"></path></g></svg>"#,
        r#"<svg><g data-cell-id="board"><path stroke="black" d="M 1 nope H 3 V 4"></path></g></svg>"#,
        r#"<svg><g data-cell-id="board"><path stroke="black" d="M 1 2 H nope V 4"></path></g></svg>"#,
        r#"<svg><g data-cell-id="board"><path stroke="black" d="M 1 2 H 3 V nope"></path></g></svg>"#,
    ];

    assert_eq!(
        svg_cell_stroked_path_box(missing_group_close, "board"),
        None
    );
    assert_eq!(svg_cell_stroked_path_box(no_paint, "board"), None);
    assert_eq!(svg_cell_stroked_path_box(missing_path_group, "board"), None);
    assert_eq!(svg_cell_stroked_path_box(none_stroke, "board"), None);
    for svg in invalid_coordinates {
        assert_eq!(svg_cell_stroked_path_box(svg, "board"), None);
    }
}

#[test]
fn svg_cell_stroked_path_skips_malformed_and_none_candidates_for_valid_paint() {
    let mixed_candidates = r#"<svg><g data-cell-id="board"><path stroke="none" d="M 0 0 H 1 V 1"></path><path stroke="black" d="M 0 0 H 1 V 1"<path stroke="black" d="M 1 2 H 5 V 6"></path></g></svg>"#;

    assert_eq!(
        svg_cell_stroked_path_box(mixed_candidates, "board"),
        Some(SvgBox::new(1, 2, 4, 4))
    );
}

#[test]
fn sketch_crop_and_canvas_helpers_reject_missing_canvas_or_board() {
    let canvas_without_board = r#"<svg viewBox="0 0 1 1"></svg>"#;
    let canvas_with_one_board = r#"<svg viewBox="0 0 10 10"><g data-cell-id="board"><rect x="1" y="1" width="2" height="2"></rect></g></svg>"#;

    assert!(!svg_canvas_contains_cells("<svg></svg>", ["board"]));
    assert!(svg_canvas_contains_cells(canvas_with_one_board, ["board"]));
    assert!(!sketch_stack_layout_crop_matches("<svg></svg>", 1, 1, 0));
    assert!(!sketch_stack_layout_crop_matches(
        canvas_without_board,
        1,
        1,
        0
    ));
}

#[test]
fn normal_crop_comparison_rejects_missing_canvas_and_baseline() {
    assert!(!normal_crop_is_smaller_than_special(
        "<svg></svg>",
        Some(SvgBox::new(0, 0, 2, 2))
    ));
    assert!(!normal_crop_is_smaller_than_special(
        r#"<svg viewBox="0 0 1 1"></svg>"#,
        None,
    ));
}
