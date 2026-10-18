use super::HtmlDocument;
use crate::renderer::backends::html_dom_helpers::{attribute_value, text_content};
use markup5ever_rcdom::{Handle, NodeData};

impl HtmlDocument {
    pub(crate) fn form_value(&self, node_id: u64) -> Result<Option<String>, String> {
        let node = self.node(node_id)?;
        let NodeData::Element { name, .. } = &node.data else {
            return Err("value target is not an element".to_string());
        };
        if !is_html_element(name) {
            return Ok(None);
        }
        read_form_value(self, node_id, &node, name.local.as_str())
    }

    pub(in crate::renderer::backends) fn set_select_value(
        &mut self,
        node_id: u64,
        value: &str,
    ) -> Result<(), String> {
        let select = self.node(node_id)?;
        let NodeData::Element { name, .. } = &select.data else {
            return Err("value target is not an element".to_string());
        };
        if !is_html_element(name) || name.local.as_str() != "select" {
            return Err("select value target is not an HTML select".to_string());
        }
        let matched = self.update_select_options(select_options(&select), value)?;
        if matched {
            self.select_value_empty_overrides.remove(&node_id);
        } else {
            self.select_value_empty_overrides.insert(node_id);
        }
        Ok(())
    }

    fn update_select_options(&mut self, options: Vec<Handle>, value: &str) -> Result<bool, String> {
        let mut matched = false;
        for option in options {
            let option_value = option_property_value(&option);
            let option_id = self.register_subtree(&option);
            if !matched && option_value == value {
                self.set_attribute(option_id, "selected", "")?;
                matched = true;
            } else {
                self.remove_attribute(option_id, "selected")?;
            }
        }
        Ok(matched)
    }

    pub(crate) fn clear_select_value_override_for(&mut self, node_id: u64) {
        let Some(mut node) = self.nodes.get(&node_id).cloned() else {
            return;
        };
        loop {
            let node_pointer = std::rc::Rc::as_ptr(&node) as usize;
            if let Some(select_id) = self.node_ids.get(&node_pointer)
                && is_select_element(&node)
            {
                self.select_value_empty_overrides.remove(select_id);
            }
            let parent = node.parent.take();
            node.parent.set(parent.clone());
            let Some(parent) = parent.and_then(|parent| parent.upgrade()) else {
                return;
            };
            node = parent;
        }
    }
}

fn read_form_value(
    document: &HtmlDocument,
    node_id: u64,
    node: &Handle,
    tag: &str,
) -> Result<Option<String>, String> {
    let value = match tag {
        "input" => input_value(document, node_id)?,
        "button" => document.attribute(node_id, "value")?,
        "textarea" => Some(document.text_content(node_id)?),
        "option" => Some(option_property_value(node)),
        "select" => Some(select_document_value(document, node_id, node)),
        _ => return Ok(None),
    };
    let Some(value) = value else {
        return Ok(Some(String::new()));
    };
    Ok(Some(value))
}

fn input_value(document: &HtmlDocument, node_id: u64) -> Result<Option<String>, String> {
    if let Some(value) = document.attribute(node_id, "value")? {
        return Ok(Some(value));
    }
    let default = match document.attribute(node_id, "type")?.as_deref() {
        Some(kind)
            if kind.eq_ignore_ascii_case("checkbox") || kind.eq_ignore_ascii_case("radio") =>
        {
            "on"
        }
        _ => "",
    };
    Ok(Some(default.to_string()))
}

fn select_document_value(document: &HtmlDocument, node_id: u64, node: &Handle) -> String {
    if document.select_value_empty_overrides.contains(&node_id) {
        String::new()
    } else {
        select_value(node)
    }
}

fn select_value(select: &Handle) -> String {
    let mut selected = None;
    let mut first = None;
    for option in select_options(select) {
        let value = option_property_value(&option);
        if first.is_none() {
            first = Some(value.clone());
        }
        if selected.is_none() && option_is_selected(&option) {
            selected = Some(value);
        }
    }
    let Some(value) = selected.or(first) else {
        return String::new();
    };
    value
}

fn select_options(select: &Handle) -> Vec<Handle> {
    fn visit(node: &Handle, options: &mut Vec<Handle>) {
        if let NodeData::Element { name, .. } = &node.data {
            if name.local.as_str() == "select" {
                return;
            }
            if name.local.as_str() == "option" {
                options.push(node.clone());
                return;
            }
        }
        for child in node.children.borrow().iter() {
            visit(child, options);
        }
    }

    let mut options = Vec::new();
    for child in select.children.borrow().iter() {
        visit(child, &mut options);
    }
    options
}

fn option_property_value(option: &Handle) -> String {
    let NodeData::Element { attrs, .. } = &option.data else {
        return String::new();
    };
    attribute_value(&attrs.borrow(), "value")
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| text_content(option))
}

fn option_is_selected(option: &Handle) -> bool {
    let NodeData::Element { attrs, .. } = &option.data else {
        return false;
    };
    attrs
        .borrow()
        .iter()
        .any(|attribute| attribute.name.local.as_str() == "selected")
}

fn is_select_element(node: &Handle) -> bool {
    matches!(&node.data, NodeData::Element { name, .. } if is_html_element(name) && name.local.as_str() == "select")
}

pub(super) fn is_html_element(name: &html5ever::QualName) -> bool {
    name.ns.as_str() == "http://www.w3.org/1999/xhtml"
}
