use super::HtmlDocument;
use crate::renderer::backends::html_dom_helpers::{attribute_value, detach, text_content};
use html5ever::{Attribute, QualName, ns};
use markup5ever_rcdom::{Node, NodeData};
use std::rc::Rc;

impl HtmlDocument {
    pub(in crate::renderer::backends) fn create_element(
        &mut self,
        tag: &str,
    ) -> Result<u64, String> {
        let tag = tag.trim().to_ascii_lowercase();
        if tag.is_empty()
            || !tag
                .chars()
                .all(|character| character.is_ascii_alphanumeric())
        {
            return Err(format!("unsupported element name: {tag}"));
        }
        let node = Node::new(NodeData::Element {
            name: QualName::new(None, ns!(html), tag.into()),
            attrs: Default::default(),
            template_contents: Default::default(),
            mathml_annotation_xml_integration_point: false,
        });
        Ok(self.register_subtree(&node))
    }

    pub(in crate::renderer::backends) fn append_child(
        &mut self,
        parent_id: u64,
        child_id: u64,
    ) -> Result<(), String> {
        let parent = self.node(parent_id)?;
        let child = self.node(child_id)?;
        if !matches!(parent.data, NodeData::Document | NodeData::Element { .. }) {
            return Err("appendChild target is not a container".to_string());
        }
        detach(&child);
        child.parent.set(Some(Rc::downgrade(&parent)));
        parent.children.borrow_mut().push(child);
        Ok(())
    }

    pub(in crate::renderer::backends) fn remove(&mut self, node_id: u64) -> Result<(), String> {
        let node = self.node(node_id)?;
        detach(&node);
        Ok(())
    }

    pub(in crate::renderer::backends) fn text_content(
        &self,
        node_id: u64,
    ) -> Result<String, String> {
        self.node(node_id).map(|node| text_content(&node))
    }

    pub(in crate::renderer::backends) fn set_text_content(
        &mut self,
        node_id: u64,
        value: &str,
    ) -> Result<(), String> {
        let node = self.node(node_id)?;
        let children = std::mem::take(&mut *node.children.borrow_mut());
        for child in children {
            child.parent.set(None);
        }
        let text = Node::new(NodeData::Text {
            contents: std::cell::RefCell::new(value.into()),
        });
        text.parent.set(Some(Rc::downgrade(&node)));
        node.children.borrow_mut().push(text.clone());
        self.register_subtree(&text);
        Ok(())
    }

    pub(in crate::renderer::backends) fn attribute(
        &self,
        node_id: u64,
        name: &str,
    ) -> Result<Option<String>, String> {
        let node = self.node(node_id)?;
        let NodeData::Element { attrs, .. } = &node.data else {
            return Err("attribute target is not an element".to_string());
        };
        Ok(attribute_value(&attrs.borrow(), name).map(ToOwned::to_owned))
    }

    pub(in crate::renderer::backends) fn set_attribute(
        &mut self,
        node_id: u64,
        name: &str,
        value: &str,
    ) -> Result<(), String> {
        let node = self.node(node_id)?;
        let NodeData::Element { attrs, .. } = &node.data else {
            return Err("attribute target is not an element".to_string());
        };
        let name = name.trim().to_ascii_lowercase();
        if name.is_empty() {
            return Err("attribute name is empty".to_string());
        }
        let mut attrs = attrs.borrow_mut();
        if let Some(attribute) = attrs
            .iter_mut()
            .find(|attribute| attribute.name.local.as_str() == name)
        {
            attribute.value = value.into();
        } else {
            attrs.push(Attribute {
                name: QualName::new(None, Default::default(), name.into()),
                value: value.into(),
            });
        }
        Ok(())
    }

    pub(in crate::renderer::backends) fn remove_attribute(
        &mut self,
        node_id: u64,
        name: &str,
    ) -> Result<(), String> {
        let node = self.node(node_id)?;
        let NodeData::Element { attrs, .. } = &node.data else {
            return Err("attribute target is not an element".to_string());
        };
        let name = name.trim().to_ascii_lowercase();
        if name.is_empty() {
            return Err("attribute name is empty".to_string());
        }
        attrs
            .borrow_mut()
            .retain(|attribute| attribute.name.local.as_str() != name);
        Ok(())
    }

    pub(in crate::renderer::backends) fn toggle_boolean_attribute(
        &mut self,
        node_id: u64,
        name: &str,
    ) -> Result<bool, String> {
        let node = self.node(node_id)?;
        let NodeData::Element { attrs, .. } = &node.data else {
            return Err("attribute target is not an element".to_string());
        };
        let name = name.trim().to_ascii_lowercase();
        if name.is_empty() {
            return Err("attribute name is empty".to_string());
        }
        let mut attributes = attrs.borrow_mut();
        if let Some(index) = attributes
            .iter()
            .position(|attribute| attribute.name.local.as_str() == name)
        {
            attributes.remove(index);
            return Ok(false);
        }
        attributes.push(Attribute {
            name: QualName::new(None, Default::default(), name.into()),
            value: String::new().into(),
        });
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::HtmlDocument;

    #[test]
    fn removes_and_toggles_boolean_attributes_without_duplicate_state() -> Result<(), String> {
        let mut document =
            HtmlDocument::parse("<details id=panel open data-state=ready></details>");
        let panel = required_element_id(&mut document, "panel")?;

        document.remove_attribute(panel, "data-state")?;
        assert_eq!(document.attribute(panel, "data-state"), Ok(None),);
        assert_eq!(document.toggle_boolean_attribute(panel, "open"), Ok(false));
        assert_eq!(document.attribute(panel, "open"), Ok(None));
        assert_eq!(document.toggle_boolean_attribute(panel, "open"), Ok(true));
        assert_eq!(document.attribute(panel, "open"), Ok(Some(String::new())));
        Ok(())
    }

    #[test]
    fn attribute_mutation_rejects_text_nodes_and_empty_names() -> Result<(), String> {
        let mut document = HtmlDocument::parse("<p id=target>Visible</p>");
        let target = required_element_id(&mut document, "target")?;
        document.set_text_content(target, "replacement")?;
        let text = document.next_node_id - 1;

        assert_text_node_attributes_are_rejected(&mut document, text);
        assert_empty_attribute_names_are_rejected(&mut document, target);
        assert!(matches!(
            required_element_id(&mut document, "missing"),
            Err(error) if error == "missing element #missing"
        ));
        assert_missing_attribute_nodes_are_rejected(&mut document);
        Ok(())
    }

    fn assert_text_node_attributes_are_rejected(document: &mut HtmlDocument, text: u64) {
        assert!(matches!(
            document.remove_attribute(text, "state"),
            Err(error) if error.contains("not an element")
        ));
        assert!(matches!(
            document.toggle_boolean_attribute(text, "open"),
            Err(error) if error.contains("not an element")
        ));
    }

    fn assert_empty_attribute_names_are_rejected(document: &mut HtmlDocument, target: u64) {
        assert!(matches!(
            document.remove_attribute(target, " "),
            Err(error) if error.contains("attribute name is empty")
        ));
        assert!(matches!(
            document.toggle_boolean_attribute(target, " "),
            Err(error) if error.contains("attribute name is empty")
        ));
    }

    fn required_element_id(document: &mut HtmlDocument, id: &str) -> Result<u64, String> {
        document
            .get_element_by_id(id)
            .ok_or_else(|| format!("missing element #{id}"))
    }

    fn assert_missing_attribute_nodes_are_rejected(document: &mut HtmlDocument) {
        let missing_node_id = document.next_node_id + 1;
        assert!(matches!(
            document.remove_attribute(missing_node_id, "state"),
            Err(error) if error.contains("does not exist")
        ));
        assert!(matches!(
            document.toggle_boolean_attribute(missing_node_id, "open"),
            Err(error) if error.contains("does not exist")
        ));
    }
}
