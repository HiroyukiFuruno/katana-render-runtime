pub(super) fn sketch_source_with_extra_cell(cell: &str) -> String {
    sketch_stack_layout_source().replace("</root>", &format!("{cell}\n</root>"))
}

pub(super) fn top_level_outside_source() -> String {
    sketch_source_with_extra_cell(
        r#"<mxCell id="outside" style="shape=rect;strokeWidth=2;" vertex="1" parent="1">
  <mxGeometry x="600" y="20" width="30" height="30" as="geometry"/>
</mxCell>"#,
    )
}

pub(super) fn second_sketch_board_source() -> String {
    sketch_source_with_extra_cell(
        r#"<mxCell id="board-2" style="swimlane;childLayout=flowLayout;sketch=1;strokeWidth=2;" vertex="1" parent="1">
  <mxGeometry x="600" y="550" width="100" height="50" as="geometry"/>
</mxCell>"#,
    )
}

pub(super) fn non_stack_outside_child_source() -> String {
    sketch_source_with_extra_cell(
        r#"<mxCell id="outside-child" style="shape=rect;childLayout=flowLayout;sketch=1;strokeWidth=2;" vertex="1" parent="board">
  <mxGeometry x="600" y="550" width="30" height="30" as="geometry"/>
</mxCell>"#,
    )
}

pub(super) fn sketch_normal_crop_source() -> String {
    sketch_stack_layout_source().replace(
        "childLayout=stackLayout;horizontalStack=1",
        "childLayout=stackLayout;horizontalStack=0",
    )
}

pub(super) fn sketch_stack_layout_source() -> &'static str {
    r#"<mxfile type="device"><diagram><mxGraphModel page="1" background="none"><root>
<mxCell id="1" parent="0"/>
<mxCell id="board" style="swimlane;childLayout=stackLayout;horizontalStack=1;sketch=1;strokeWidth=2;" vertex="1" parent="1">
  <mxGeometry x="2" y="2" width="540" height="440" as="geometry"/>
</mxCell>
<mxCell id="column-a" style="swimlane;childLayout=stackLayout;horizontalStack=0;sketch=1;strokeWidth=1;" vertex="1" parent="board">
  <mxGeometry x="0" y="0" width="180" height="440" as="geometry"/>
</mxCell>
<mxCell id="column-b" style="swimlane;childLayout=stackLayout;horizontalStack=0;sketch=1;strokeWidth=3;" vertex="1" parent="board">
  <mxGeometry x="180" y="0" width="180" height="440" as="geometry"/>
</mxCell>
<mxCell id="column-c" style="swimlane;childLayout=stackLayout;horizontalStack=0;sketch=1;strokeWidth=1;" vertex="1" parent="board">
  <mxGeometry x="360" y="0" width="180" height="440" as="geometry"/>
</mxCell>
</root></mxGraphModel></diagram></mxfile>"#
}

pub(super) fn sketch_stack_layout_actual_shape_source() -> &'static str {
    r#"<mxfile type="device"><diagram><mxGraphModel page="1" background="none"><root>
<mxCell id="1" parent="0"/>
<mxCell id="board" style="swimlane;childLayout=stackLayout;horizontalStack=1;sketch=1;strokeWidth=2;" vertex="1" parent="1">
  <mxGeometry x="2" y="1" width="540" height="440" as="geometry"/>
</mxCell>
<mxCell id="column-a" style="swimlane;childLayout=stackLayout;horizontalStack=0;sketch=1;" vertex="1" parent="board">
  <mxGeometry x="0" y="0" width="180" height="440" as="geometry"/>
</mxCell>
<mxCell id="column-b" style="swimlane;childLayout=stackLayout;horizontalStack=0;sketch=1;" vertex="1" parent="board">
  <mxGeometry x="180" y="0" width="180" height="440" as="geometry"/>
</mxCell>
<mxCell id="column-c" style="swimlane;childLayout=stackLayout;horizontalStack=0;sketch=1;" vertex="1" parent="board">
  <mxGeometry x="360" y="0" width="180" height="440" as="geometry"/>
</mxCell>
</root></mxGraphModel></diagram></mxfile>"#
}

pub(super) fn sketch_stack_layout_rough_fill_source() -> String {
    sketch_stack_layout_actual_shape_source().replace("strokeWidth=2", "strokeWidth=1")
}

pub(super) fn sketch_stack_layout_outside_column_source() -> String {
    sketch_stack_layout_rough_fill_source().replace(
        "</root>",
        r#"<mxCell id="outside-column" style="swimlane;childLayout=stackLayout;horizontalStack=0;sketch=1;" vertex="1" parent="board">
  <mxGeometry x="600" y="0" width="30" height="30" as="geometry"/>
</mxCell></root>"#,
    )
}

pub(super) fn sourdough_multi_page_source() -> &'static str {
    r#"<mxfile type="device">
<diagram name="sourdough"><mxGraphModel page="0" background="none"><root>
<mxCell id="1" parent="0"/>
<mxCell id="shape" style="strokeColor=none;" vertex="1" parent="1"><mxGeometry x="0" y="0" width="791" height="541" as="geometry"/></mxCell>
</root></mxGraphModel></diagram>
<diagram name="disabled-page"><mxGraphModel page="0" background="none"><root>
<mxCell id="other" style="strokeColor=none;" vertex="1" parent="1"><mxGeometry x="5000" y="5000" width="10" height="10" as="geometry"/></mxCell>
</root></mxGraphModel></diagram>
</mxfile>"#
}
