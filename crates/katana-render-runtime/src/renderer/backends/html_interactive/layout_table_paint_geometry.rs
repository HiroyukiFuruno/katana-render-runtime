use super::super::super::style::{CssStyle, CssVerticalAlign};
use super::super::super::types::ElementBox;

pub(super) fn update_table_element_box(
    element: &mut ElementBox,
    node_id: u64,
    style: &CssStyle,
    bounds: (f32, f32, f32, f32),
    union: bool,
    clip_axes: (bool, bool),
) {
    let (x, y, width, height) = union_bounds(element, bounds, union);
    element.x = x;
    element.y = y;
    element.width = width;
    element.height = height;
    element.transformed_corners = ElementBox::rectangle_corners(x, y, width, height);
    element.overflow_clip_axes = clip_axes;
    element.padding_edge =
        super::super::super::layout_element::padding_edge(node_id, x, y, width, height, style)
            .with_active_axes(clip_axes);
}

fn union_bounds(
    element: &ElementBox,
    bounds: (f32, f32, f32, f32),
    union: bool,
) -> (f32, f32, f32, f32) {
    let (x, y, width, height) = bounds;
    if !union {
        return bounds;
    }
    let left = element.x.min(x);
    let top = element.y.min(y);
    let right = (element.x + element.width).max(x + width);
    let bottom = (element.y + element.height).max(y + height);
    (left, top, right - left, bottom - top)
}

pub(super) fn table_cell_content_offset(
    alignment: CssVerticalAlign,
    row_height: f32,
    cell_height: f32,
) -> f32 {
    let free_space = (row_height - cell_height).max(0.0);
    match alignment {
        CssVerticalAlign::Top | CssVerticalAlign::Baseline => 0.0,
        CssVerticalAlign::Middle => free_space / 2.0,
        CssVerticalAlign::Bottom => free_space,
    }
}
