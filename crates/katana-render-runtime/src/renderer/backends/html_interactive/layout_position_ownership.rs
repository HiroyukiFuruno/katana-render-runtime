use super::super::layout::HtmlLayoutRenderer;
use super::super::types::ElementPositioningContext;

impl HtmlLayoutRenderer {
    pub(super) fn in_flow_positioning_context(&self) -> ElementPositioningContext {
        self.ownership
            .rendering_elements
            .iter()
            .rev()
            .filter_map(|owner| {
                self.element_boxes
                    .iter()
                    .rev()
                    .find(|element| element.node_id == *owner)
                    .map(|element| element.positioning_context)
            })
            .find(|context| *context != ElementPositioningContext::InFlow)
            .unwrap_or(ElementPositioningContext::InFlow)
    }
}
