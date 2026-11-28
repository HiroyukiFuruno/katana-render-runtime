use super::{HtmlDomBridgeState, argument, node_id};
use crate::renderer::backends::html_document::HtmlDocument;
use crate::renderer::backends::html_runtime::types::DomValue;

impl HtmlDomBridgeState {
    pub(super) fn lookup_node(
        &self,
        operation: &str,
        arguments: &[String],
    ) -> Result<DomValue, String> {
        let mut document = self.document.borrow_mut();
        match operation {
            "getElementById" => lookup_element_by_id(&mut document, argument(arguments, 0)?),
            "querySelector" => lookup_query_selector(&mut document, argument(arguments, 0)?),
            "querySelectorAll" => lookup_query_selector_all(&mut document, argument(arguments, 0)?),
            "elementQuerySelector" => lookup_element_query_selector(&mut document, arguments),
            "elementQuerySelectorAll" => {
                lookup_element_query_selector_all(&mut document, arguments)
            }
            "firstElementChild" | "lastElementChild" => {
                lookup_child_node(&mut document, arguments, operation == "firstElementChild")
            }
            "elementChildren" => lookup_element_children(&mut document, arguments),
            "treeRevision" => Ok(DomValue::String(document.tree_revision())),
            "createElement" => create_element(&mut document, argument(arguments, 0)?),
            _ => Err(format!(
                "unsupported HTML node lookup operation: {operation}"
            )),
        }
    }
}

fn lookup_element_by_id(document: &mut HtmlDocument, id: &str) -> Result<DomValue, String> {
    Ok(document
        .get_element_by_id(id)
        .map(DomValue::NodeId)
        .unwrap_or(DomValue::Null))
}

fn lookup_query_selector(document: &mut HtmlDocument, selector: &str) -> Result<DomValue, String> {
    Ok(document
        .query_selector(selector)
        .map(DomValue::NodeId)
        .unwrap_or(DomValue::Null))
}

fn lookup_query_selector_all(
    document: &mut HtmlDocument,
    selector: &str,
) -> Result<DomValue, String> {
    Ok(DomValue::NodeIds(document.query_selector_all(selector)))
}

fn lookup_element_children(
    document: &mut HtmlDocument,
    arguments: &[String],
) -> Result<DomValue, String> {
    Ok(DomValue::NodeIds(
        document.element_children(node_id(argument(arguments, 0)?)?)?,
    ))
}

fn lookup_element_query_selector(
    document: &mut HtmlDocument,
    arguments: &[String],
) -> Result<DomValue, String> {
    Ok(document
        .query_selector_from(node_id(argument(arguments, 0)?)?, argument(arguments, 1)?)?
        .map(DomValue::NodeId)
        .unwrap_or(DomValue::Null))
}

fn lookup_element_query_selector_all(
    document: &mut HtmlDocument,
    arguments: &[String],
) -> Result<DomValue, String> {
    Ok(DomValue::NodeIds(document.query_selector_all_from(
        node_id(argument(arguments, 0)?)?,
        argument(arguments, 1)?,
    )?))
}

fn lookup_child_node(
    document: &mut HtmlDocument,
    arguments: &[String],
    first: bool,
) -> Result<DomValue, String> {
    Ok(document
        .element_child(node_id(argument(arguments, 0)?)?, first)?
        .map(DomValue::NodeId)
        .unwrap_or(DomValue::Null))
}

fn create_element(document: &mut HtmlDocument, tag_name: &str) -> Result<DomValue, String> {
    document.create_element(tag_name).map(DomValue::NodeId)
}
