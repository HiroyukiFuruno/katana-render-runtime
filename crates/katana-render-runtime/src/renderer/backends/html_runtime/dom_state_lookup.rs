use super::{HtmlDomBridgeState, argument};
use crate::renderer::backends::html_runtime::types::DomValue;
use crate::renderer::backends::html_subresources::HtmlSubresourceLoader;

impl HtmlDomBridgeState {
    pub(crate) fn dispatch(
        &self,
        operation: &str,
        arguments: &[String],
    ) -> Result<DomValue, String> {
        if let Some(result) = self.dispatch_lookup(operation, arguments) {
            return result;
        }
        if let Some(result) = self.dispatch_tree_mutation(operation, arguments) {
            return result;
        }
        if let Some(result) = self.dispatch_misc(operation, arguments) {
            return result;
        }
        Err(format!("unsupported DOM operation: {operation}"))
    }

    fn dispatch_lookup<'a>(
        &self,
        operation: &'a str,
        arguments: &'a [String],
    ) -> Option<Result<DomValue, String>> {
        if is_node_lookup(operation) || is_content_lookup(operation) || is_layout_lookup(operation)
        {
            Some(self.lookup(operation, arguments))
        } else {
            None
        }
    }

    fn dispatch_tree_mutation<'a>(
        &self,
        operation: &'a str,
        arguments: &'a [String],
    ) -> Option<Result<DomValue, String>> {
        match operation {
            "appendChild" | "appendText" | "remove" | "insertAdjacentHTML"
            | "insertAdjacentText" => Some(self.mutate_tree(operation, arguments)),
            "setSelectValue" => Some(self.set_select_value(arguments)),
            "setTextControlValue" => Some(self.set_text_control_value(arguments)),
            "setTextContent" | "setInnerHTML" => Some(self.mutate_content(operation, arguments)),
            "setAttribute" | "removeAttribute" => Some(self.set_attribute(operation, arguments)),
            "styleGet" | "styleSet" => Some(self.style(operation, arguments)),
            _ => None,
        }
    }

    fn dispatch_misc<'a>(
        &self,
        operation: &'a str,
        arguments: &'a [String],
    ) -> Option<Result<DomValue, String>> {
        match operation {
            "setEventTarget" => Some(self.set_event_target(arguments)),
            "requestText" => Some(self.request_text(arguments)),
            "validateImageDataUrl" => Some(self.validate_image_data_url(arguments)),
            _ => None,
        }
    }

    fn validate_image_data_url(&self, arguments: &[String]) -> Result<DomValue, String> {
        let source = argument(arguments, 0)?;
        let cached = self.document.borrow().prevalidated_image_event(source);
        let event = cached.unwrap_or_else(|| {
            if HtmlSubresourceLoader::image_data_url_is_decodable(source) {
                "load"
            } else {
                "error"
            }
        });
        Ok(DomValue::String(event.to_string()))
    }

    pub(super) fn lookup(&self, operation: &str, arguments: &[String]) -> Result<DomValue, String> {
        match operation {
            "getElementById"
            | "querySelector"
            | "querySelectorAll"
            | "elementQuerySelector"
            | "elementQuerySelectorAll"
            | "firstElementChild"
            | "lastElementChild"
            | "elementChildren"
            | "treeRevision"
            | "createElement" => self.lookup_node(operation, arguments),
            _ => self.lookup_content(operation, arguments),
        }
    }
}

fn is_node_lookup(operation: &str) -> bool {
    matches!(
        operation,
        "getElementById"
            | "querySelector"
            | "querySelectorAll"
            | "createElement"
            | "elementQuerySelector"
            | "elementQuerySelectorAll"
            | "firstElementChild"
            | "lastElementChild"
            | "elementChildren"
            | "treeRevision"
    )
}

fn is_content_lookup(operation: &str) -> bool {
    matches!(
        operation,
        "textContent"
            | "innerHTML"
            | "outerHTML"
            | "getAttribute"
            | "anchorHash"
            | "tagName"
            | "hidden"
            | "value"
            | "isConnected"
            | "closest"
    )
}

fn is_layout_lookup(operation: &str) -> bool {
    matches!(
        operation,
        "boundingClientRect"
            | "layoutBoxPresent"
            | "intersectionMetadata"
            | "layoutMetrics"
            | "resourcePresence"
            | "eventPath"
    )
}

#[cfg(test)]
#[path = "dom_state_lookup_tests.rs"]
mod tests;
