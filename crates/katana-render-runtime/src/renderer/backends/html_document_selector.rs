use super::attributes;
use crate::renderer::backends::html_css_selector::{CssAncestor, CssSelector};
use markup5ever_rcdom::{Handle, NodeData};

#[path = "html_document_selector_matching.rs"]
mod selector_matching;
#[path = "html_document_sibling_position.rs"]
mod sibling_position;
#[path = "html_document_selector_traversal.rs"]
mod traversal;
#[cfg(test)]
use sibling_position::element_sibling_index;
use sibling_position::{element_sibling_positions, parent};

pub(super) fn find_selector(
    node: &Handle,
    selector: &CssSelector,
    ancestors: &[CssAncestor],
) -> Option<Handle> {
    traversal::SelectorTraversal::find_selector(node, selector, ancestors)
}

pub(super) fn collect_selectors(
    node: &Handle,
    selector: &CssSelector,
    ancestors: &[CssAncestor],
    matches: &mut Vec<Handle>,
) {
    traversal::SelectorTraversal::collect_selectors(node, selector, ancestors, matches);
}

pub(super) fn matches_selector(node: &Handle, selector: &CssSelector) -> bool {
    let NodeData::Element { name, attrs, .. } = &node.data else {
        return false;
    };
    let tag = name.local.to_string().to_ascii_lowercase();
    let attributes = attributes(&attrs.borrow());
    selector.matches_at_with_type(
        &tag,
        &attributes,
        &selector_ancestors(node),
        element_sibling_positions(node).0,
        element_sibling_positions(node).1,
        element_sibling_positions(node).2,
    )
}

fn selector_ancestors(node: &Handle) -> Vec<CssAncestor> {
    let mut current = parent(node);
    let mut ancestors = Vec::new();
    while let Some(node) = current {
        if let NodeData::Element { name, attrs, .. } = &node.data {
            let tag = name.local.to_string().to_ascii_lowercase();
            ancestors.push(CssAncestor::new_at_state_with_type(
                &tag,
                &attributes(&attrs.borrow()),
                element_sibling_positions(&node).0,
                element_sibling_positions(&node).1,
                element_sibling_positions(&node).2,
                false,
            ));
        }
        current = parent(&node);
    }
    ancestors.reverse();
    ancestors
}

pub(super) fn find_descendant_selector(node: &Handle, selector: &CssSelector) -> Option<Handle> {
    for child in node.children.borrow().iter() {
        if matches_selector(child, selector) {
            return Some(child.clone());
        }
        if let Some(found) = find_descendant_selector(child, selector) {
            return Some(found);
        }
    }
    None
}

pub(super) fn collect_descendant_selectors(
    node: &Handle,
    selector: &CssSelector,
    matches: &mut Vec<Handle>,
) {
    for child in node.children.borrow().iter() {
        if matches_selector(child, selector) {
            matches.push(child.clone());
        }
        collect_descendant_selectors(child, selector, matches);
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::html_css_selector::CssSelector;
    use super::super::HtmlDocument;
    use super::{collect_descendant_selectors, find_descendant_selector, find_selector};
    use super::{element_sibling_index, element_sibling_positions, parent};
    use markup5ever_rcdom::NodeData;

    #[test]
    fn detached_child_uses_the_first_sibling_index() -> Result<(), String> {
        let mut document = HtmlDocument::parse("<div><span id=target></span></div>");
        let node_id = document
            .query_selector("#target")
            .ok_or("target node must exist")?;
        let node = document.node(node_id)?;
        let parent = parent(&node).ok_or("target parent must exist")?;
        parent.children.borrow_mut().clear();

        assert_eq!(element_sibling_index(&node), 1);
        Ok(())
    }

    #[test]
    fn find_selector_recurses_to_descendants_without_root_match() -> Result<(), String> {
        let document = HtmlDocument::parse("<main><section><p id=target>Text</p></section></main>");
        let selector = CssSelector::parse("p").ok_or("selector must parse")?;
        let handle = find_selector(&document.document, &selector, &[]);

        assert!(handle.is_some());
        Ok(())
    }

    #[test]
    fn find_descendant_selector_recurses_to_descendant_nodes() {
        let document = HtmlDocument::parse("<main><section><p id=target>Text</p></section></main>");
        let selector = CssSelector::parse("p#target");
        assert!(selector.is_some());
        selector.into_iter().for_each(|selector| {
            assert!(find_descendant_selector(&document.document, &selector).is_some());
        });
    }

    #[test]
    fn type_pseudo_selector_ignores_other_element_tags() {
        let document = HtmlDocument::parse(
            "<main><div id=first></div><span id=target></span><div id=second></div><span id=last></span></main>",
        );
        assert!(
            CssSelector::parse("span:first-of-type").is_some_and(|selector| {
                let mut matches = Vec::new();
                collect_descendant_selectors(&document.document, &selector, &mut matches);
                assert_eq!(matches.len(), 1);
                assert!(matches!(
                    &matches[0].data,
                    NodeData::Element { attrs, .. }
                        if attrs.borrow().iter()
                            .find(|attr| attr.name.local.to_string() == "id")
                            .map(|attr| attr.value.to_string())
                            .as_deref() == Some("target")
                ));
                true
            }),
            "fixture first-of-type selector must parse"
        );
    }

    #[test]
    fn sibling_positions_count_elements_and_same_tag_case_insensitively() -> Result<(), String> {
        let mut document = HtmlDocument::parse(
            "<main><div id=first></div><span id=middle></span><DIV id=last></DIV></main>",
        );
        let first_id = document.query_selector("#first").ok_or("first")?;
        let middle_id = document.query_selector("#middle").ok_or("middle")?;
        let last_id = document.query_selector("#last").ok_or("last")?;
        let first = document.node(first_id)?;
        let middle = document.node(middle_id)?;
        let last = document.node(last_id)?;
        assert_eq!(element_sibling_positions(&first), (1, 1, 2));
        assert_eq!(element_sibling_positions(&middle), (2, 1, 1));
        assert_eq!(element_sibling_positions(&last), (3, 2, 2));
        Ok(())
    }

    #[test]
    fn text_node_sibling_position_uses_element_count_without_a_tag() -> Result<(), String> {
        let mut document = HtmlDocument::parse("<main>text<span id=target></span></main>");
        let target_id = document.query_selector("#target").ok_or("target")?;
        let target = document.node(target_id)?;
        let text = parent(&target)
            .ok_or("parent")?
            .children
            .borrow()
            .first()
            .cloned()
            .ok_or("text")?;
        assert!(matches!(text.data, NodeData::Text { .. }));
        assert_eq!(element_sibling_positions(&text), (1, 1, 1));
        Ok(())
    }
}
