use super::super::super::dom_state::HtmlDomBridgeState;
use super::super::super::types::HtmlRuntimeError;
use super::super::StaticHtmlRuntimeSession;
use super::{discarded_runtime_error, dom_state_unavailable_error};

impl StaticHtmlRuntimeSession {
    pub(crate) fn is_text_control(&self, node_id: u64) -> Result<bool, HtmlRuntimeError> {
        let document = self
            .isolate
            .as_ref()
            .ok_or_else(discarded_runtime_error)?
            .get_slot::<HtmlDomBridgeState>()
            .ok_or_else(dom_state_unavailable_error)?
            .document
            .borrow();
        let tag = document
            .tag_name(node_id)
            .map_err(HtmlRuntimeError::DomBridge)?;
        let input_type = document
            .attribute(node_id, "type")
            .map_err(HtmlRuntimeError::DomBridge)?;
        Ok(is_text_control(&tag, input_type.as_deref()))
    }
}

const NON_TEXT_INPUT_TYPES: [&str; 10] = [
    "button", "checkbox", "color", "file", "hidden", "image", "radio", "range", "reset", "submit",
];

fn is_text_control(tag: &str, input_type: Option<&str>) -> bool {
    match tag {
        "TEXTAREA" => true,
        "INPUT" => input_type.is_none_or(|input_type| {
            NON_TEXT_INPUT_TYPES
                .iter()
                .all(|non_text_type| !input_type.eq_ignore_ascii_case(non_text_type))
        }),
        _ => false,
    }
}
