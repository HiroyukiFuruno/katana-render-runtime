use markup5ever_rcdom::{Handle, NodeData};
use std::collections::HashMap;
use std::rc::Rc;

pub(super) struct ElementSiblingPositions {
    positions: HashMap<usize, (usize, usize, usize)>,
}

impl ElementSiblingPositions {
    pub(super) fn for_subtree(root: &Handle) -> Self {
        let mut positions = HashMap::new();
        collect_positions(root, &mut positions);
        Self { positions }
    }

    pub(super) fn get(&self, node: &Handle) -> (usize, usize, usize) {
        self.positions
            .get(&(Rc::as_ptr(node) as usize))
            .copied()
            .unwrap_or((1, 1, 1))
    }
}

fn collect_positions(node: &Handle, positions: &mut HashMap<usize, (usize, usize, usize)>) {
    let children = node.children.borrow();
    let mut type_counts = HashMap::<String, usize>::new();
    for child in children.iter() {
        if let NodeData::Element { name, .. } = &child.data {
            *type_counts
                .entry(name.local.to_string().to_ascii_lowercase())
                .or_default() += 1;
        }
    }
    let mut sibling_index = 0;
    let mut type_indices = HashMap::<String, usize>::new();
    for child in children.iter() {
        if let NodeData::Element { name, .. } = &child.data {
            sibling_index += 1;
            let tag = name.local.to_string().to_ascii_lowercase();
            let type_index = type_indices
                .entry(tag.clone())
                .and_modify(|v| *v += 1)
                .or_insert(1);
            positions.insert(
                Rc::as_ptr(child) as usize,
                (sibling_index, *type_index, type_counts[&tag]),
            );
        }
        collect_positions(child, positions);
    }
}

pub(super) fn parent(node: &Handle) -> Option<Handle> {
    let parent = node.parent.take();
    node.parent.set(parent.clone());
    parent.and_then(|parent| parent.upgrade())
}

#[cfg(test)]
pub(super) fn element_sibling_index(node: &Handle) -> usize {
    element_sibling_positions(node).0
}

pub(super) fn element_sibling_positions(node: &Handle) -> (usize, usize, usize) {
    let Some(parent) = parent(node) else {
        return (1, 1, 1);
    };
    let mut index = 0;
    let mut type_index = 0;
    let mut type_count = 0;
    let tag = match &node.data {
        NodeData::Element { name, .. } => name.local.to_string(),
        _ => String::new(),
    };
    for child in parent.children.borrow().iter() {
        type_count += usize::from(same_element_type(child, &tag));
    }
    for child in parent.children.borrow().iter() {
        if matches!(&child.data, NodeData::Element { .. }) {
            index += 1;
            type_index += usize::from(same_element_type(child, &tag));
        }
        if std::rc::Rc::ptr_eq(child, node) {
            return (index.max(1), type_index.max(1), type_count.max(1));
        }
    }
    (1, 1, type_count.max(1))
}

fn same_element_type(node: &Handle, tag: &str) -> bool {
    match &node.data {
        NodeData::Element { name, .. } => name.local.to_string().eq_ignore_ascii_case(tag),
        _ => false,
    }
}
