use super::super::layout_container::horizontal_box_geometry;
use super::super::style::CssStyle;
use super::super::types::{
    ELEMENT_BOX_CORNER_COUNT, ElementBox, ElementPositioningContext, LayoutContext, OverflowClip,
};
use super::HtmlLayoutRenderer;

impl HtmlLayoutRenderer {
    pub(in crate::renderer::backends::html_interactive) fn start_element_box(
        &mut self,
        node_id: u64,
        positioning_context: ElementPositioningContext,
        participates_in_flow: bool,
        positioning_origin_node_id: Option<u64>,
    ) -> usize {
        let index = self.element_boxes.len();
        self.element_boxes.push(ElementBox {
            node_id,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            transformed_corners: [(0.0, 0.0); ELEMENT_BOX_CORNER_COUNT],
            positioning_context,
            positioning_origin_node_id,
            participates_in_flow,
            ancestor_node_ids: self.ancestor_node_ids(positioning_context),
            overflow_clips: Vec::new(),
            clips_overflow: false,
            padding_edge: OverflowClip::new(node_id, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            inline_fragments: Vec::new(),
        });
        index
    }

    fn ancestor_node_ids(&self, positioning_context: ElementPositioningContext) -> Vec<u64> {
        match positioning_context {
            ElementPositioningContext::FixedContainingBlock { .. }
            | ElementPositioningContext::AbsoluteContainingBlock { .. } => {
                self.ownership.rendering_elements.clone()
            }
            ElementPositioningContext::InFlow | ElementPositioningContext::FixedViewport => {
                Vec::new()
            }
        }
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
        element_box.clips_overflow = layout.style.clips_overflow();
        element_box.padding_edge = padding_edge(node_id, x, y, width, height, layout.style);
    }
}

fn padding_edge(
    node_id: u64,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    style: &CssStyle,
) -> OverflowClip {
    let padding_x = x + style.border_left_width();
    let padding_y = y + style.border_top_width();
    let padding_width = (width - style.border_left_width() - style.border_right_width()).max(0.0);
    let padding_height = (height - style.border_top_width() - style.border_bottom_width()).max(0.0);
    OverflowClip::new(
        node_id,
        padding_x,
        padding_y,
        padding_width,
        padding_height,
        0.0,
        0.0,
    )
    .with_corner_radii(style.resolved_inner_border_radii(width, height))
}
