use super::super::{DrawioJsRuntimeOps, page_crop_test_support::temp_runtime_path};
use super::page_crop_bundle_fixtures::*;
use super::page_crop_geometry::{
    SvgBox, normal_crop_is_smaller_than_special, render_sketch_bundle_with_normal_crop_control,
    sketch_stack_layout_crop_matches, svg_canvas_box, svg_canvas_contains_cells, svg_cell_rect_box,
    svg_cell_stroked_path_box,
};
use super::page_crop_source_fixtures::{
    non_stack_outside_child_source, second_sketch_board_source, sketch_normal_crop_source,
    sketch_stack_layout_actual_shape_source, sketch_stack_layout_outside_column_source,
    sketch_stack_layout_rough_fill_source, sketch_stack_layout_source, top_level_outside_source,
};
use crate::markdown::color_preset::DiagramColorPreset;

#[test]
fn fake_bundle_crops_sketch_stack_layout_from_board_paint_bounds() {
    let path = temp_runtime_path("krr-drawio-sketch-stack-layout-crop-unit");
    assert!(std::fs::write(&path, sketch_stack_layout_bundle()).is_ok());

    let source = sketch_stack_layout_source().to_owned();
    for stroke in [1, 2, 3, 4] {
        let source = source.replace("strokeWidth=2", &format!("strokeWidth={stroke}"));
        let width = 540 + stroke + 2;
        let height = 440 + stroke + 2;
        let origin = stroke / 2;
        let rendered = DrawioJsRuntimeOps::render(&source, &path, DiagramColorPreset::dark());

        assert!(
            rendered
                .as_ref()
                .is_ok_and(|svg| sketch_stack_layout_crop_matches(svg, width, height, origin)),
            "stroke={stroke}: {rendered:?}"
        );
    }
}

#[test]
fn fake_bundle_crops_sketch_stack_layout_with_transparent_header_path_and_rough_board_stroke() {
    let path = temp_runtime_path("krr-drawio-sketch-transparent-header-path-unit");
    assert!(std::fs::write(&path, sketch_stack_layout_actual_shape_bundle()).is_ok());

    let rendered = DrawioJsRuntimeOps::render(
        sketch_stack_layout_actual_shape_source(),
        &path,
        DiagramColorPreset::dark(),
    );

    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            let Some(canvas) = svg_canvas_box(svg) else {
                return false;
            };
            let Some(board_paint) = svg_cell_stroked_path_box(svg, "board") else {
                return false;
            };
            canvas == SvgBox::new(0, 0, 544, 444)
                && board_paint == SvgBox::new(0, 1, 542, 440)
                && svg.contains(r#"d="M 2 1 H 542 V 29 H 2 Z""#)
                && svg.contains(r#"transform="translate(-1,0)""#)
                && svg_canvas_contains_cells(svg, ["column-a", "column-b", "column-c"])
        }),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_crops_sketch_stack_layout_with_finite_board_and_column_rough_fill_overhang() {
    let path = temp_runtime_path("krr-drawio-sketch-rough-fill-overhang-unit");
    assert!(std::fs::write(&path, sketch_stack_layout_rough_fill_bundle("0.78", "0.96")).is_ok());

    let rendered = DrawioJsRuntimeOps::render(
        &sketch_stack_layout_rough_fill_source(),
        &path,
        DiagramColorPreset::dark(),
    );

    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            svg_canvas_box(svg) == Some(SvgBox::new(0, 0, 543, 443))
                && svg.contains(r#"transform="translate(-1,-1)""#)
                && svg_canvas_contains_cells(svg, ["column-a", "column-b", "column-c"])
        }),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_preserves_rough_fill_that_exceeds_the_finite_sketch_overhang_guard() {
    let bundle = sketch_stack_layout_rough_fill_bundle("0.78", "-1.01");
    let source = sketch_stack_layout_rough_fill_source();
    let (rendered, control) = render_sketch_bundle_with_normal_crop_control(
        &source,
        &bundle,
        &source,
        &sketch_stack_layout_normal_crop_bundle(&bundle),
        "krr-drawio-sketch-rough-fill-overflow-unit",
    );

    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            let Some(canvas) = svg_canvas_box(svg) else {
                return false;
            };
            control
                .as_ref()
                .ok()
                .and_then(|svg| svg_canvas_box(svg))
                .is_some_and(|normal_canvas| {
                    canvas == normal_canvas
                        && canvas != SvgBox::new(0, 0, 543, 443)
                        && svg_canvas_contains_cells(svg, ["column-a", "column-b", "column-c"])
                })
        }),
        "rendered={rendered:?}; control={control:?}"
    );
}

#[test]
fn fake_bundle_preserves_outside_stack_child_instead_of_treating_it_as_rough_fill() {
    let bundle = sketch_stack_layout_bundle_with_outside_column();
    let source = sketch_stack_layout_outside_column_source();
    let (rendered, control) = render_sketch_bundle_with_normal_crop_control(
        &source,
        &bundle,
        &source,
        &sketch_stack_layout_normal_crop_bundle(&bundle),
        "krr-drawio-sketch-outside-stack-child-unit",
    );

    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            let Some(canvas) = svg_canvas_box(svg) else {
                return false;
            };
            control
                .as_ref()
                .ok()
                .and_then(|svg| svg_canvas_box(svg))
                .is_some_and(|normal_canvas| {
                    canvas == normal_canvas
                        && canvas.right() > 543.0
                        && svg.contains(r#"data-cell-id="outside-column""#)
                })
        }),
        "rendered={rendered:?}; control={control:?}"
    );
}

#[test]
fn sketch_stack_layout_source_matches_special_crop_predicate() {
    let source = sketch_stack_layout_source();
    assert!(source.contains(r#"<mxfile type="device""#));
    assert!(source.contains(r#"<mxGraphModel page="1""#));
    assert!(source.contains("swimlane;childLayout=stackLayout;horizontalStack=1;sketch=1"));
    assert_eq!(
        source
            .matches("childLayout=stackLayout;horizontalStack=0;sketch=1")
            .count(),
        3
    );
}

#[test]
fn fake_bundle_does_not_apply_sketch_stack_layout_crop_without_stack_layout() {
    let path = temp_runtime_path("krr-drawio-sketch-non-stack-crop-unit");
    assert!(std::fs::write(&path, sketch_stack_layout_bundle()).is_ok());

    let special = DrawioJsRuntimeOps::render(
        sketch_stack_layout_source(),
        &path,
        DiagramColorPreset::dark(),
    );
    let rendered = DrawioJsRuntimeOps::render(
        &sketch_normal_crop_source(),
        &path,
        DiagramColorPreset::dark(),
    );

    let special_canvas = special.as_ref().ok().and_then(|svg| svg_canvas_box(svg));
    assert!(
        rendered
            .as_ref()
            .is_ok_and(|svg| normal_crop_is_smaller_than_special(svg, special_canvas)),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_keeps_top_level_shape_outside_sketch_board_in_normal_crop() {
    let (rendered, control) = render_sketch_bundle_with_normal_crop_control(
        &top_level_outside_source(),
        &sketch_bundle_with_top_level_shape(),
        &sketch_normal_crop_source(),
        sketch_stack_layout_bundle(),
        "krr-drawio-sketch-top-level-shape-boundary-unit",
    );
    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            let Some(canvas) = svg_canvas_box(svg) else {
                return false;
            };
            let Some(control_canvas) = control.as_ref().ok().and_then(|svg| svg_canvas_box(svg))
            else {
                return false;
            };
            let Some(outside) = svg_cell_rect_box(svg, "outside") else {
                return false;
            };
            svg_canvas_contains_cells(
                svg,
                ["board", "column-a", "column-b", "column-c", "outside"],
            ) && canvas.contains(outside)
                && canvas.right() > control_canvas.right()
        }),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_keeps_second_sketch_board_in_normal_crop() {
    let (rendered, control) = render_sketch_bundle_with_normal_crop_control(
        &second_sketch_board_source(),
        &sketch_bundle_with_second_board(),
        &sketch_normal_crop_source(),
        sketch_stack_layout_bundle(),
        "krr-drawio-sketch-multiple-board-boundary-unit",
    );
    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            let Some(canvas) = svg_canvas_box(svg) else {
                return false;
            };
            let Some(control_canvas) = control.as_ref().ok().and_then(|svg| svg_canvas_box(svg))
            else {
                return false;
            };
            let Some(second_board) = svg_cell_rect_box(svg, "board-2") else {
                return false;
            };
            svg_canvas_contains_cells(
                svg,
                ["board", "column-a", "column-b", "column-c", "board-2"],
            ) && canvas.contains(second_board)
                && canvas.right() > control_canvas.right()
                && canvas.bottom() > control_canvas.bottom()
        }),
        "{rendered:?}"
    );
}

#[test]
fn fake_bundle_keeps_non_stack_child_in_normal_crop() {
    let (rendered, control) = render_sketch_bundle_with_normal_crop_control(
        &non_stack_outside_child_source(),
        &sketch_bundle_with_outside_child(),
        &sketch_normal_crop_source(),
        sketch_stack_layout_bundle(),
        "krr-drawio-sketch-non-stack-child-boundary-unit",
    );
    assert!(
        rendered.as_ref().is_ok_and(|svg| {
            let Some(canvas) = svg_canvas_box(svg) else {
                return false;
            };
            let Some(control_canvas) = control.as_ref().ok().and_then(|svg| svg_canvas_box(svg))
            else {
                return false;
            };
            let Some(outside_child) = svg_cell_rect_box(svg, "outside-child") else {
                return false;
            };
            svg_canvas_contains_cells(
                svg,
                ["board", "column-a", "column-b", "column-c", "outside-child"],
            ) && canvas.contains(outside_child)
                && canvas.right() > control_canvas.right()
                && canvas.bottom() > control_canvas.bottom()
        }),
        "{rendered:?}"
    );
}
