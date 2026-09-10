use super::{HtmlDomBridgeState, argument, node_id};
use crate::renderer::backends::html_document::HtmlDocument;
use crate::renderer::backends::html_runtime::types::DomValue;

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
        match operation {
            "getElementById"
            | "querySelector"
            | "querySelectorAll"
            | "createElement"
            | "elementQuerySelector"
            | "elementQuerySelectorAll"
            | "firstElementChild"
            | "lastElementChild"
            | "textContent"
            | "innerHTML"
            | "outerHTML"
            | "getAttribute"
            | "boundingClientRect"
            | "layoutMetrics"
            | "eventPath"
            | "closest" => Some(self.lookup(operation, arguments)),
            _ => None,
        }
    }

    fn dispatch_tree_mutation<'a>(
        &self,
        operation: &'a str,
        arguments: &'a [String],
    ) -> Option<Result<DomValue, String>> {
        match operation {
            "appendChild" | "remove" | "insertAdjacentHTML" => {
                Some(self.mutate_tree(operation, arguments))
            }
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
            _ => None,
        }
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
            | "createElement" => self.lookup_node(operation, arguments),
            _ => self.lookup_content(operation, arguments),
        }
    }

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

#[cfg(test)]
mod tests {
    use super::{DomValue, HtmlDomBridgeState};
    use crate::renderer::backends::html_document::HtmlDocument;

    #[test]
    fn lookup_element_query_selector_all_routes_to_parent_matches() {
        let state = HtmlDomBridgeState::new(HtmlDocument::parse(
            "<div id=scope><p class=item>first</p><p class=item>second</p></div>",
        ));
        let root = state
            .document
            .borrow_mut()
            .get_element_by_id("scope")
            .map(|root| root.to_string());

        assert!(root.is_some_and(|root| matches!(
            state.lookup("elementQuerySelectorAll", &[root, ".item".to_string()]),
            Ok(DomValue::NodeIds(values)) if values.len() == 2
        )));
    }

    #[test]
    fn lookup_query_selector_returns_null_when_not_found_or_syntax_invalid() {
        let state = HtmlDomBridgeState::new(HtmlDocument::parse(
            "<main><p id=\"target\">Item</p></main>",
        ));
        assert!(matches!(
            state.lookup("querySelector", &["main +".to_string()]),
            Ok(DomValue::Null)
        ));

        assert!(matches!(
            state.lookup("querySelector", &["#missing".to_string()]),
            Ok(DomValue::Null)
        ));
    }

    #[test]
    fn lookup_element_query_selector_all_reports_missing_parent_as_error() {
        let state = HtmlDomBridgeState::new(HtmlDocument::parse(
            "<main><p class=\"item\">Item</p></main>",
        ));

        assert!(matches!(
            state.lookup(
                "elementQuerySelectorAll",
                &["999".to_string(), ".item".to_string()],
            ),
            Err(error) if error.contains("HTML node 999")
        ));
    }
}
