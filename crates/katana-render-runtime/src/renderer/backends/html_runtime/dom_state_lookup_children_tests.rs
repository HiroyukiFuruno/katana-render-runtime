use super::{DomValue, HtmlDomBridgeState, assert_dom_undefined};
use crate::renderer::backends::html_document::HtmlDocument;

#[test]
fn element_children_reads_live_element_order_and_revision_after_mutations() -> Result<(), String> {
    let state = HtmlDomBridgeState::new(HtmlDocument::parse(
        "<main id=scope>text<span id=first>one</span><!--comment--><b name=second>two</b></main>",
    ));
    let scope = required_element_id(&state, "scope")?;

    assert!(matches!(
        state.dispatch("treeRevision", &[]),
        Ok(DomValue::String(revision)) if revision == "0"
    ));
    let initial = child_ids(&state, &scope)?;
    assert_eq!(initial.len(), 2);
    let added = create_element_id(&state, "em")?;
    assert_dom_undefined(state.dispatch("appendChild", &[scope.clone(), added.clone()]));
    assert_eq!(
        child_ids(&state, &scope)?,
        [initial[0].clone(), initial[1].clone(), added.clone()]
    );
    assert_revision(&state, "1");
    assert_child_collection_replacements(&state, &scope, &initial, &added)?;
    Ok(())
}

#[test]
fn request_text_rejects_missing_arguments_before_host_io() {
    let state = HtmlDomBridgeState::new(HtmlDocument::parse("<main></main>"));

    assert!(matches!(state.request_text(&[]), Err(error) if error == "missing DOM argument 0"));
    assert!(matches!(
        state.request_text(&["GET".to_string()]),
        Err(error) if error == "missing DOM argument 1"
    ));
    assert!(
        !state
            .host_io_active()
            .load(std::sync::atomic::Ordering::SeqCst)
    );
}

#[test]
fn request_text_rejects_non_get_and_get_without_loader_as_response_values() -> Result<(), String> {
    let state = HtmlDomBridgeState::new(HtmlDocument::parse("<main></main>"));
    for (method, expected_status) in [
        ("POST", "dynamic request method is not allowed: POST"),
        ("GET", "dynamic requests require an interactive session"),
    ] {
        assert_request_status(&state, method, expected_status)?;
    }
    Ok(())
}

fn assert_request_status(
    state: &HtmlDomBridgeState,
    method: &str,
    expected_status: &str,
) -> Result<(), String> {
    let response = state.request_text(&[method.to_string(), "/resource".to_string()])?;
    let DomValue::String(response) = response else {
        return Err("request_text must return a serialized response".to_string());
    };
    let status = response_status(&response)?;
    assert_eq!(status, expected_status);
    assert!(
        !state
            .host_io_active()
            .load(std::sync::atomic::Ordering::SeqCst)
    );
    Ok(())
}

fn response_status(response: &str) -> Result<String, String> {
    serde_json::from_str::<serde_json::Value>(response)
        .map_err(|error| error.to_string())?
        .get("statusText")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "response statusText must be a string".to_string())
}

fn assert_child_collection_replacements(
    state: &HtmlDomBridgeState,
    scope: &str,
    initial: &[String],
    added: &str,
) -> Result<(), String> {
    assert_dom_undefined(state.dispatch("remove", &[initial[0].clone()]));
    assert_eq!(
        child_ids(state, scope)?,
        [initial[1].clone(), added.to_string()]
    );
    assert_revision(state, "2");
    assert_dom_undefined(state.dispatch(
        "setInnerHTML",
        &[scope.to_string(), "<u id=replacement></u>tail".to_string()],
    ));
    assert_eq!(child_ids(state, scope)?.len(), 1);
    assert_revision(state, "3");
    assert_dom_undefined(
        state.dispatch("setTextContent", &[scope.to_string(), "plain".to_string()]),
    );
    assert!(child_ids(state, scope)?.is_empty());
    assert_revision(state, "4");
    Ok(())
}

#[test]
fn element_children_reparent_updates_both_parent_collections() -> Result<(), String> {
    let state = HtmlDomBridgeState::new(HtmlDocument::parse(
        "<main><section id=left><i id=moved></i></section><section id=right></section></main>",
    ));
    let left = required_element_id(&state, "left")?;
    let right = required_element_id(&state, "right")?;
    let moved = required_element_id(&state, "moved")?;

    assert_eq!(child_ids(&state, &left)?, std::slice::from_ref(&moved));
    assert!(child_ids(&state, &right)?.is_empty());
    assert_dom_undefined(state.dispatch("appendChild", &[right.clone(), moved.clone()]));
    assert!(child_ids(&state, &left)?.is_empty());
    assert_eq!(child_ids(&state, &right)?, [moved]);
    Ok(())
}

#[test]
fn element_children_rejects_non_element_and_missing_node() {
    let state = HtmlDomBridgeState::new(HtmlDocument::parse("<main>text</main>"));
    assert!(matches!(
        state.dispatch("elementChildren", &["999".to_string()]),
        Err(error) if error == "HTML node 999 does not exist"
    ));
    let document_id = "1".to_string();
    assert!(matches!(
        state.dispatch("elementChildren", &[document_id]),
        Err(error) if error == "element child target is not an element"
    ));
}

fn child_ids(state: &HtmlDomBridgeState, parent: &str) -> Result<Vec<String>, String> {
    match state.dispatch("elementChildren", &[parent.to_string()]) {
        Ok(DomValue::NodeIds(node_ids)) => {
            Ok(node_ids.into_iter().map(|id| id.to_string()).collect())
        }
        Ok(_) => Err("elementChildren returned a non-node-ID value".to_string()),
        Err(error) => Err(error),
    }
}

fn required_element_id(state: &HtmlDomBridgeState, id: &str) -> Result<String, String> {
    match state.document.borrow_mut().get_element_by_id(id) {
        Some(node_id) => Ok(node_id.to_string()),
        None => Err(format!("fixture element #{id} must exist")),
    }
}

fn create_element_id(state: &HtmlDomBridgeState, tag: &str) -> Result<String, String> {
    match state.dispatch("createElement", &[tag.to_string()]) {
        Ok(DomValue::NodeId(node_id)) => Ok(node_id.to_string()),
        Ok(_) => Err("createElement returned a non-node-ID value".to_string()),
        Err(error) => Err(error),
    }
}

fn assert_revision(state: &HtmlDomBridgeState, expected: &str) {
    assert!(matches!(
        state.dispatch("treeRevision", &[]),
        Ok(DomValue::String(revision)) if revision == expected
    ));
}
