use super::attributes;
use super::sibling_position::ElementSiblingPositions;
use crate::renderer::backends::html_css::HtmlAttributes;
use crate::renderer::backends::html_css_selector::{CssAncestor, CssSelector};
use markup5ever_rcdom::{Handle, NodeData};

pub(super) fn node_selector_details(
    node: &Handle,
    positions: &ElementSiblingPositions,
) -> Option<(String, HtmlAttributes, usize, usize)> {
    let NodeData::Element { name, attrs, .. } = &node.data else {
        return None;
    };
    let tag = name.local.to_string().to_ascii_lowercase();
    let node_attributes = attributes(&attrs.borrow());
    let (_, type_index, type_count) = positions.get(node);
    Some((tag, node_attributes, type_index, type_count))
}

pub(super) fn push_selector_ancestor(
    ancestors: &mut Vec<CssAncestor>,
    tag: &str,
    attributes: &HtmlAttributes,
    sibling_index: usize,
    type_index: usize,
    type_count: usize,
) {
    ancestors.push(CssAncestor::new_at_state_with_type(
        tag,
        attributes,
        sibling_index,
        type_index,
        type_count,
        false,
    ));
}

pub(super) fn selector_matches_node(
    selector: &CssSelector,
    node: &Handle,
    ancestors: &[CssAncestor],
    sibling_index: usize,
    positions: &ElementSiblingPositions,
) -> bool {
    let Some((tag, attributes, type_index, type_count)) = node_selector_details(node, positions)
    else {
        return false;
    };
    selector.matches_at_with_type(
        &tag,
        &attributes,
        ancestors,
        sibling_index,
        type_index,
        type_count,
    )
}
