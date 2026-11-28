#[path = "layout_input_geometry.rs"]
mod geometry;
#[path = "layout_input_render.rs"]
mod render;
#[path = "layout_input_values.rs"]
mod values;

#[cfg(test)]
#[path = "layout_input_tests.rs"]
mod tests;

#[cfg(test)]
fn input_geometry(
    x: f32,
    y: f32,
    width: f32,
    style: &super::style::CssStyle,
) -> geometry::InputGeometry {
    geometry::input_geometry(x, y, width, &[], style)
}

pub(super) fn is_checkbox(attributes: &[(String, String)]) -> bool {
    geometry::is_checkbox(attributes)
}

#[cfg(test)]
pub(super) use super::style::CssStyle;
