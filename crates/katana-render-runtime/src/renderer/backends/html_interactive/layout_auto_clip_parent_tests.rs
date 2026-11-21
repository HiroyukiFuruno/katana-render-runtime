use super::deferred_clip_tests::assert_clip_owner;
use super::paint_context_boundary_tests::renderer;
use super::stacking_context_tests::{element, pixel, rasterize_layout, render_pixels};
use crate::renderer::backends::html_document::HtmlDocumentNode as Node;
use crate::renderer::backends::html_interactive::style::CssStyle;
use crate::renderer::backends::html_interactive::types::DetailsContext;

type TestResult = Result<(), Box<dyn std::error::Error>>;
const CLIP_NODE_ID: u64 = 2;
const AUTO_PARENT_NODE_ID: u64 = 3;
const ABSOLUTE_CHILD_NODE_ID: u64 = 4;
const OUTER_SIBLING_NODE_ID: u64 = 5;

#[test]
fn auto_parent_background_stays_behind_eligible_and_escaping_auto_children() -> TestResult {
    for position in ["absolute", "fixed"] {
        assert_auto_parent_background(position)?;
    }
    Ok(())
}

fn assert_auto_parent_background(position: &str) -> TestResult {
    let child = element(
        ABSOLUTE_CHILD_NODE_ID,
        &format!("position:{position};left:10px;top:10px;width:30px;height:30px;background:red"),
        vec![],
    );
    let parent = element(
        AUTO_PARENT_NODE_ID,
        "position:relative;width:40px;height:40px;background:blue",
        vec![child],
    );
    let clip = element(
        CLIP_NODE_ID,
        "width:40px;height:40px;overflow:hidden",
        vec![parent],
    );
    let root = element(1, "position:relative;width:100px;height:100px", vec![clip]);
    let (layout, pixels) = render_pixels(root)?;
    assert_clip_owner(&layout, AUTO_PARENT_NODE_ID, CLIP_NODE_ID, true)?;
    assert_clip_owner(
        &layout,
        ABSOLUTE_CHILD_NODE_ID,
        CLIP_NODE_ID,
        position == "absolute",
    )?;
    assert_eq!(pixel(&pixels, 20, 20)?, [255, 0, 0, 255]);
    assert_eq!(pixel(&pixels, 5, 5)?, [0, 0, 255, 255]);
    Ok(())
}

#[test]
fn auto_parent_does_not_trap_explicit_z_descendant_below_outer_sibling() -> TestResult {
    assert_explicit_z_descendant_order()
}

fn assert_explicit_z_descendant_order() -> TestResult {
    let child = element(
        ABSOLUTE_CHILD_NODE_ID,
        "position:absolute;left:10px;top:10px;width:30px;height:30px;background:red;z-index:9",
        vec![],
    );
    let parent = element(
        AUTO_PARENT_NODE_ID,
        "position:relative;width:40px;height:40px;background:blue",
        vec![child],
    );
    let clip = element(
        CLIP_NODE_ID,
        "width:40px;height:40px;overflow:hidden",
        vec![parent],
    );
    let sibling = element(
        OUTER_SIBLING_NODE_ID,
        "position:absolute;left:10px;top:10px;width:30px;height:30px;background:lime;z-index:1",
        vec![],
    );
    let root = explicit_z_root(clip, sibling);
    let (layout, pixels) = render_pixels(root)?;
    assert_clip_owner(&layout, ABSOLUTE_CHILD_NODE_ID, CLIP_NODE_ID, true)?;
    assert_clip_owner(&layout, OUTER_SIBLING_NODE_ID, CLIP_NODE_ID, false)?;
    assert_eq!(pixel(&pixels, 20, 20)?, [255, 0, 0, 255]);
    Ok(())
}

fn explicit_z_root(clip: Node, sibling: Node) -> Node {
    element(
        1,
        "position:relative;width:100px;height:100px",
        vec![clip, sibling],
    )
}

#[test]
fn reserved_auto_orders_stay_unique_when_explicit_context_defer_interleaves() -> TestResult {
    let mut renderer = renderer();
    renderer.render_node(
        &order_fixture(),
        0.0,
        0.0,
        160.0,
        &CssStyle::browser_default(),
        DetailsContext::NONE,
    );
    let mut observed: Vec<_> = renderer
        .deferred_paint
        .iter()
        .map(|paint| (paint.node_id, paint.order))
        .collect();
    observed.sort_by_key(|entry| entry.0);
    assert_eq!(observed, [(2, 0), (3, 1), (4, 2), (5, 3)]);
    assert_eq!(renderer.next_paint_order, 4);
    assert!(renderer.ownership.paint_contexts.is_empty());
    assert!(renderer.ownership.rendering_elements.is_empty());
    let layout = renderer.into_layout_result(100.0)?;
    let pixels = rasterize_layout(&layout)?;
    assert_eq!(pixel(&pixels, 20, 20)?, [0, 255, 0, 255]);
    Ok(())
}

fn order_fixture() -> Node {
    let auto = element(
        AUTO_PARENT_NODE_ID,
        "position:absolute;left:10px;top:10px;width:30px;height:30px;background:red",
        vec![],
    );
    let explicit = element(
        ABSOLUTE_CHILD_NODE_ID,
        "position:absolute;left:10px;top:10px;width:30px;height:30px;background:lime;z-index:1",
        vec![],
    );
    let parent = element(
        2,
        "position:relative;width:40px;height:40px;background:blue",
        vec![auto, explicit],
    );
    let later = element(
        OUTER_SIBLING_NODE_ID,
        "position:absolute;left:10px;top:10px;width:30px;height:30px;background:blue",
        vec![],
    );
    element(
        1,
        "position:relative;width:100px;height:100px;overflow:hidden",
        vec![parent, later],
    )
}
