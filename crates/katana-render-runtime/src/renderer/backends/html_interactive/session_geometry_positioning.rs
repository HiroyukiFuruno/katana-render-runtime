use super::super::types::{ElementBox, ElementPositioningContext};

pub(super) fn intersection_positioning(element: &ElementBox) -> (&'static str, String, bool) {
    match element.positioning_context {
        ElementPositioningContext::InFlow => ("in-flow", "null".to_string(), false),
        ElementPositioningContext::FixedViewport => ("fixed", "null".to_string(), true),
        ElementPositioningContext::FixedContainingBlock {
            owner_node_id,
            viewport_escape,
        } => (
            "fixed-containing-block",
            owner_node_id.to_string(),
            viewport_escape,
        ),
        ElementPositioningContext::AbsoluteContainingBlock {
            owner_node_id,
            viewport_escape,
        } => (
            "absolute",
            owner_node_id.map_or_else(|| "null".to_string(), |owner| owner.to_string()),
            viewport_escape,
        ),
    }
}
