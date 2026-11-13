use super::super::html_css_selector::CssSelector;
use super::super::html_dom_helpers::attribute_value;
use super::{HtmlDocument, form, selector};
use markup5ever_rcdom::{Handle, NodeData};

impl HtmlDocument {
    pub(crate) fn anchor_href_attribute(
        &self,
        node_id: u64,
    ) -> Result<Option<Option<String>>, String> {
        let node = self.node(node_id)?;
        let NodeData::Element { name, attrs, .. } = &node.data else {
            return Err("tagName target is not an element".to_string());
        };
        let tag_name = name.local.to_string();
        let tag_name = if form::is_html_element(name) {
            tag_name.to_ascii_uppercase()
        } else {
            tag_name
        };
        if !matches!(tag_name.as_str(), "A" | "AREA") {
            return Ok(None);
        }
        Ok(Some(
            attribute_value(&attrs.borrow(), "href").map(ToOwned::to_owned),
        ))
    }

    pub(crate) fn tag_name(&self, node_id: u64) -> Result<String, String> {
        let node = self.node(node_id)?;
        match &node.data {
            markup5ever_rcdom::NodeData::Element { name, .. } => {
                let local_name = name.local.to_string();
                if form::is_html_element(name) {
                    Ok(local_name.to_ascii_uppercase())
                } else {
                    Ok(local_name)
                }
            }
            _ => Err("tagName target is not an element".to_string()),
        }
    }

    pub(crate) fn hidden_property(&self, node_id: u64) -> Result<Option<bool>, String> {
        let node = self.node(node_id)?;
        let NodeData::Element { name, attrs, .. } = &node.data else {
            return Err("hidden target is not an element".to_string());
        };
        if !form::is_html_element(name) {
            return Ok(None);
        }
        Ok(Some(attrs.borrow().iter().any(|attribute| {
            attribute.name.local.as_str() == "hidden"
        })))
    }

    pub(crate) fn is_connected(&self, node_id: u64) -> bool {
        let Some(mut node) = self.nodes.get(&node_id).cloned() else {
            return false;
        };
        loop {
            if std::rc::Rc::ptr_eq(&node, &self.document) {
                return true;
            }
            let parent = node.parent.take();
            node.parent.set(parent.clone());
            let Some(parent) = parent.and_then(|parent| parent.upgrade()) else {
                return false;
            };
            node = parent;
        }
    }

    pub(in crate::renderer::backends) fn resource_element_presence(&self) -> (bool, bool) {
        fn visit(node: &Handle, images: &mut bool, frames: &mut bool) {
            if let NodeData::Element { name, .. } = &node.data {
                match name.local.as_ref() {
                    "img" => *images = true,
                    "iframe" => *frames = true,
                    _ => {}
                }
            }
            if *images && *frames {
                return;
            }
            for child in node.children.borrow().iter() {
                visit(child, images, frames);
                if *images && *frames {
                    return;
                }
            }
        }

        let mut images = false;
        let mut frames = false;
        visit(&self.document, &mut images, &mut frames);
        (images, frames)
    }

    pub(crate) fn get_element_by_id(&mut self, id: &str) -> Option<u64> {
        let handle =
            super::super::html_dom_helpers::find_element(&self.document, |tag, attributes| {
                tag == "*"
                    || super::super::html_dom_helpers::attribute_value(attributes, "id") == Some(id)
            })?;
        Some(self.register_subtree(&handle))
    }

    pub(crate) fn query_selector(&mut self, selector: &str) -> Option<u64> {
        let selector = CssSelector::parse(selector)?;
        let handle = selector::find_selector(&self.document, &selector, &[])?;
        Some(self.register_subtree(&handle))
    }

    pub(crate) fn query_selector_all(&mut self, selector: &str) -> Vec<u64> {
        let Some(selector) = CssSelector::parse(selector) else {
            return Vec::new();
        };
        let mut handles = Vec::new();
        selector::collect_selectors(&self.document, &selector, &[], &mut handles);
        handles
            .iter()
            .map(|handle| self.register_subtree(handle))
            .collect()
    }

    pub(crate) fn query_selector_from(
        &mut self,
        node_id: u64,
        selector: &str,
    ) -> Result<Option<u64>, String> {
        let node = self.node(node_id)?;
        let Some(selector) = CssSelector::parse(selector) else {
            return Ok(None);
        };
        Ok(selector::find_descendant_selector(&node, &selector)
            .map(|handle| self.register_subtree(&handle)))
    }

    pub(crate) fn query_selector_all_from(
        &mut self,
        node_id: u64,
        selector: &str,
    ) -> Result<Vec<u64>, String> {
        let node = self.node(node_id)?;
        let Some(selector) = CssSelector::parse(selector) else {
            return Ok(Vec::new());
        };
        let mut handles = Vec::new();
        selector::collect_descendant_selectors(&node, &selector, &mut handles);
        Ok(handles
            .iter()
            .map(|handle| self.register_subtree(handle))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::HtmlDocument;

    #[test]
    fn query_selector_returns_none_for_invalid_selector_syntax() -> Result<(), String> {
        let mut document = HtmlDocument::parse("<main><p id=target>Item</p></main>");
        let node = document
            .get_element_by_id("target")
            .ok_or("target node must exist")?;

        assert!(document.query_selector("main +").is_none());
        assert!(document.query_selector_from(node, "main +")?.is_none());
        assert!(document.query_selector_all("main +").is_empty());
        Ok(())
    }

    #[test]
    fn element_only_dom_operations_reject_document_root() {
        let mut document = HtmlDocument::parse("<p>Original</p>");
        assert_eq!(
            document.anchor_href_attribute(1),
            Err("tagName target is not an element".into())
        );
        assert_eq!(
            document.tag_name(1),
            Err("tagName target is not an element".into())
        );
        assert_eq!(
            document.insert_adjacent_text(1, "beforeend", "text"),
            Err("insertAdjacentText target is not an element".into())
        );
        assert_eq!(
            document.insert_adjacent_html(1, "beforeend", "<b>text</b>"),
            Err("insertAdjacentHTML target is not an element".into())
        );
        assert_eq!(
            document.anchor_href_attribute(9999),
            Err("HTML node 9999 does not exist".into())
        );
    }

    #[test]
    fn svg_tag_name_and_hidden_property_keep_namespace_semantics() -> Result<(), String> {
        let mut document = HtmlDocument::parse(
            "<svg><linearGradient id=gradient hidden></linearGradient><a id=svg-anchor href='#target'></a></svg>",
        );
        let gradient = document.get_element_by_id("gradient").ok_or("gradient")?;
        let anchor = document
            .get_element_by_id("svg-anchor")
            .ok_or("svg anchor")?;

        assert_eq!(document.tag_name(gradient)?, "linearGradient");
        assert_eq!(document.hidden_property(gradient)?, None);
        assert_eq!(document.tag_name(anchor)?, "a");
        assert_eq!(document.anchor_href_attribute(anchor)?, None);
        Ok(())
    }
}
