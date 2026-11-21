use super::selector_matching::{
    node_selector_details, push_selector_ancestor, selector_matches_node,
};
use super::sibling_position::ElementSiblingPositions;
use crate::renderer::backends::html_css_selector::{CssAncestor, CssSelector};
use markup5ever_rcdom::{Handle, NodeData};

pub(super) struct SelectorTraversal {
    positions: ElementSiblingPositions,
}

impl SelectorTraversal {
    pub(super) fn find_selector(
        node: &Handle,
        selector: &CssSelector,
        ancestors: &[CssAncestor],
    ) -> Option<Handle> {
        let positions = ElementSiblingPositions::for_subtree(node);
        Self { positions }.find_selector_at(node, selector, ancestors, 1)
    }

    fn find_selector_at(
        &self,
        node: &Handle,
        selector: &CssSelector,
        ancestors: &[CssAncestor],
        sibling_index: usize,
    ) -> Option<Handle> {
        let Some(child_ancestors) =
            self.selector_child_ancestors(node, selector, ancestors, sibling_index)
        else {
            return Some(node.clone());
        };
        self.find_selector_children(node, selector, &child_ancestors)
    }

    fn selector_child_ancestors(
        &self,
        node: &Handle,
        selector: &CssSelector,
        ancestors: &[CssAncestor],
        sibling_index: usize,
    ) -> Option<Vec<CssAncestor>> {
        let mut child_ancestors = ancestors.to_vec();
        if selector_matches_node(selector, node, ancestors, sibling_index, &self.positions) {
            return None;
        }
        let Some((tag, node_attributes, type_index, type_count)) =
            node_selector_details(node, &self.positions)
        else {
            return Some(child_ancestors);
        };
        push_selector_ancestor(
            &mut child_ancestors,
            &tag,
            &node_attributes,
            sibling_index,
            type_index,
            type_count,
        );
        Some(child_ancestors)
    }

    fn find_selector_children(
        &self,
        node: &Handle,
        selector: &CssSelector,
        ancestors: &[CssAncestor],
    ) -> Option<Handle> {
        let mut child_index = 0;
        for child in node.children.borrow().iter() {
            if matches!(&child.data, NodeData::Element { .. }) {
                child_index += 1;
            }
            if let Some(found) = self.find_selector_at(child, selector, ancestors, child_index) {
                return Some(found);
            }
        }
        None
    }

    pub(super) fn collect_selectors(
        node: &Handle,
        selector: &CssSelector,
        ancestors: &[CssAncestor],
        matches: &mut Vec<Handle>,
    ) {
        let traversal = Self {
            positions: ElementSiblingPositions::for_subtree(node),
        };
        traversal.collect_selectors_at(node, selector, ancestors, matches, 1);
    }

    fn collect_selectors_at(
        &self,
        node: &Handle,
        selector: &CssSelector,
        ancestors: &[CssAncestor],
        matches: &mut Vec<Handle>,
        sibling_index: usize,
    ) {
        let child_ancestors = collect_selector_node(
            node,
            selector,
            ancestors,
            sibling_index,
            matches,
            &self.positions,
        );
        let mut child_index = 0;
        for child in node.children.borrow().iter() {
            if matches!(&child.data, NodeData::Element { .. }) {
                child_index += 1;
            }
            self.collect_selectors_at(child, selector, &child_ancestors, matches, child_index);
        }
    }
}

fn collect_selector_node(
    node: &Handle,
    selector: &CssSelector,
    ancestors: &[CssAncestor],
    sibling_index: usize,
    matches: &mut Vec<Handle>,
    positions: &ElementSiblingPositions,
) -> Vec<CssAncestor> {
    let mut child_ancestors = ancestors.to_vec();
    let Some((tag, node_attributes, type_index, type_count)) =
        node_selector_details(node, positions)
    else {
        return child_ancestors;
    };
    if selector_matches_node(selector, node, ancestors, sibling_index, positions) {
        matches.push(node.clone());
    }
    push_selector_ancestor(
        &mut child_ancestors,
        &tag,
        &node_attributes,
        sibling_index,
        type_index,
        type_count,
    );
    child_ancestors
}
