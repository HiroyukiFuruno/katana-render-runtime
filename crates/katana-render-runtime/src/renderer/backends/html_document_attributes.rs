use super::HtmlDocument;
use crate::renderer::backends::html_dom_helpers::attribute_value;
use html5ever::{Attribute, QualName};
use markup5ever_rcdom::NodeData;

impl HtmlDocument {
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

    fn with_attribute_mutation<T>(
        &self,
        node_id: u64,
        name: &str,
        mutate: impl FnOnce(&std::cell::RefCell<Vec<Attribute>>, String) -> Result<T, String>,
    ) -> Result<T, String> {
        let node = self.node(node_id)?;
        let NodeData::Element { attrs, .. } = &node.data else {
            return Err("attribute target is not an element".to_string());
        };
        let name = name.trim().to_ascii_lowercase();
        if name.is_empty() {
            return Err("attribute name is empty".to_string());
        }
        mutate(attrs, name)
    }

    fn clear_option_value_override_for_attribute(
        &mut self,
        node_id: u64,
        name: &str,
    ) -> Result<(), String> {
        let target = self.node(node_id)?;
        let affects_option_value = matches!(
            name.trim().to_ascii_lowercase().as_str(),
            "selected" | "value"
        ) && matches!(&target.data, NodeData::Element { name, .. } if name.local.as_str() == "option");
        if affects_option_value {
            self.clear_select_value_override_for(node_id);
        }
        Ok(())
    }

    pub(in crate::renderer::backends) fn set_attribute(
        &mut self,
        node_id: u64,
        name: &str,
        value: &str,
    ) -> Result<(), String> {
        self.clear_option_value_override_for_attribute(node_id, name)?;
        self.with_attribute_mutation(node_id, name, |attrs, name| {
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
        })
    }

    pub(in crate::renderer::backends) fn remove_attribute(
        &mut self,
        node_id: u64,
        name: &str,
    ) -> Result<(), String> {
        self.clear_option_value_override_for_attribute(node_id, name)?;
        self.with_attribute_mutation(node_id, name, |attrs, name| {
            attrs
                .borrow_mut()
                .retain(|attribute| attribute.name.local.as_str() != name);
            Ok(())
        })
    }

    pub(in crate::renderer::backends) fn toggle_boolean_attribute(
        &mut self,
        node_id: u64,
        name: &str,
    ) -> Result<bool, String> {
        self.with_attribute_mutation(node_id, name, |attrs, name| {
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
        })
    }
}
