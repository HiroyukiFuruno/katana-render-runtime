use super::constants::BORDER_RADIUS_CORNER_COUNT;
use super::layout::HtmlLayoutRenderer;

impl HtmlLayoutRenderer {
    pub(super) fn clip_painted_range(
        &mut self,
        index: usize,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        corner_radii: [(f32, f32); BORDER_RADIUS_CORNER_COUNT],
    ) {
        let clip_id = self.next_clip_id;
        self.next_clip_id += 1;
        self.svg.push_str("</g>");
        let clip_shape = clip_shape(x, y, width, height, self.scroll_y, corner_radii);
        self.svg.insert_str(
            index,
            &format!(
                r#"<defs><clipPath id="krr-clip-{clip_id}">{clip_shape}</clipPath></defs><g clip-path="url(#krr-clip-{clip_id})">"#,
            ),
        );
    }
}

fn clip_shape(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    scroll_y: f32,
    corner_radii: [(f32, f32); BORDER_RADIUS_CORNER_COUNT],
) -> String {
    if corner_radii.iter().all(|radius| *radius == corner_radii[0]) {
        return format!(
            r#"<rect x="{x}" y="{}" width="{width}" height="{height}" rx="{}" ry="{}"/>"#,
            y - scroll_y,
            corner_radii[0].0,
            corner_radii[0].1,
        );
    }
    rounded_clip_path(x, y - scroll_y, width, height, corner_radii)
}

fn rounded_clip_path(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    corner_radii: [(f32, f32); BORDER_RADIUS_CORNER_COUNT],
) -> String {
    let [
        (top_left_x, top_left_y),
        (top_right_x, top_right_y),
        (bottom_right_x, bottom_right_y),
        (bottom_left_x, bottom_left_y),
    ] = corner_radii;
    format!(
        r#"<path d="M {} {y} H {} A {top_right_x} {top_right_y} 0 0 1 {} {} V {} A {bottom_right_x} {bottom_right_y} 0 0 1 {} {} H {} A {bottom_left_x} {bottom_left_y} 0 0 1 {x} {} V {} A {top_left_x} {top_left_y} 0 0 1 {} {y} Z"/>"#,
        x + top_left_x,
        x + width - top_right_x,
        x + width,
        y + top_right_y,
        y + height - bottom_right_y,
        x + width - bottom_right_x,
        y + height,
        x + bottom_left_x,
        y + height - bottom_left_y,
        y + top_left_y,
        x + top_left_x,
    )
}
