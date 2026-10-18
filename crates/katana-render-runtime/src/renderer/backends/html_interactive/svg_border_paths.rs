use super::RoundedBorderGeometry;
use crate::renderer::backends::html_interactive::constants::BORDER_RADIUS_CORNER_COUNT;

#[path = "svg_border_corner.rs"]
mod corner;

pub(super) fn rounded_border_edge_path(edge: usize, geometry: &RoundedBorderGeometry) -> String {
    let corners = corner::rounded_corners(geometry);
    let start = &corners[edge];
    let end = &corners[(edge + 1) % BORDER_RADIUS_CORNER_COUNT];
    let mut path = String::new();
    append_point(&mut path, 'M', start.outer_split);
    append_arc(&mut path, start.outer_radius, true, start.outer_outgoing);
    append_point(&mut path, 'L', end.outer_incoming);
    append_arc(&mut path, end.outer_radius, true, end.outer_split);
    append_point(&mut path, 'L', end.inner_split);
    append_arc(&mut path, end.inner_radius, false, end.inner_incoming);
    append_point(&mut path, 'L', start.inner_outgoing);
    append_arc(&mut path, start.inner_radius, false, start.inner_split);
    path.push('Z');
    path
}

fn append_point(path: &mut String, command: char, point: (f32, f32)) {
    path.push_str(&format!("{command} {} {} ", point.0, point.1));
}

fn append_arc(path: &mut String, radius: (f32, f32), clockwise: bool, point: (f32, f32)) {
    if radius.0 <= 0.0 || radius.1 <= 0.0 {
        append_point(path, 'L', point);
        return;
    }
    path.push_str(&format!(
        "A {} {} 0 0 {} {} {} ",
        radius.0,
        radius.1,
        u8::from(clockwise),
        point.0,
        point.1
    ));
}
