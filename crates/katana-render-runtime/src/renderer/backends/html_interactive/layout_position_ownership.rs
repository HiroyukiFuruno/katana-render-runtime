use super::super::layout::HtmlLayoutRenderer;
use super::super::style::CssPosition;
use super::super::types::ElementPositioningContext;

impl HtmlLayoutRenderer {
    pub(in crate::renderer::backends::html_interactive) fn in_flow_positioning_context(
        &self,
    ) -> ElementPositioningContext {
        self.ownership
            .rendering_elements
            .iter()
            .rev()
            .filter_map(|owner| self.ownership.element_box_indices.get(owner))
            .filter_map(|index| self.element_boxes.get(*index))
            .map(|element| element.positioning_context)
            .find(|context| *context != ElementPositioningContext::InFlow)
            .unwrap_or(ElementPositioningContext::InFlow)
    }

    pub(in crate::renderer::backends::html_interactive) fn positioning_origin_node_id(
        &self,
        node_id: u64,
        position: CssPosition,
    ) -> Option<u64> {
        if matches!(position, CssPosition::Absolute | CssPosition::Fixed) {
            return Some(node_id);
        }
        self.ownership
            .rendering_elements
            .iter()
            .rev()
            .filter_map(|owner| self.ownership.element_box_indices.get(owner))
            .filter_map(|index| self.element_boxes.get(*index))
            .find_map(|element| element.positioning_origin_node_id)
    }
}
