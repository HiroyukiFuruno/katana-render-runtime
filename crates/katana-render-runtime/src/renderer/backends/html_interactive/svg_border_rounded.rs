use super::super::super::style::CssStyle;
use super::super::escape_xml;
use crate::renderer::backends::html_interactive::constants::BORDER_RADIUS_CORNER_COUNT;

#[path = "svg_border_paths.rs"]
mod paths;

pub(super) fn rounded_edge_colors_present(style: &CssStyle) -> bool {
    let colors = [
        style.border_top_color(),
        style.border_right_color(),
        style.border_bottom_color(),
        style.border_left_color(),
    ];
    colors.into_iter().all(|color| color.is_some())
}

pub(super) fn append_rounded_border_edges(
    svg: &mut String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    style: &CssStyle,
) {
    let geometry = rounded_border_dimensions(x, y, width, height, style);
    let colors = [
        style.border_top_color(),
        style.border_right_color(),
        style.border_bottom_color(),
        style.border_left_color(),
    ];
    for (index, color) in colors.into_iter().enumerate() {
        if geometry.widths[index] <= 0.0 {
            continue;
        }
        if let Some(color) = color {
            let path = paths::rounded_border_edge_path(index, &geometry);
            svg.push_str(&format!(
                r#"<path d="{path}" fill="{}" fill-rule="evenodd" clip-rule="evenodd"/>"#,
                escape_xml(color)
            ));
        }
    }
}

struct RoundedBorderGeometry {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
    outer: [(f32, f32); BORDER_RADIUS_CORNER_COUNT],
    inner: [(f32, f32); BORDER_RADIUS_CORNER_COUNT],
    widths: [f32; BORDER_RADIUS_CORNER_COUNT],
}

fn rounded_border_dimensions(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    style: &CssStyle,
) -> RoundedBorderGeometry {
    let outer_radius = style.resolved_border_radius(width, height);
    let left = style.border_left_width();
    let right = style.border_right_width();
    let top = style.border_top_width();
    let bottom = style.border_bottom_width();
    let inner = inner_border_radii(style, width, height, left, right, top, bottom);
    RoundedBorderGeometry {
        x,
        y,
        width,
        height,
        left,
        right,
        top,
        bottom,
        outer: [outer_radius; BORDER_RADIUS_CORNER_COUNT],
        inner,
        widths: [top, right, bottom, left],
    }
}

fn inner_border_radii(
    style: &CssStyle,
    width: f32,
    height: f32,
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
) -> [(f32, f32); BORDER_RADIUS_CORNER_COUNT] {
    let inner_width = (width - left - right).max(0.0);
    let inner_height = (height - top - bottom).max(0.0);
    style
        .resolved_inner_border_radii(width, height)
        .map(|(rx, ry)| (rx.min(inner_width / 2.0), ry.min(inner_height / 2.0)))
}
