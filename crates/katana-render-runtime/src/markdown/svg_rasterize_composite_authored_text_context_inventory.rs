use std::collections::HashMap;

use resvg::usvg::roxmltree::Node;

pub(super) fn text_occurrences(nodes: &[Node<'_, '_>]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for node in nodes {
        if let Some(text) = source_text(*node) {
            *counts.entry(text).or_insert(0) += 1;
        }
    }
    counts
}

pub(super) fn source_text(root: Node<'_, '_>) -> Option<String> {
    let mut text = String::new();
    for node in root.descendants().filter(|node| node.is_text()) {
        text.push_str(node.text()?);
    }
    Some(text)
}
