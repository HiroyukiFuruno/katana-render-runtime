use super::{
    DEVICE_SCALE, HashMap, HtmlBrowserViewport, HtmlLayoutRenderer, NATURAL_LAYOUT_HEIGHT,
    NATURAL_LAYOUT_WIDTH, NO_SCROLL_Y, ZERO_LENGTH, find_element_id, parsed_nodes,
};
use crate::renderer::backends::html_interactive::types::LayoutResult;

#[test]
fn parsed_flex_list_keeps_markers_for_direct_items() -> Result<(), String> {
    let nodes = parsed_nodes("<ol style='display:flex'><li>A</li><li>B</li></ol>");
    let layout = HtmlLayoutRenderer::render(
        &nodes,
        natural_layout_viewport(),
        NO_SCROLL_Y,
        &HashMap::new(),
        None,
    )?;
    for content in [">1.<", ">2.<", ">A<", ">B<"] {
        assert!(layout.svg.contains(content), "missing {content}");
    }
    Ok(())
}

#[test]
fn parsed_column_stretch_and_center_keep_expected_cross_widths() {
    assert_column_alignment_width("stretch", true);
    assert_column_alignment_width("normal", true);
    assert_column_alignment_width("center;align-items:normal", true);
    assert_column_alignment_width("center", false);
}

fn assert_column_alignment_width(alignment: &str, stretched: bool) {
    let html = format!(
        "<style>html,body{{margin:0}}</style><div id=column style='display:flex;flex-direction:column;align-items:{alignment};width:120px'><span id=child style='white-space:nowrap'>small</span></div>"
    );
    let nodes = parsed_nodes(&html);
    let ids = find_element_id(&nodes, "column").zip(find_element_id(&nodes, "child"));
    let rendered = HtmlLayoutRenderer::render(
        &nodes,
        natural_layout_viewport(),
        NO_SCROLL_Y,
        &HashMap::new(),
        None,
    );
    assert!(
        rendered.is_ok_and(|layout| column_widths_match(&layout, ids, stretched)),
        "parsed column alignment {alignment} did not preserve expected cross width"
    );
}

fn column_widths_match(layout: &LayoutResult, ids: Option<(u64, u64)>, stretched: bool) -> bool {
    ids.is_some_and(|(column_id, child_id)| {
        let width = |id| layout.element_boxes.iter().find(|item| item.node_id == id);
        width(column_id)
            .zip(width(child_id))
            .is_some_and(|(column, child)| {
                column.width == NATURAL_LAYOUT_WIDTH as f32
                    && if stretched {
                        child.width == column.width
                    } else {
                        child.width > ZERO_LENGTH && child.width < column.width
                    }
            })
    })
}

fn natural_layout_viewport() -> HtmlBrowserViewport {
    HtmlBrowserViewport {
        width: NATURAL_LAYOUT_WIDTH,
        height: NATURAL_LAYOUT_HEIGHT,
        device_scale_factor: DEVICE_SCALE,
    }
}
