use super::super::test_support::with_fixture_element;
use super::{DomValue, HtmlDomBridgeState};
use crate::renderer::backends::html_browser::{HtmlBrowserError, HtmlBrowserSource};
use crate::renderer::backends::html_document::HtmlDocument;
use crate::renderer::backends::html_subresources::HtmlSubresourceLoader;

#[path = "dom_state_lookup_children_tests.rs"]
mod children_tests;

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
fn dispatch_lookup_covers_real_selector_child_and_create_operations() {
    let state = HtmlDomBridgeState::new(HtmlDocument::parse(
        "<main id=scope><p class=item>first</p><p class=item>second</p></main>",
    ));
    assert!(matches!(
        state.dispatch("querySelector", &["#scope".to_string()]),
        Ok(DomValue::NodeId(_))
    ));
    assert!(matches!(
        state.dispatch("querySelectorAll", &[".item".to_string()]),
        Ok(DomValue::NodeIds(values)) if values.len() == 2
    ));
    with_fixture_element(&state, "scope", |scope| {
        assert!(matches!(
            state.dispatch("firstElementChild", &[scope]),
            Ok(DomValue::NodeId(_))
        ));
        assert!(matches!(
            state.dispatch("createElement", &["article".to_string()]),
            Ok(DomValue::NodeId(_))
        ));
    });
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
fn lookup_after_real_dom_replacement_reports_removed_selector() {
    let state = HtmlDomBridgeState::new(HtmlDocument::parse(
        "<main><div id=target><span class=old>Text</span></div></main>",
    ));
    with_fixture_element(&state, "target", |target| {
        assert_dom_undefined(state.mutate_content(
            "setInnerHTML",
            &[target, "<span class=new>Updated</span>".to_string()],
        ));
        assert!(matches!(
            state.lookup("querySelector", &[".old".to_string()]),
            Ok(DomValue::Null)
        ));
        assert!(matches!(
            state.lookup("querySelector", &[".new".to_string()]),
            Ok(DomValue::NodeId(_))
        ));
    });
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

#[test]
fn dispatch_lookup_rejects_missing_scoped_selector_arguments() {
    let state = HtmlDomBridgeState::new(HtmlDocument::parse("<main><p>Item</p></main>"));
    assert!(matches!(
        state.dispatch("elementQuerySelector", &[]),
        Err(error) if error == "missing DOM argument 0"
    ));
}

#[test]
fn dispatch_lookup_rejects_non_numeric_scoped_node_ids() {
    let state = HtmlDomBridgeState::new(HtmlDocument::parse("<main><p>Item</p></main>"));
    assert!(matches!(
        state.dispatch(
            "elementQuerySelectorAll",
            &["not-a-node".to_string(), "p".to_string()],
        ),
        Err(error) if error == "invalid HTML node id: not-a-node"
    ));
}

#[test]
fn dispatch_lookup_reports_unknown_numeric_child_node_ids() {
    let state = HtmlDomBridgeState::new(HtmlDocument::parse("<main><p>Item</p></main>"));
    assert!(matches!(
        state.dispatch("firstElementChild", &["999".to_string()]),
        Err(error) if error == "HTML node 999 does not exist"
    ));
}

#[test]
fn lookup_child_and_create_element_operations_use_real_document_nodes() {
    let state = HtmlDomBridgeState::new(HtmlDocument::parse(
        "<main id=scope><span>first</span><p>second</p></main>",
    ));
    with_fixture_element(&state, "scope", |scope| {
        assert!(matches!(
            state.lookup("firstElementChild", std::slice::from_ref(&scope)),
            Ok(DomValue::NodeId(_))
        ));
        assert!(matches!(
            state.lookup("lastElementChild", std::slice::from_ref(&scope)),
            Ok(DomValue::NodeId(_))
        ));
        assert!(matches!(
            state.lookup("createElement", &["article".to_string()]),
            Ok(DomValue::NodeId(_))
        ));
        assert!(matches!(
            state.lookup("elementQuerySelector", &[scope, "p".to_string()]),
            Ok(DomValue::NodeId(_))
        ));
    });
}

#[test]
fn lookup_and_mutation_cover_markup_attribute_path_and_style_contracts() {
    let state = HtmlDomBridgeState::new(HtmlDocument::parse(
        "<main><div id=target data-kind=card><span>Text</span></div></main>",
    ));
    with_fixture_element(&state, "target", |target| {
        assert_markup_lookup(&state, &target);
        mutate_markup_and_style(&state, &target);
    });
}

fn assert_markup_lookup(state: &HtmlDomBridgeState, target: &str) {
    assert_text_and_attribute_values(state, target);
    assert_tag_and_closest_values(state, target);
}

fn assert_text_and_attribute_values(state: &HtmlDomBridgeState, target: &str) {
    assert!(matches!(
        state.lookup("textContent", &[target.to_string()]),
        Ok(DomValue::String(value)) if value == "Text"
    ));
    assert!(matches!(
        state.lookup("getAttribute", &[target.to_string(), "data-kind".to_string()]),
        Ok(DomValue::String(value)) if value == "card"
    ));
}

fn assert_tag_and_closest_values(state: &HtmlDomBridgeState, target: &str) {
    assert!(matches!(
        state.lookup("tagName", &[target.to_string()]),
        Ok(DomValue::String(value)) if value == "DIV"
    ));
    assert!(matches!(
        state.lookup("closest", &[target.to_string(), "main".to_string()]),
        Ok(DomValue::NodeId(_))
    ));
}

#[path = "dom_state_lookup_property_tests.rs"]
mod property_tests;

fn mutate_markup_and_style(state: &HtmlDomBridgeState, target: &str) {
    assert_dom_undefined(state.mutate_content(
        "setInnerHTML",
        &[target.to_string(), "<b>Updated</b>".to_string()],
    ));
    assert_dom_undefined(state.set_attribute(
        "setAttribute",
        &[
            target.to_string(),
            "style".to_string(),
            "color: red".to_string(),
        ],
    ));
    assert_dom_undefined(state.style(
        "styleSet",
        &[
            target.to_string(),
            "backgroundColor".to_string(),
            "blue".to_string(),
        ],
    ));
    assert!(matches!(
        state.style("styleGet", &[target.to_string(), "background-color".to_string()]),
        Ok(DomValue::String(value)) if value == "blue"
    ));
}

fn assert_dom_undefined(result: Result<DomValue, String>) {
    assert!(matches!(result, Ok(DomValue::Undefined)));
}

#[test]
fn loader_image_results_are_reused_by_the_native_bridge_for_exact_sources() {
    assert!(verify_loader_image_cache("https://example.test/index.html").is_ok());
    assert!(matches!(
        verify_loader_image_cache("not-an-origin"),
        Err(HtmlBrowserError::InvalidOrigin { .. })
    ));
}

fn verify_loader_image_cache(origin: &str) -> Result<(), HtmlBrowserError> {
    let valid = "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///ywAAAAAAQABAAACAUwAOw==";
    let invalid = "data:image/gif;base64,not-valid";
    let source = HtmlBrowserSource::new(
        format!("<img src=\"{valid}\"><img src=\"{invalid}\">"),
        origin,
    )?;
    let loader = HtmlSubresourceLoader::new(&source);
    let mut document = HtmlDocument::parse(&source.raw_html);
    assert_eq!(document.prevalidated_image_event(valid), None);

    loader.load(&mut document);
    assert_loader_cache(&document, valid, invalid);
    let state = HtmlDomBridgeState::new(document);

    assert_image_event(&state, valid, "load");
    assert_image_event(&state, invalid, "error");
    assert_image_event(&state, "https://example.test/changed.png", "error");

    let mut document = state.document.borrow_mut();
    document.clear_prevalidated_image_events();
    assert_eq!(document.prevalidated_image_event(valid), None);

    Ok(())
}

fn assert_loader_cache(document: &HtmlDocument, valid: &str, invalid: &str) {
    assert_eq!(document.prevalidated_image_event(valid), Some("load"));
    assert_eq!(document.prevalidated_image_event(invalid), Some("error"));
    assert_eq!(
        document.prevalidated_image_event("https://example.test/changed.png"),
        None
    );
}

fn assert_image_event(state: &HtmlDomBridgeState, source: &str, expected: &str) {
    assert!(matches!(
        state.dispatch("validateImageDataUrl", &[source.to_string()]),
        Ok(DomValue::String(event)) if event == expected
    ));
}
