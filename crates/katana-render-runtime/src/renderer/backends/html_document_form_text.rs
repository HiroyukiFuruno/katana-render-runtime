use super::{HtmlDocument, NodeData, is_html_element};

impl HtmlDocument {
    pub(in crate::renderer::backends) fn set_text_control_value(
        &mut self,
        node_id: u64,
        value: &str,
    ) -> Result<(), String> {
        let node = self.node(node_id)?;
        if matches!(&node.data, NodeData::Element { name, .. }
            if is_html_element(name) && name.local.as_str() == "textarea")
        {
            self.textarea_values.insert(node_id, value.to_owned());
            return Ok(());
        }
        self.set_attribute(node_id, "value", value)
    }

    pub(in crate::renderer::backends) fn textarea_value(
        &self,
        node_id: u64,
    ) -> Result<String, String> {
        if let Some(value) = self.textarea_values.get(&node_id) {
            return Ok(value.clone());
        }
        self.text_content(node_id)
    }
}
