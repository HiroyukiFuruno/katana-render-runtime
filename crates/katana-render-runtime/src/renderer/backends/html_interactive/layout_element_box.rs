use super::super::layout_container::horizontal_box_geometry;
use super::super::types::{
    ELEMENT_BOX_CORNER_COUNT, ElementBox, ElementPositioningContext, LayoutContext,
};
use super::HtmlLayoutRenderer;

impl HtmlLayoutRenderer {
    pub(in crate::renderer::backends::html_interactive) fn start_element_box(
        &mut self,
        node_id: u64,
        positioning_context: ElementPositioningContext,
    ) -> usize {
        let index = self.element_boxes.len();
        let ancestor_node_ids = match positioning_context {
            ElementPositioningContext::AbsoluteContainingBlock { .. } => {
                self.ownership.rendering_elements.clone()
            }
            ElementPositioningContext::InFlow | ElementPositioningContext::FixedViewport => {
                Vec::new()
            }
        };
        self.element_boxes.push(ElementBox {
            node_id,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            transformed_corners: [(0.0, 0.0); ELEMENT_BOX_CORNER_COUNT],
            positioning_context,
            ancestor_node_ids,
            overflow_clips: Vec::new(),
            inline_fragments: Vec::new(),
        });
        index
    }

    pub(super) fn finish_element_box(
        &mut self,
        index: usize,
        node_id: u64,
        layout: LayoutContext<'_>,
        bottom: f32,
    ) {
        let (x, width) = horizontal_box_geometry(layout.x, layout.width, layout.style);
        let y = layout.y + layout.style.margin_top;
        let height = (bottom - y - layout.style.margin_bottom).max(0.0);
        let element_box = &mut self.element_boxes[index];
        debug_assert_eq!(element_box.node_id, node_id);
        element_box.x = x;
        element_box.y = y;
        element_box.width = width;
        element_box.height = height;
        element_box.transformed_corners = ElementBox::rectangle_corners(x, y, width, height);
    }
}
