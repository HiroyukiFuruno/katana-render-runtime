use super::html_document::HtmlDocument;
use super::html_document_serialization::{fragment_children, serialize_node};
#[cfg(test)]
use super::html_document_serialization::{serialization_error, utf8_error};
use super::html_dom_helpers::find_element;
use html5ever::serialize::TraversalScope;
use markup5ever_rcdom::{Node, NodeData};
use std::rc::Rc;

type InsertAdjacentContext = (markup5ever_rcdom::Handle, usize);

impl HtmlDocument {
    pub(super) fn inner_html(&self, node_id: u64) -> Result<String, String> {
        serialize_node(&self.node(node_id)?, TraversalScope::ChildrenOnly(None))
    }

    pub(super) fn outer_html(&self, node_id: u64) -> Result<String, String> {
        serialize_node(&self.node(node_id)?, TraversalScope::IncludeNode)
    }

    pub(super) fn set_inner_html(&mut self, node_id: u64, value: &str) -> Result<(), String> {
        let target = self.node(node_id)?;
        let children = fragment_children(&target, value, "innerHTML target")?;
        self.clear_select_value_override_for(node_id);
        let changed = !target.children.borrow().is_empty() || !children.is_empty();
        clear_children(&target);
        self.attach_children_at(&target, 0, children);
        if changed {
            self.advance_tree_revision();
        }
        Ok(())
    }

    pub(super) fn insert_adjacent_html(
        &mut self,
        node_id: u64,
        position: &str,
        value: &str,
    ) -> Result<(), String> {
        let target = self.node(node_id)?;
        if !matches!(&target.data, NodeData::Element { .. }) {
            return Err("insertAdjacentHTML target is not an element".to_string());
        }
        let position = position.trim().to_ascii_lowercase();
        let (container, index) = insert_target(&target, &position)?;
        let children = fragment_children(&container, value, "insertAdjacentHTML container")?;
        let changed = !children.is_empty();
        if changed && let Some(container_id) = self.node_id_for_handle(&container) {
            self.clear_select_value_override_for(container_id);
        }
        self.attach_children_at(&container, index, children);
        if changed {
            self.advance_tree_revision();
        }
        Ok(())
    }

    pub(super) fn insert_adjacent_text(
        &mut self,
        node_id: u64,
        position: &str,
        value: &str,
    ) -> Result<(), String> {
        let target = self.node(node_id)?;
        if !matches!(&target.data, NodeData::Element { .. }) {
            return Err("insertAdjacentText target is not an element".to_string());
        }
        let (container, index) = insert_target(&target, &position.trim().to_ascii_lowercase())?;
        if let Some(container_id) = self.node_id_for_handle(&container) {
            self.clear_select_value_override_for(container_id);
        }
        let text = Node::new(NodeData::Text {
            contents: std::cell::RefCell::new(value.into()),
        });
        self.attach_children_at(&container, index, vec![text]);
        self.advance_tree_revision();
        Ok(())
    }

    fn attach_children_at(
        &mut self,
        target: &markup5ever_rcdom::Handle,
        index: usize,
        children: Vec<markup5ever_rcdom::Handle>,
    ) {
        for (offset, child) in children.into_iter().enumerate() {
            child.parent.set(Some(Rc::downgrade(target)));
            target
                .children
                .borrow_mut()
                .insert(index + offset, child.clone());
            self.register_subtree(&child);
        }
    }
}

fn insert_target(
    target: &markup5ever_rcdom::Handle,
    position: &str,
) -> Result<InsertAdjacentContext, String> {
    let is_after_begin = position == "afterbegin";
    let is_before_end = position == "beforeend";
    if is_after_begin {
        return Ok((target.clone(), 0));
    }
    if is_before_end {
        return Ok((target.clone(), target.children.borrow().len()));
    }
    let is_before_begin = position == "beforebegin";
    if is_before_begin || position == "afterend" {
        let parent = parent(target)
            .ok_or_else(|| format!("insertAdjacentHTML {position} requires a parent"))?;
        if !matches!(&parent.data, NodeData::Element { .. }) {
            return Err(format!(
                "insertAdjacentHTML {position} requires an element parent"
            ));
        }
        return insertion_index(&parent, target, position == "afterend")
            .map(|index| (parent, index));
    }
    Err(format!(
        "unsupported insertAdjacentHTML position: {position}"
    ))
}

fn insertion_index(
    parent: &markup5ever_rcdom::Handle,
    target: &markup5ever_rcdom::Handle,
    after_end: bool,
) -> Result<usize, String> {
    let target_index = parent
        .children
        .borrow()
        .iter()
        .position(|child| Rc::ptr_eq(child, target))
        .ok_or_else(|| "insertAdjacentHTML target is detached".to_string())?;
    Ok(target_index + usize::from(after_end))
}

fn parent(node: &markup5ever_rcdom::Handle) -> Option<markup5ever_rcdom::Handle> {
    let parent = node.parent.take();
    node.parent.set(parent.clone());
    parent.and_then(|parent| parent.upgrade())
}

fn clear_children(target: &markup5ever_rcdom::Handle) {
    let previous = std::mem::take(&mut *target.children.borrow_mut());
    for child in previous {
        child.parent.set(None);
    }
}

#[cfg(test)]
fn fragment_context(
    name: html5ever::QualName,
    attributes: Vec<html5ever::Attribute>,
) -> markup5ever_rcdom::Handle {
    Node::new(NodeData::Element {
        name,
        attrs: std::cell::RefCell::new(attributes),
        template_contents: Default::default(),
        mathml_annotation_xml_integration_point: false,
    })
}

pub(super) fn fragment_root(document: &markup5ever_rcdom::Handle) -> markup5ever_rcdom::Handle {
    if let Some(body) = find_element(document, is_body) {
        return body;
    }
    if let Some(html) = find_element(document, is_html) {
        return html;
    }
    document.clone()
}

fn is_body(tag: &str, _attributes: &[html5ever::Attribute]) -> bool {
    tag == "body"
}

fn is_html(tag: &str, _attributes: &[html5ever::Attribute]) -> bool {
    tag == "html"
}

#[cfg(test)]
#[path = "html_document_fragment_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "html_document_fragment_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "html_document_fragment_boundary_tests.rs"]
mod boundary_tests;
