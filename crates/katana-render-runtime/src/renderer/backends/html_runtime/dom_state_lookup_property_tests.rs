use super::{DomValue, HtmlDomBridgeState, assert_dom_undefined};
use crate::renderer::backends::html_document::HtmlDocument;

#[test]
fn dom_property_getters_match_html_and_foreign_element_semantics() -> Result<(), String> {
    let state = property_fixture();
    assert_element_names(&state)?;
    assert_form_values(&state)?;
    assert_hidden_values(&state)?;
    Ok(())
}

fn property_fixture() -> HtmlDomBridgeState {
    HtmlDomBridgeState::new(HtmlDocument::parse(
        "<div id=plain><input id=text value='typed'><input id=check type=checkbox><input id=radio type=radio><button id=action value=run>Run</button><textarea id=area>note</textarea><select id=selected><option id=first-option value=first>First</option><option value=chosen selected>Chosen</option></select><select id=default><option>Default text</option><option value=other>Other</option></select><select id=empty></select><span id=concealed hidden></span><svg><linearGradient id=gradient></linearGradient></svg></div>",
    ))
}

fn element_id(state: &HtmlDomBridgeState, id: &str) -> Result<String, String> {
    state
        .document
        .borrow_mut()
        .get_element_by_id(id)
        .map(|node| node.to_string())
        .ok_or_else(|| format!("fixture node {id} exists"))
}

fn assert_element_names(state: &HtmlDomBridgeState) -> Result<(), String> {
    assert_lookup_string(state, "tagName", &element_id(state, "plain")?, "DIV");
    assert_lookup_string(
        state,
        "tagName",
        &element_id(state, "gradient")?,
        "linearGradient",
    );
    Ok(())
}

fn assert_form_values(state: &HtmlDomBridgeState) -> Result<(), String> {
    assert!(matches!(
        state.lookup("value", &[element_id(state, "plain")?]),
        Ok(DomValue::Undefined)
    ));
    assert_text_input_values(state)?;
    assert_select_values(state)?;
    assert_additional_form_values(state)
}

fn assert_additional_form_values(state: &HtmlDomBridgeState) -> Result<(), String> {
    assert_lookup_string(state, "value", &element_id(state, "radio")?, "on");
    assert_lookup_string(state, "value", &element_id(state, "action")?, "run");
    assert_lookup_string(state, "value", &element_id(state, "first-option")?, "first");
    assert!(matches!(
        state.lookup("value", &[element_id(state, "gradient")?]),
        Ok(DomValue::Undefined)
    ));
    Ok(())
}

fn assert_text_input_values(state: &HtmlDomBridgeState) -> Result<(), String> {
    assert_lookup_string(state, "value", &element_id(state, "text")?, "typed");
    assert_lookup_string(state, "value", &element_id(state, "check")?, "on");
    assert_lookup_string(state, "value", &element_id(state, "area")?, "note");
    Ok(())
}

fn assert_select_values(state: &HtmlDomBridgeState) -> Result<(), String> {
    assert_lookup_string(state, "value", &element_id(state, "selected")?, "chosen");
    assert_lookup_string(
        state,
        "value",
        &element_id(state, "default")?,
        "Default text",
    );
    assert_lookup_string(state, "value", &element_id(state, "empty")?, "");
    Ok(())
}

fn assert_hidden_values(state: &HtmlDomBridgeState) -> Result<(), String> {
    assert_lookup_string(state, "hidden", &element_id(state, "concealed")?, "1");
    assert!(matches!(
        state.lookup("hidden", &[element_id(state, "gradient")?]),
        Ok(DomValue::Undefined)
    ));
    Ok(())
}

#[test]
fn active_element_falls_back_to_body_after_real_rcdom_remove() -> Result<(), String> {
    let state = HtmlDomBridgeState::new(HtmlDocument::parse(
        "<body><button id=target>Target</button></body>",
    ));
    let target = element_id(&state, "target")?;
    assert_connected(&state, &target, "1");
    assert_dom_undefined(state.mutate_tree("remove", std::slice::from_ref(&target)));
    assert_connected(&state, &target, "0");
    assert_connected(&state, "999", "0");
    assert_connected(&state, &(u64::MAX.to_string() + "0"), "0");
    Ok(())
}

fn assert_connected(state: &HtmlDomBridgeState, target: &str, expected: &str) {
    assert_lookup_string(state, "isConnected", target, expected);
}

fn assert_lookup_string(state: &HtmlDomBridgeState, operation: &str, target: &str, expected: &str) {
    assert!(matches!(
        state.lookup(operation, &[target.to_owned()]),
        Ok(DomValue::String(value)) if value == expected
    ));
}

#[test]
fn select_value_setter_tracks_selection_and_invalidates_empty_override() -> Result<(), String> {
    let state = HtmlDomBridgeState::new(HtmlDocument::parse(
        "<select id=choice><option value=first>First</option><option value=second>Second</option></select>",
    ));
    let select = element_id(&state, "choice")?;
    assert_initial_selection(&state, &select);
    assert_empty_override(&state, &select);
    replace_options_and_check_selection(&state, &select)?;
    Ok(())
}

fn assert_initial_selection(state: &HtmlDomBridgeState, select: &str) {
    assert_lookup_string(state, "value", select, "first");
    assert_dom_undefined(
        state.dispatch("setSelectValue", &[select.to_owned(), "second".to_owned()]),
    );
    assert_lookup_string(state, "value", select, "second");
}

fn assert_empty_override(state: &HtmlDomBridgeState, select: &str) {
    assert_dom_undefined(
        state.dispatch("setSelectValue", &[select.to_owned(), "missing".to_owned()]),
    );
    assert_lookup_string(state, "value", select, "");
}

fn replace_options_and_check_selection(
    state: &HtmlDomBridgeState,
    select: &str,
) -> Result<(), String> {
    assert_dom_undefined(state.mutate_content(
        "setInnerHTML",
        &[
            select.to_owned(),
            "<option value=next>Next</option>".to_owned(),
        ],
    ));
    assert_lookup_string(state, "value", select, "next");
    assert_selected_attribute_after_empty_override(state, select)?;
    Ok(())
}

fn assert_selected_attribute_after_empty_override(
    state: &HtmlDomBridgeState,
    select: &str,
) -> Result<(), String> {
    assert_dom_undefined(
        state.dispatch("setSelectValue", &[select.to_owned(), "missing".to_owned()]),
    );
    let option = state
        .document
        .borrow_mut()
        .query_selector("option")
        .map(|option| option.to_string())
        .ok_or("replacement option exists")?;
    assert_dom_undefined(state.set_attribute(
        "setAttribute",
        &[option, "selected".to_owned(), String::new()],
    ));
    assert_lookup_string(state, "value", select, "next");
    Ok(())
}
