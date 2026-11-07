use super::{
    BORDER_RADIUS_CORNER_COUNT, ContainerGeometry, CssStyle, HtmlLayoutRenderer, OverflowClip,
};

impl HtmlLayoutRenderer {
    pub(super) fn record_overflow_clip(
        &mut self,
        descendant_box_start: usize,
        geometry: &ContainerGeometry,
        height: f32,
        style: &CssStyle,
    ) {
        if !style.clips_overflow() {
            return;
        }
        let Some(owner_node_id) = self.current_element_owner() else {
            return;
        };
        if !self.clips_element_overflow(owner_node_id, style) {
            return;
        }
        let (x, y, width, height) = overflow_clip_geometry(geometry, height, style);
        let corner_radii = overflow_clip_radius(geometry, height, style);
        let clip = OverflowClip::new(owner_node_id, x, y, width, height, 0.0, 0.0)
            .with_active_axes(self.element_overflow_clip_axes(owner_node_id, style))
            .with_corner_radii(corner_radii);
        for element_box in &mut self.element_boxes[descendant_box_start..] {
            if overflow_clip_applies_to(element_box, owner_node_id) {
                element_box.overflow_clips.push(clip);
            }
        }
    }

    pub(super) fn paint_container_box(
        &mut self,
        box_start: usize,
        content_start: usize,
        geometry: &ContainerGeometry,
        height: f32,
        style: &CssStyle,
    ) {
        self.paint_container_overflow(content_start, geometry, height, style);
        let before_background = self.svg.len();
        self.insert_box(
            box_start,
            geometry.box_x,
            geometry.start,
            geometry.box_width,
            height,
            style,
        );
        self.record_stacking_background(self.svg.len() - before_background);
    }

    fn paint_container_overflow(
        &mut self,
        content_start: usize,
        geometry: &ContainerGeometry,
        height: f32,
        style: &CssStyle,
    ) {
        self.capture_stacking_context_children();
        if style.clips_overflow()
            && self
                .current_element_owner()
                .is_none_or(|owner| self.clips_element_overflow(owner, style))
        {
            let source_owner = (geometry.box_x, geometry.start, geometry.box_width, height);
            let (x, y, width, height) = overflow_clip_geometry(geometry, height, style);
            let corner_radii = overflow_clip_radius(geometry, height, style);
            let Some(owner_node_id) = self.current_element_owner() else {
                self.layout_error = Some("overflow paint clip has no owner".to_string());
                return;
            };
            let clip = OverflowClip::new(owner_node_id, x, y, width, height, 0.0, 0.0)
                .with_active_axes(self.element_overflow_clip_axes(owner_node_id, style))
                .with_corner_radii(corner_radii);
            let clip_id = self.clip_painted_range(content_start, clip, source_owner);
            self.clip_deferred_paint(owner_node_id, clip_id);
        }
    }
}

fn overflow_clip_geometry(
    geometry: &ContainerGeometry,
    height: f32,
    style: &CssStyle,
) -> (f32, f32, f32, f32) {
    let left = style.border_left_width();
    let top = style.border_top_width();
    let width = (geometry.box_width - left - style.border_right_width()).max(0.0);
    let height = (height - top - style.border_bottom_width()).max(0.0);
    (geometry.box_x + left, geometry.start + top, width, height)
}

fn overflow_clip_radius(
    geometry: &ContainerGeometry,
    height: f32,
    style: &CssStyle,
) -> [(f32, f32); BORDER_RADIUS_CORNER_COUNT] {
    style.resolved_inner_border_radii(geometry.box_width, height)
}

pub(super) fn overflow_clip_applies_to(
    element_box: &super::super::types::ElementBox,
    owner_node_id: u64,
) -> bool {
    let context = element_box.positioning_context;
    if context.escapes_element_root() {
        return false;
    }
    if element_box.participates_in_flow {
        return true;
    }
    context
        .containing_block_owner()
        .is_some_and(|containing_block_owner| {
            contains_overflow_clip_owner(element_box, owner_node_id, containing_block_owner)
        })
}

fn contains_overflow_clip_owner(
    element_box: &super::super::types::ElementBox,
    owner_node_id: u64,
    containing_block_owner: u64,
) -> bool {
    element_box
        .ancestor_node_ids
        .iter()
        .position(|ancestor| *ancestor == containing_block_owner)
        .is_some_and(|containing_block_index| {
            element_box.ancestor_node_ids[..=containing_block_index].contains(&owner_node_id)
        })
}
