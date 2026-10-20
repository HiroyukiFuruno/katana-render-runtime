use super::HtmlDocument;

pub(super) fn append_selected_option(
    document: &mut HtmlDocument,
    group: u64,
) -> Result<(), String> {
    let option = document.create_element("option")?;
    document.set_attribute(option, "value", "new-value")?;
    document.set_attribute(option, "selected", "")?;
    document.set_text_content(option, "New option")?;
    document.append_child(group, option)
}

pub(super) fn append_nested_select(document: &mut HtmlDocument, group: u64) -> Result<(), String> {
    let select = document.create_element("select")?;
    let option = document.create_element("option")?;
    document.set_attribute(option, "value", "nested-value")?;
    document.set_attribute(option, "selected", "")?;
    document.append_child(select, option)?;
    document.append_child(group, select)
}

pub(super) fn node_id(document: &mut HtmlDocument, id: &str) -> Result<u64, String> {
    document
        .get_element_by_id(id)
        .ok_or_else(|| format!("missing HTML element #{id}"))
}
