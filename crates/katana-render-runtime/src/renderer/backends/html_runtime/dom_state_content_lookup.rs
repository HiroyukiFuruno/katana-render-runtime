use super::{HtmlDomBridgeState, argument, node_id};
use crate::renderer::backends::html_runtime::types::DomValue;

impl HtmlDomBridgeState {
    pub(super) fn lookup_content(
        &self,
        operation: &str,
        arguments: &[String],
    ) -> Result<DomValue, String> {
        if let Some(result) = self.lookup_geometry(operation, arguments)? {
            return Ok(result);
        }
        let document = self.document.borrow();
        lookup_document_content(&document, operation, arguments)
    }

    fn lookup_geometry(
        &self,
        operation: &str,
        arguments: &[String],
    ) -> Result<Option<DomValue>, String> {
        match operation {
            "boundingClientRect" => Ok(Some(DomValue::String(
                self.bounding_client_rect_json(node_id(argument(arguments, 0)?)?),
            ))),
            "layoutMetrics" => Ok(Some(DomValue::String(self.layout_metrics_json()))),
            _ => Ok(None),
        }
    }
}

fn lookup_document_content(
    document: &crate::renderer::backends::html_document::HtmlDocument,
    operation: &str,
    arguments: &[String],
) -> Result<DomValue, String> {
    match operation {
        "textContent" => document
            .text_content(node_id(argument(arguments, 0)?)?)
            .map(DomValue::String),
        "innerHTML" => document
            .inner_html(node_id(argument(arguments, 0)?)?)
            .map(DomValue::String),
        "outerHTML" => document
            .outer_html(node_id(argument(arguments, 0)?)?)
            .map(DomValue::String),
        "getAttribute" => Ok(document
            .attribute(node_id(argument(arguments, 0)?)?, argument(arguments, 1)?)?
            .map(DomValue::String)
            .unwrap_or(DomValue::Null)),
        "eventPath" => document
            .event_path(node_id(argument(arguments, 0)?)?)
            .map(DomValue::NodeIds),
        "closest" => document
            .closest_selector(node_id(argument(arguments, 0)?)?, argument(arguments, 1)?)
            .map(|node| node.map(DomValue::NodeId).unwrap_or(DomValue::Null)),
        _ => Err(format!(
            "unsupported HTML content lookup operation: {operation}"
        )),
    }
}
