use super::{
    HtmlDocument,
    form_test_support::{append_nested_select, append_selected_option, node_id},
};

const FORM_DEFAULTS: &str = "<input id=checkbox type=checkbox><input id=radio type=RADIO><input id=text type=text><select><option id=plain>Fallback label</option></select>";
const SELECT_OPTIONS: &str = "<select id=choice><optgroup id=group><option id=first value=duplicate>First</option><option id=second value=duplicate selected>Second</option><option id=other value=other>Other</option></optgroup></select>";

#[test]
fn form_values_use_input_defaults_and_option_text() -> Result<(), String> {
    let mut document = HtmlDocument::parse(FORM_DEFAULTS);
    let cases = [
        ("checkbox", "on"),
        ("radio", "on"),
        ("text", ""),
        ("plain", "Fallback label"),
    ];

    for (id, expected) in cases {
        let node = node_id(&mut document, id)?;
        assert_eq!(document.form_value(node)?, Some(expected.to_string()));
    }
    Ok(())
}

#[test]
fn setting_select_value_chooses_only_the_first_matching_optgroup_option() -> Result<(), String> {
    let mut document = HtmlDocument::parse(SELECT_OPTIONS);
    let select = node_id(&mut document, "choice")?;
    let first = node_id(&mut document, "first")?;
    let second = node_id(&mut document, "second")?;

    document.set_select_value(select, "duplicate")?;

    assert_eq!(document.form_value(select)?, Some("duplicate".to_string()));
    assert_eq!(document.attribute(first, "selected")?, Some(String::new()));
    assert_eq!(document.attribute(second, "selected")?, None);
    Ok(())
}

#[test]
fn unmatched_select_value_stays_empty_until_dom_mutation() -> Result<(), String> {
    let mut document = HtmlDocument::parse(SELECT_OPTIONS);
    let select = node_id(&mut document, "choice")?;
    let group = node_id(&mut document, "group")?;

    document.set_select_value(select, "missing")?;
    assert_eq!(document.form_value(select)?, Some(String::new()));
    append_selected_option(&mut document, group)?;
    assert_eq!(document.form_value(select)?, Some("new-value".to_string()));
    Ok(())
}

#[test]
fn nested_select_options_do_not_become_outer_select_values() -> Result<(), String> {
    let mut document = HtmlDocument::parse(SELECT_OPTIONS);
    let select = node_id(&mut document, "choice")?;
    let group = node_id(&mut document, "group")?;
    let selected = node_id(&mut document, "second")?;
    document.remove_attribute(selected, "selected")?;
    append_nested_select(&mut document, group)?;

    assert_eq!(document.form_value(select)?, Some("duplicate".to_string()));
    Ok(())
}
