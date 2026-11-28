use super::super::super::script::{HtmlTryCatchScope, evaluate_value};
use super::super::super::types::HtmlRuntimeError;
use super::super::StaticHtmlRuntimeSession;

impl StaticHtmlRuntimeSession {
    pub(crate) fn is_text_control(&self, node_id: u64) -> Result<bool, HtmlRuntimeError> {
        let document = self.dom_document()?.borrow();
        let tag = document
            .tag_name(node_id)
            .map_err(HtmlRuntimeError::DomBridge)?;
        let input_type = document
            .attribute(node_id, "type")
            .map_err(HtmlRuntimeError::DomBridge)?;
        Ok(is_text_control(&tag, input_type.as_deref()))
    }
}

pub(in super::super) fn set_focused_element(
    scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
    node_id: Option<u64>,
) -> Result<(), HtmlRuntimeError> {
    let node = node_id.map_or_else(|| "null".to_string(), |id| format!("\"{id}\""));
    evaluate_value(
        scope,
        "krr-html-set-active-element",
        &format!("__krrSetFocusedNodeId({node})"),
    )?;
    Ok(())
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
