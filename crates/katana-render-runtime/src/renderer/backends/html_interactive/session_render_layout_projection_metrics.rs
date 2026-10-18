use super::super::super::html_debug_trace::HtmlDebugTrace;
use super::super::super::html_document::HtmlDocumentNode;

pub(super) fn node_count(nodes: &[HtmlDocumentNode]) -> usize {
    nodes
        .iter()
        .map(|node| match node {
            HtmlDocumentNode::Element { children, .. } => 1 + node_count(children),
            HtmlDocumentNode::Text(_) => 1,
        })
        .sum()
}

pub(super) fn projection_fingerprint(nodes: &[HtmlDocumentNode]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    fn hash_nodes<H: Hasher>(nodes: &[HtmlDocumentNode], hasher: &mut H) {
        nodes.len().hash(hasher);
        for node in nodes {
            match node {
                HtmlDocumentNode::Text(text) => {
                    0_u8.hash(hasher);
                    text.hash(hasher);
                }
                HtmlDocumentNode::Element {
                    node_id,
                    tag,
                    attributes,
                    children,
                } => {
                    1_u8.hash(hasher);
                    node_id.hash(hasher);
                    tag.hash(hasher);
                    attributes.hash(hasher);
                    children.len().hash(hasher);
                    hash_nodes(children, hasher);
                }
            }
        }
    }
    hash_nodes(nodes, &mut hasher);
    hasher.finish()
}

pub(super) fn traced_node_count(trace: &HtmlDebugTrace, nodes: &[HtmlDocumentNode]) -> usize {
    if trace.enabled() {
        node_count(nodes)
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::{HtmlDocumentNode, traced_node_count};
    use crate::renderer::backends::html_debug_trace::HtmlDebugTrace;

    #[test]
    fn debug_node_metric_counts_elements_and_text_recursively() {
        let nodes = vec![HtmlDocumentNode::Element {
            node_id: 1,
            tag: "p".to_string(),
            attributes: Vec::new(),
            children: vec![HtmlDocumentNode::Text("visible".to_string())],
        }];

        assert_eq!(
            traced_node_count(&HtmlDebugTrace::enabled_for_test(), &nodes),
            2
        );
        assert_eq!(traced_node_count(&HtmlDebugTrace::disabled(), &nodes), 0);
    }

    #[test]
    fn projection_fingerprint_includes_dom_structure_and_attributes() {
        let nested = vec![HtmlDocumentNode::Element {
            node_id: 1,
            tag: "div".into(),
            attributes: vec![("class".into(), "a".into())],
            children: vec![HtmlDocumentNode::Text("x".into())],
        }];
        let sibling = vec![
            HtmlDocumentNode::Element {
                node_id: 1,
                tag: "div".into(),
                attributes: vec![("class".into(), "a".into())],
                children: Vec::new(),
            },
            HtmlDocumentNode::Text("x".into()),
        ];
        assert_ne!(
            super::projection_fingerprint(&nested),
            super::projection_fingerprint(&sibling)
        );
    }
}
