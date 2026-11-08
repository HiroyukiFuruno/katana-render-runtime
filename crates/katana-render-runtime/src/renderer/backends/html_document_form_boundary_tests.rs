use super::{HtmlDocument, option_is_selected, option_property_value, select_options};

const ROOT_NODE_ID: u64 = 1;
const MISSING_NODE_ID: u64 = u64::MAX;

#[test]
fn missing_button_value_and_empty_select_produce_empty_dom_values() -> Result<(), String> {
    let mut document =
        HtmlDocument::parse("<button id=button></button><select id=select></select>");
    for id in ["button", "select"] {
        let node_id = document.get_element_by_id(id).ok_or("element must exist")?;
        assert_eq!(document.form_value(node_id)?, Some(String::new()));
    }
    Ok(())
}

#[test]
fn select_reads_the_first_nested_optgroup_option() -> Result<(), String> {
    let mut document = HtmlDocument::parse(
        "<select id=choice><optgroup label=group><option>nested choice</option></optgroup></select>",
    );
    let select = document
        .get_element_by_id("choice")
        .ok_or("select must exist")?;

    assert_eq!(
        document.form_value(select)?,
        Some("nested choice".to_string())
    );
    Ok(())
}

#[test]
fn missing_override_and_document_form_helpers_keep_non_element_boundaries() {
    let mut document = HtmlDocument::parse("<select><option>label</option></select>");
    document.clear_select_value_override_for(MISSING_NODE_ID);
    assert!(document.select_value_empty_overrides.is_empty());
    assert_eq!(option_property_value(&document.document), "");
    assert!(!option_is_selected(&document.document));
    assert!(select_options(&document.document).is_empty());
    assert_eq!(
        document.hidden_property(ROOT_NODE_ID),
        Err("hidden target is not an element".to_owned())
    );
}
