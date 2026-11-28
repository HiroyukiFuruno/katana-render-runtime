use markup5ever_rcdom::{Handle, NodeData};
use std::collections::HashMap;

pub(super) fn sibling_type_counts(node: &Handle) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for child in node.children.borrow().iter() {
        if let NodeData::Element { name, .. } = &child.data {
            *counts
                .entry(name.local.to_string().to_ascii_lowercase())
                .or_default() += 1;
        }
    }
    counts
}

pub(super) fn next_sibling_positions(
    child: &Handle,
    sibling_index: &mut usize,
    type_indices: &mut HashMap<String, usize>,
    type_counts: &HashMap<String, usize>,
) -> (usize, usize, usize) {
    if matches!(child.data, NodeData::Element { .. }) {
        *sibling_index += 1;
    }
    if let NodeData::Element { name, .. } = &child.data {
        let tag = name.local.to_string().to_ascii_lowercase();
        let index = type_indices
            .entry(tag.clone())
            .and_modify(|v| *v += 1)
            .or_insert(1);
        (*sibling_index, *index, type_counts[&tag])
    } else {
        (*sibling_index, 1, 1)
    }
}
