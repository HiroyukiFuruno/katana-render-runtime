use resvg::usvg::{self, Node};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(super) struct TreeIds {
    pub(super) counts: HashMap<String, Vec<()>>,
    pub(super) text_ids: HashSet<String>,
    pub(super) has_unregistered_text: bool,
}

pub(super) fn tree_ids(tree: &usvg::Tree) -> TreeIds {
    let mut ids = TreeIds::default();
    collect_group_ids(tree.root(), &mut ids);
    ids
}

fn collect_group_ids(group: &usvg::Group, ids: &mut TreeIds) {
    record_tree_id(group.id(), false, ids);
    for node in group.children() {
        collect_node_ids(node, ids);
    }
}

fn collect_node_ids(node: &Node, ids: &mut TreeIds) {
    match node {
        Node::Group(group) => {
            record_tree_id(group.id(), false, ids);
            for child in group.children() {
                collect_node_ids(child, ids);
            }
        }
        Node::Text(text) => record_tree_id(text.id(), true, ids),
        Node::Path(path) => record_tree_id(path.id(), false, ids),
        Node::Image(image) => record_tree_id(image.id(), false, ids),
    }
    /* WHY: Textのsubrootsは同じIDを持つ内部outlineなので、意味上のTextを重複計上しない。 */
    if !matches!(node, Node::Text(_)) {
        node.subroots(|subroot| collect_group_ids(subroot, ids));
    }
}

fn record_tree_id(id: &str, is_text: bool, ids: &mut TreeIds) {
    if id.is_empty() {
        ids.has_unregistered_text |= is_text;
        return;
    }
    ids.counts.entry(id.to_owned()).or_default().push(());
    if is_text {
        ids.text_ids.insert(id.to_owned());
    }
}
