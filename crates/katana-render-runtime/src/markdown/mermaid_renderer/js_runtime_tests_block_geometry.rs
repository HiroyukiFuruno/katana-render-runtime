use super::*;
use resvg::usvg::roxmltree::Document as SvgDocument;

const BLOCK_BOUNDS_COMPONENT_COUNT: usize = 4;
const BLOCK_RIGHT_BOUND_INDEX: usize = 2;
const BLOCK_BOTTOM_BOUND_INDEX: usize = 3;
const BLOCK_EXPECTED_CHILD_COUNT: usize = 3;
const BLOCK_SERIALIZED_NBSP_TOKEN_COUNT: usize = 3;
const BLOCK_MIN_ARROW_POLYGON_VERTICES: usize = 3;
const BLOCK_NORMAL_FONT_WEIGHT: u16 = 400;
const BLOCK_LABEL_HORIZONTAL_PADDING_PX: f32 = 32.0;
const BLOCK_ARROW_TOTAL_PADDING_PX: f32 = 24.0;

#[path = "js_runtime_tests_block_arrow.rs"]
mod arrows;
#[path = "js_runtime_coverage_gap_tests.rs"]
mod coverage_gap_tests;
#[path = "js_runtime_tests_block_layout.rs"]
mod layout;

#[test]
fn runtime_normalizes_block_nbsp_arrow_width() -> Result<(), String> {
    let mermaid = RuntimeAsset::mermaid();
    let rendered = mermaid
        .materialize_at(mermaid.materialized_path())
        .and_then(|mermaid_js| {
            MermaidJsRuntimeOps::render(
                block_nbsp_arrow_source(),
                &mermaid_js,
                DiagramColorPreset::dark(),
            )
        });

    rendered.and_then(|svg| assert_block_nbsp_arrow_svg(&svg))
}

fn block_nbsp_arrow_source() -> &'static str {
    "block-beta\ncolumns 1\n  db((\"DB\"))\n  blockArrowId6<[\"&nbsp;&nbsp;&nbsp;\"]>(down)\n  block:ID\n    A\n    B[\"A wide one in the middle\"]\n    C\n  end\n  space\n  D\n  ID --> D\n  C --> D\n  style B fill:#969,stroke:#333,stroke-width:4px"
}

fn assert_block_nbsp_arrow_svg(svg: &str) -> Result<(), String> {
    let document = SvgDocument::parse(svg).map_err(|error| error.to_string())?;
    let root = document.root_element();
    let block = layout::semantic_node(root, "-ID")?;
    let label = layout::semantic_node(root, "-B")?;
    let label_rect = layout::label_rectangle(label)?;

    assert_eq!(layout::node_text(layout::semantic_node(root, "-A")?), "A");
    assert_eq!(layout::node_text(label), "A wide one in the middle");
    assert_eq!(layout::node_text(layout::semantic_node(root, "-C")?), "C");
    assert_eq!(layout::node_text(layout::semantic_node(root, "-D")?), "D");
    layout::assert_block_label_width(root, label_rect)?;
    layout::assert_label_style(label_rect)?;
    layout::assert_block_containment(root, block)?;
    arrows::assert_block_arrow(root, svg)?;
    arrows::assert_connected_block_edges(root)?;
    Ok(())
}
