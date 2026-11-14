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
            "layoutBoxPresent" => Ok(Some(DomValue::String(
                if self.has_layout_box(node_id(argument(arguments, 0)?)?) {
                    "1"
                } else {
                    "0"
                }
                .to_string(),
            ))),
            "intersectionMetadata" => Ok(Some(DomValue::String(
                self.intersection_metadata_json(node_id(argument(arguments, 0)?)?),
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
    if operation == "resourcePresence" {
        return resource_presence(document);
    }
    lookup_document_markup(document, operation, arguments)
}

fn lookup_document_markup(
    document: &crate::renderer::backends::html_document::HtmlDocument,
    operation: &str,
    arguments: &[String],
) -> Result<DomValue, String> {
    match operation {
        "textContent" | "innerHTML" | "outerHTML" => lookup_markup(document, operation, arguments),
        "getAttribute" | "anchorHash" | "tagName" | "hidden" | "value" | "isConnected" => {
            lookup_attribute(document, operation, arguments)
        }
        "eventPath" | "closest" => lookup_path(document, operation, arguments),
        _ => Err(format!(
            "unsupported HTML content lookup operation: {operation}"
        )),
    }
}

fn lookup_markup(
    document: &crate::renderer::backends::html_document::HtmlDocument,
    operation: &str,
    arguments: &[String],
) -> Result<DomValue, String> {
    let node = node_id(argument(arguments, 0)?)?;
    let value = match operation {
        "textContent" => document.text_content(node),
        "innerHTML" => document.inner_html(node),
        "outerHTML" => document.outer_html(node),
        _ => return Err(format!("unsupported markup lookup operation: {operation}")),
    }?;
    Ok(DomValue::String(value))
}

fn lookup_attribute(
    document: &crate::renderer::backends::html_document::HtmlDocument,
    operation: &str,
    arguments: &[String],
) -> Result<DomValue, String> {
    if operation == "isConnected" {
        return Ok(lookup_connected(document, arguments));
    }
    let node = node_id(argument(arguments, 0)?)?;
    match operation {
        "anchorHash" => lookup_anchor_hash(document, node, arguments),
        "getAttribute" => Ok(document
            .attribute(node, argument(arguments, 1)?)?
            .map(DomValue::String)
            .unwrap_or(DomValue::Null)),
        "tagName" => document.tag_name(node).map(DomValue::String),
        "hidden" => lookup_hidden(document, node),
        "value" => lookup_form_value(document, node),
        _ => Err(format!(
            "unsupported attribute lookup operation: {operation}"
        )),
    }
}

fn lookup_anchor_hash(
    document: &crate::renderer::backends::html_document::HtmlDocument,
    node: u64,
    arguments: &[String],
) -> Result<DomValue, String> {
    let Some(href) = document.anchor_href_attribute(node)? else {
        return Ok(DomValue::Undefined);
    };
    let Some(href) = href else {
        return Ok(DomValue::String(String::new()));
    };
    let hash = resolve_fragment_hash(&href, argument(arguments, 1)?);
    Ok(DomValue::String(hash))
}

fn resolve_fragment_hash(href: &str, base_href: &str) -> String {
    let resolved = url::Url::parse(href)
        .or_else(|_| url::Url::parse(base_href).and_then(|base| base.join(href)));
    match resolved {
        Ok(url) => match url.fragment() {
            Some(fragment) if !fragment.is_empty() => format!("#{fragment}"),
            Some(_) | None => String::new(),
        },
        Err(_) => String::new(),
    }
}

fn lookup_connected(
    document: &crate::renderer::backends::html_document::HtmlDocument,
    arguments: &[String],
) -> DomValue {
    let connected = arguments
        .first()
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|node| document.is_connected(node));
    DomValue::String(if connected { "1" } else { "0" }.to_string())
}

fn lookup_hidden(
    document: &crate::renderer::backends::html_document::HtmlDocument,
    node: u64,
) -> Result<DomValue, String> {
    Ok(document
        .hidden_property(node)?
        .map(|hidden| DomValue::String(if hidden { "1" } else { "0" }.to_string()))
        .unwrap_or(DomValue::Undefined))
}

fn lookup_form_value(
    document: &crate::renderer::backends::html_document::HtmlDocument,
    node: u64,
) -> Result<DomValue, String> {
    Ok(document
        .form_value(node)?
        .map(DomValue::String)
        .unwrap_or(DomValue::Undefined))
}

fn lookup_path(
    document: &crate::renderer::backends::html_document::HtmlDocument,
    operation: &str,
    arguments: &[String],
) -> Result<DomValue, String> {
    let node = node_id(argument(arguments, 0)?)?;
    match operation {
        "eventPath" => document.event_path(node).map(DomValue::NodeIds),
        "closest" => document
            .closest_selector(node, argument(arguments, 1)?)?
            .map(DomValue::NodeId)
            .map_or(Ok(DomValue::Null), Ok),
        _ => Err(format!("unsupported path lookup operation: {operation}")),
    }
}

fn resource_presence(
    document: &crate::renderer::backends::html_document::HtmlDocument,
) -> Result<DomValue, String> {
    let (images, frames) = document.resource_element_presence();
    Ok(DomValue::String(format!(
        "{}:{}",
        images as u8, frames as u8
    )))
}

#[cfg(test)]
#[path = "dom_state_content_lookup_tests.rs"]
mod tests;
