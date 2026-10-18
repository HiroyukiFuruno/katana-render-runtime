use super::constants::BORDER_RADIUS_CORNER_COUNT;
use super::layout::{DeferredAxisClip, HtmlLayoutRenderer};
use super::layout_paint_order::apply_deferred_clip;
use super::types::OverflowClip;

impl HtmlLayoutRenderer {
    pub(super) fn clip_painted_range(
        &mut self,
        index: usize,
        clip: OverflowClip,
        source_owner: (f32, f32, f32, f32),
    ) -> u64 {
        let clip_id = self.next_clip_id;
        self.next_clip_id += 1;
        self.svg.push_str("</g>");
        if clip.active_axes.0 != clip.active_axes.1 {
            return self.reserve_axis_clip(index, clip_id, clip, source_owner);
        }
        self.reserve_rect_clip(index, clip_id, clip)
    }

    fn reserve_axis_clip(
        &mut self,
        index: usize,
        id: u64,
        clip: OverflowClip,
        source_owner: (f32, f32, f32, f32),
    ) -> u64 {
        self.deferred_axis_clips.push(DeferredAxisClip {
            id,
            owner_node_id: clip.owner_node_id,
            source_owner,
            clip,
        });
        self.insert_clip_wrapper(index, id);
        id
    }

    fn reserve_rect_clip(&mut self, index: usize, id: u64, clip: OverflowClip) -> u64 {
        let clip_shape = clip_shape(
            clip.x - self.scroll_x,
            clip.y,
            clip.width,
            clip.height,
            self.scroll_y,
            clip.corner_radii,
        );
        self.svg.insert_str(
            index,
            &format!(
                r#"<defs><clipPath id="krr-clip-{id}">{clip_shape}</clipPath></defs><g clip-path="url(#krr-clip-{id})">"#,
            ),
        );
        id
    }

    fn insert_clip_wrapper(&mut self, index: usize, id: u64) {
        self.svg
            .insert_str(index, &format!(r#"<g clip-path="url(#krr-clip-{id})">"#));
    }

    pub(super) fn finalize_deferred_axis_clips(&mut self) -> Result<(), String> {
        super::axis_clip::finalize(self)
    }

    pub(super) fn clip_deferred_paint(&mut self, owner_node_id: u64, clip_id: u64) {
        let depth = self.ownership.paint_contexts.len();
        let indices = &self.ownership.element_box_indices;
        let boxes = &self.element_boxes;
        for paint in &mut self.deferred_paint {
            apply_deferred_clip(paint, owner_node_id, clip_id, depth, indices, boxes);
        }
        for context in &mut self.ownership.paint_contexts {
            for paint in &mut context.children {
                apply_deferred_clip(paint, owner_node_id, clip_id, depth, indices, boxes);
            }
        }
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
