pub(super) fn sketch_bundle_with_top_level_shape() -> String {
    sketch_stack_layout_bundle().replace(
        "  svg.appendChild(createRectGroup(\"column-c\", 361, 1, 180, 440));",
        "  svg.appendChild(createRectGroup(\"column-c\", 361, 1, 180, 440));\n  svg.appendChild(createRectGroup(\"outside\", 600, 20, 30, 30));",
    )
}

pub(super) fn sketch_bundle_with_second_board() -> String {
    sketch_stack_layout_bundle().replace(
        "  svg.appendChild(createRectGroup(\"column-c\", 361, 1, 180, 440));",
        "  svg.appendChild(createRectGroup(\"column-c\", 361, 1, 180, 440));\n  svg.appendChild(createRectGroup(\"board-2\", 600, 550, 100, 50));",
    )
}

pub(super) fn sketch_bundle_with_outside_child() -> String {
    sketch_stack_layout_bundle().replace(
        "  svg.appendChild(createRectGroup(\"column-c\", 361, 1, 180, 440));",
        "  svg.appendChild(createRectGroup(\"column-c\", 361, 1, 180, 440));\n  svg.appendChild(createRectGroup(\"outside-child\", 600, 550, 30, 30));",
    )
}

pub(super) fn sketch_stack_layout_bundle() -> &'static str {
    r#"
function Graph() {}
const Editor = { convertHtmlToText(value) { return String(value); } };
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "800px");
  svg.setAttribute("height", "700px");
  svg.setAttribute("viewBox", "0 0 800 700");
  svg.appendChild(createRectGroup("board", 1, 1, 540, 440));
  svg.appendChild(createRectGroup("column-a", 1, 1, 180, 440));
  svg.appendChild(createRectGroup("column-b", 181, 1, 180, 440));
  svg.appendChild(createRectGroup("column-c", 361, 1, 180, 440));
  callback({ graph: { getSvg() { return svg; } } });
};
function createRectGroup(id, x, y, width, height) {
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", id);
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("x", String(x));
  rect.setAttribute("y", String(y));
  rect.setAttribute("width", String(width));
  rect.setAttribute("height", String(height));
  group.appendChild(rect);
  return group;
}
"#
}

const SKETCH_STACK_LAYOUT_ACTUAL_SHAPE_BUNDLE: &str = r##"
function Graph() {}
const Editor = { convertHtmlToText(value) { return String(value); } };
function GraphViewer() {}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "800px");
  svg.setAttribute("height", "700px");
  svg.setAttribute("viewBox", "0 0 800 700");
  const board = document.createElementNS("http://www.w3.org/2000/svg", "g");
  board.setAttribute("data-cell-id", "board");
  const headerHitRegion = document.createElementNS("http://www.w3.org/2000/svg", "path");
  headerHitRegion.setAttribute("d", "M 2 1 H 542 V 29 H 2 Z");
  headerHitRegion.setAttribute("fill", "none");
  headerHitRegion.setAttribute("stroke", "none");
  headerHitRegion.setAttribute("pointer-events", "all");
  board.appendChild(headerHitRegion);
  const roughStroke = document.createElementNS("http://www.w3.org/2000/svg", "path");
  roughStroke.setAttribute("d", "M 0 1 H 542 V 441 H 0 Z");
  roughStroke.setAttribute("fill", "none");
  roughStroke.setAttribute("stroke", "#000000");
  board.appendChild(roughStroke);
  svg.appendChild(board);
  svg.appendChild(createRectGroup("column-a", 2, 1, 180, 440));
  svg.appendChild(createRectGroup("column-b", 182, 1, 180, 440));
  svg.appendChild(createRectGroup("column-c", 362, 1, 180, 440));
  callback({ graph: { getSvg() { return svg; } } });
};
function createRectGroup(id, x, y, width, height) {
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", id);
  const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  rect.setAttribute("x", String(x));
  rect.setAttribute("y", String(y));
  rect.setAttribute("width", String(width));
  rect.setAttribute("height", String(height));
  group.appendChild(rect);
  return group;
}
"##;

pub(super) fn sketch_stack_layout_actual_shape_bundle() -> &'static str {
    SKETCH_STACK_LAYOUT_ACTUAL_SHAPE_BUNDLE
}

const SKETCH_ROUGH_FILL_HELPERS: &str = r#"function createFillPath(value) {
  const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
  path.setAttribute("d", value);
  path.setAttribute("fill", "rough-fill");
  path.setAttribute("stroke", "none");
  return path;
}
function createFillPathGroup(id, value) {
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", id);
  group.appendChild(createFillPath(value));
  return group;
}
function createRectGroupWithFill(id, value, x, y, width, height) {
  const group = createRectGroup(id, x, y, width, height);
  group.insertBefore(createFillPath(value), group.firstChild);
  return group;
}
function createRectGroup(id, x, y, width, height) {"#;

pub(super) fn sketch_stack_layout_rough_fill_bundle(
    board_fill_y: &str,
    column_fill_x: &str,
) -> String {
    let board_fill = format!(
        "  board.appendChild(roughStroke);\n  board.appendChild(createFillPath(\"M 1 {board_fill_y} H 541 V 441 H 1 Z\"));"
    );
    let column_fill = format!(
        "  svg.appendChild(createRectGroupWithFill(\"column-a\", \"M {column_fill_x} 29 H 181 V 30 H {column_fill_x} Z\", 1, 1, 180, 440));"
    );
    sketch_stack_layout_actual_shape_bundle()
        .replace(r#"M 2 1 H 542 V 29 H 2 Z"#, r#"M 1 29 H 541 V 1 H 1 Z"#)
        .replace("  board.appendChild(roughStroke);", &board_fill)
        .replace(
            "  svg.appendChild(createRectGroup(\"column-a\", 2, 1, 180, 440));",
            &column_fill,
        )
        .replace(
            "function createRectGroup(id, x, y, width, height) {",
            SKETCH_ROUGH_FILL_HELPERS,
        )
}

pub(super) fn sketch_stack_layout_bundle_with_outside_column() -> String {
    sketch_stack_layout_rough_fill_bundle("0.78", "0.96").replace(
        "  svg.appendChild(createRectGroupWithFill(\"column-a\", \"M 0.96 29 H 181 V 30 H 0.96 Z\", 1, 1, 180, 440));",
        "  svg.appendChild(createRectGroupWithFill(\"column-a\", \"M 0.96 29 H 181 V 30 H 0.96 Z\", 1, 1, 180, 440));\n  svg.appendChild(createFillPathGroup(\"outside-column\", \"M 600 1 H 630 V 31 H 600 Z\"));",
    )
}

pub(super) fn sketch_stack_layout_normal_crop_bundle(bundle: &str) -> String {
    bundle.replacen(
        r#"headerHitRegion.setAttribute("pointer-events", "all");"#,
        r#"headerHitRegion.setAttribute("pointer-events", "none");"#,
        1,
    )
}

const SOURDOUGH_MULTI_PAGE_BUNDLE: &str = r#"
function Graph() {{}}
const Editor = {{ convertHtmlToText(value) {{ return String(value); }} }};
function GraphViewer() {{}}
GraphViewer.createViewerForElement = function createViewerForElement(_container, callback) {{
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", "792px");
  svg.setAttribute("height", "563px");
  svg.setAttribute("viewBox", "0 0 792 563");
  const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
  group.setAttribute("data-cell-id", "shape");
  const sourcePaint = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  sourcePaint.setAttribute("x", "0");
  sourcePaint.setAttribute("y", "0");
  sourcePaint.setAttribute("width", "791");
  sourcePaint.setAttribute("height", "541");
  group.appendChild(sourcePaint);
  svg.appendChild(group);
  const bottomContent = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  bottomContent.setAttribute("x", "-1");
  bottomContent.setAttribute("y", "-1");
  bottomContent.setAttribute("width", "792");
  bottomContent.setAttribute("height", "{content_bottom_plus_one}");
  svg.appendChild(bottomContent);
  callback({{ graph: {{ getSvg() {{ return svg; }} }} }});
}};
"#;

pub(super) fn sourdough_multi_page_bundle(content_bottom: i32) -> String {
    SOURDOUGH_MULTI_PAGE_BUNDLE
        .replace("{{", "{")
        .replace("}}", "}")
        .replace(
            "{content_bottom_plus_one}",
            &(content_bottom + 1).to_string(),
        )
}
