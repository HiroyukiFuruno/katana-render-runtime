use super::super::arrows::{svg_number_or, transform_bounds};
use super::super::{
    BLOCK_BOTTOM_BOUND_INDEX, BLOCK_BOUNDS_COMPONENT_COUNT, BLOCK_EXPECTED_CHILD_COUNT,
    BLOCK_RIGHT_BOUND_INDEX,
};
use super::{label_rectangle, semantic_node, svg_number};
use resvg::usvg::roxmltree::Node as SvgNode;
use svgtypes::ViewBox;

pub(in super::super) fn assert_block_containment(
    root: SvgNode<'_, '_>,
    block: SvgNode<'_, '_>,
) -> Result<(), String> {
    let parent = element_bounds(label_rectangle(block)?)?;
    let children = [
        element_bounds(label_rectangle(semantic_node(root, "-A")?)?)?,
        element_bounds(label_rectangle(semantic_node(root, "-B")?)?)?,
        element_bounds(label_rectangle(semantic_node(root, "-C")?)?)?,
    ];
    assert_ordered_children(parent, children)?;
    assert_composite_inside_view_box(root, parent)
}

fn assert_ordered_children(
    parent: [f64; BLOCK_BOUNDS_COMPONENT_COUNT],
    children: [[f64; BLOCK_BOUNDS_COMPONENT_COUNT]; BLOCK_EXPECTED_CHILD_COUNT],
) -> Result<(), String> {
    if children.iter().any(|child| !box_contains(parent, *child)) {
        return Err("Block composite does not enclose each child label".to_owned());
    }
    if children[0][2] >= children[1][0] || children[1][2] >= children[2][0] {
        return Err("Block child labels are not ordered and separated".to_owned());
    }
    Ok(())
}

fn box_contains(
    outer: [f64; BLOCK_BOUNDS_COMPONENT_COUNT],
    inner: [f64; BLOCK_BOUNDS_COMPONENT_COUNT],
) -> bool {
    outer[0] <= inner[0]
        && outer[1] <= inner[1]
        && outer[BLOCK_RIGHT_BOUND_INDEX] >= inner[BLOCK_RIGHT_BOUND_INDEX]
        && outer[BLOCK_BOTTOM_BOUND_INDEX] >= inner[BLOCK_BOTTOM_BOUND_INDEX]
}

fn assert_composite_inside_view_box(
    root: SvgNode<'_, '_>,
    parent: [f64; BLOCK_BOUNDS_COMPONENT_COUNT],
) -> Result<(), String> {
    let view_box: ViewBox = root
        .attribute("viewBox")
        .ok_or_else(|| "missing SVG viewBox".to_owned())?
        .parse::<ViewBox>()
        .map_err(|error| error.to_string())?;
    /* WHY: svgtypes が非有限値と非正サイズを parse 時に拒否する。 */
    let view = [
        view_box.x,
        view_box.y,
        view_box.x + view_box.w,
        view_box.y + view_box.h,
    ];
    if !box_contains(view, parent) {
        return Err("Block composite extends beyond the SVG viewBox".to_owned());
    }
    Ok(())
}

fn element_bounds(element: SvgNode<'_, '_>) -> Result<[f64; BLOCK_BOUNDS_COMPONENT_COUNT], String> {
    let x = svg_number_or(element, "x", 0.0)?;
    let y = svg_number_or(element, "y", 0.0)?;
    let width = svg_number(element, "width")?;
    let height = svg_number(element, "height")?;
    if ![x, y, width, height].into_iter().all(f64::is_finite) || width <= 0.0 || height <= 0.0 {
        return Err("invalid SVG label geometry".to_owned());
    }
    let mut bounds = [x, y, x + width, y + height];
    for ancestor in element.ancestors().filter(|item| item.is_element()) {
        if let Some(value) = ancestor.attribute("transform") {
            bounds = transform_bounds(bounds, value)?;
        }
    }
    Ok(bounds)
}
