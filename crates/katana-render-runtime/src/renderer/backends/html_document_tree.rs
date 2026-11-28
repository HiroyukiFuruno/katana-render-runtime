use super::{HtmlDocument, selector};
use markup5ever_rcdom::{Handle, NodeData};
use std::rc::Rc;

impl HtmlDocument {
    pub(crate) fn element_children(&mut self, node_id: u64) -> Result<Vec<u64>, String> {
        let node = self.node(node_id)?;
        if !matches!(node.data, NodeData::Element { .. }) {
            return Err("element child target is not an element".to_string());
        }
        let children = node
            .children
            .borrow()
            .iter()
            .filter(|child| matches!(child.data, NodeData::Element { .. }))
            .cloned()
            .collect::<Vec<_>>();
        Ok(children
            .iter()
            .map(|child| self.register_node(child))
            .collect())
    }

    pub(crate) fn element_child(
        &mut self,
        node_id: u64,
        first: bool,
    ) -> Result<Option<u64>, String> {
        let node = self.node(node_id)?;
        if !matches!(node.data, NodeData::Element { .. }) {
            return Err("element child target is not an element".to_string());
        }
        let children = node.children.borrow();
        let child = if first {
            children
                .iter()
                .find(|child| matches!(child.data, NodeData::Element { .. }))
        } else {
            children
                .iter()
                .rev()
                .find(|child| matches!(child.data, NodeData::Element { .. }))
        }
        .cloned();
        drop(children);
        Ok(child.map(|child| self.register_subtree(&child)))
    }

    pub(crate) fn closest_selector(
        &self,
        node_id: u64,
        selector: &str,
    ) -> Result<Option<u64>, String> {
        let Some(selector) = super::super::html_css_selector::CssSelector::parse(selector) else {
            return Ok(None);
        };
        let mut current = Some(self.node(node_id)?);
        while let Some(node) = current {
            if selector::matches_selector(&node, &selector) {
                let pointer = Rc::as_ptr(&node) as usize;
                return Ok(self.node_ids.get(&pointer).copied());
            }
            let parent = node.parent.take();
            node.parent.set(parent.clone());
            current = parent.and_then(|parent| parent.upgrade());
        }
        Ok(None)
    }

    pub(crate) fn node(&self, node_id: u64) -> Result<Handle, String> {
        self.nodes
            .get(&node_id)
            .cloned()
            .ok_or_else(|| format!("HTML node {node_id} does not exist"))
    }

    pub(in crate::renderer::backends) fn node_id_for_handle(&self, node: &Handle) -> Option<u64> {
        self.node_ids.get(&(Rc::as_ptr(node) as usize)).copied()
    }

    pub(crate) fn event_path(&self, node_id: u64) -> Result<Vec<u64>, String> {
        let mut current = Some(self.node(node_id)?);
        let mut path = Vec::new();
        while let Some(node) = current {
            if matches!(node.data, NodeData::Element { .. }) {
                let pointer = Rc::as_ptr(&node) as usize;
                path.extend(self.node_ids.get(&pointer).copied());
            }
            let parent = node.parent.take();
            node.parent.set(parent.clone());
            current = parent.and_then(|parent| parent.upgrade());
        }
        Ok(path)
    }

    pub(crate) fn register_subtree(&mut self, node: &Handle) -> u64 {
        let node_id = self.register_node(node);
        let children = node.children.borrow().clone();
        for child in children {
            self.register_subtree(&child);
        }
        node_id
    }

    fn register_node(&mut self, node: &Handle) -> u64 {
        let pointer = Rc::as_ptr(node) as usize;
        match self.node_ids.get(&pointer) {
            Some(node_id) => *node_id,
            None => {
                let node_id = self.next_node_id;
                self.next_node_id += 1;
                self.node_ids.insert(pointer, node_id);
                self.nodes.insert(node_id, node.clone());
                node_id
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::html_css_selector::CssSelector;
    use super::super::HtmlDocument;
    use super::selector;
    use html5ever::tendril::Tendril;
    use markup5ever_rcdom::{Node, NodeData};

    fn must_option<T>(value: Option<T>) -> T {
        assert!(value.is_some());
        let mut values = value.into_iter().collect::<Vec<_>>();
        values.remove(0)
    }

    fn must_result<T, E>(value: Result<T, E>) -> T {
        assert!(value.is_ok());
        let mut values = value.into_iter().collect::<Vec<_>>();
        values.remove(0)
    }

    #[test]
    fn non_element_nodes_do_not_match_selector() {
        let text = Node::new(NodeData::Text {
            contents: std::cell::RefCell::new(Tendril::from("text")),
        });
        let selector = must_option(CssSelector::parse("main"));
        assert!(!selector::matches_selector(&text, &selector));
    }

    #[test]
    fn element_child_rejects_non_element_target() -> Result<(), String> {
        let mut document = HtmlDocument::parse("<main><p>body</p></main>");
        let non_element = Node::new(NodeData::Text {
            contents: std::cell::RefCell::new(Tendril::from("child")),
        });
        let node_id = document.register_subtree(&non_element);

        let result = document.element_child(node_id, true);
        assert!(matches!(
            result,
            Err(message) if message == "element child target is not an element"
        ));
        Ok(())
    }

    #[test]
    fn event_path_is_empty_for_text_nodes() {
        let text = Node::new(NodeData::Text {
            contents: std::cell::RefCell::new(Tendril::from("text")),
        });
        let mut document = HtmlDocument::parse("<main><p>body</p></main>");
        let text_node_id = document.register_subtree(&text);

        assert!(
            document
                .event_path(text_node_id)
                .is_ok_and(|path| path.is_empty())
        );
    }

    #[test]
    fn event_path_includes_element_and_ancestors() {
        let mut document = HtmlDocument::parse("<main><p id=inner>body</p></main>");
        let inner = must_option(document.get_element_by_id("inner"));
        let path = must_result(document.event_path(inner));

        assert_eq!(path.first().copied(), Some(inner));
        assert!(path.len() > 1);
    }
}
