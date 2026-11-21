use super::{HtmlNavigationIntent, HtmlNodeId, HtmlRuntimeError, HtmlRuntimeEventKind};
use crate::renderer::backends::html_document::HtmlDocument;

pub(super) fn scriptless_event_navigation(
    document: &HtmlDocument,
    target: HtmlNodeId,
    kind: HtmlRuntimeEventKind,
) -> Result<Option<HtmlNavigationIntent>, HtmlRuntimeError> {
    if kind == HtmlRuntimeEventKind::Click {
        return navigation_intent(document, target.0);
    }
    document
        .node(target.0)
        .map_err(HtmlRuntimeError::DomBridge)?;
    Ok(None)
}

pub(super) fn navigation_intent(
    document: &HtmlDocument,
    node_id: u64,
) -> Result<Option<HtmlNavigationIntent>, HtmlRuntimeError> {
    document
        .attribute(node_id, "href")
        .map_err(HtmlRuntimeError::DomBridge)
        .map(|href| href.map(|href| HtmlNavigationIntent { href }))
}
