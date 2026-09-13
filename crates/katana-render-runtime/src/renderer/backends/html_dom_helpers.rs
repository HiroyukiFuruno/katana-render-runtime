use markup5ever_rcdom::{Handle, NodeData};
use std::rc::Rc;

pub(super) fn collect_scripts(node: &Handle, scripts: &mut Vec<String>) -> Result<(), String> {
    if let NodeData::Element { name, attrs, .. } = &node.data
        && name.local.as_str().eq_ignore_ascii_case("script")
    {
        if let Some(source) = attribute_value(&attrs.borrow(), "src") {
            return Err(format!("external script is not supported: {source}"));
        }
        scripts.push(text_content(node));
        return Ok(());
    }
    for child in node.children.borrow().iter() {
        collect_scripts(child, scripts)?;
    }
    Ok(())
}

pub(super) fn find_element(
    node: &Handle,
    predicate: impl Fn(&str, &[html5ever::Attribute]) -> bool + Copy,
) -> Option<Handle> {
    if let NodeData::Element { name, attrs, .. } = &node.data {
        let tag = name.local.to_string().to_ascii_lowercase();
        if predicate(&tag, &attrs.borrow()) {
            return Some(node.clone());
        }
    }
    node.children
        .borrow()
        .iter()
        .find_map(|child| find_element(child, predicate))
}

pub(super) fn attribute_value<'a>(
    attributes: &'a [html5ever::Attribute],
    name: &str,
) -> Option<&'a str> {
    attributes
        .iter()
        .find(|attribute| attribute.name.local.as_str().eq_ignore_ascii_case(name))
        .map(|attribute| attribute.value.as_ref())
}

pub(super) fn text_content(node: &Handle) -> String {
    match &node.data {
        NodeData::Text { contents } => contents.borrow().to_string(),
        _ => node.children.borrow().iter().map(text_content).collect(),
    }
}

pub(super) fn detach(node: &Handle) {
    let Some(parent) = node.parent.take().and_then(|parent| parent.upgrade()) else {
        return;
    };
    parent
        .children
        .borrow_mut()
        .retain(|child| !Rc::ptr_eq(child, node));
}
