use super::{PolicyKind, policy::uniform_policy_with_nested_context};
use resvg::usvg::{AlignmentBaseline, DominantBaseline, Node, Text, Tree};

pub(in crate::markdown::svg_rasterize::html_glyph) fn has_nested_middle_candidate(
    tree: &Tree,
) -> bool {
    contains_candidate(tree.root().children())
}

fn contains_candidate(nodes: &[Node]) -> bool {
    nodes.iter().any(|node| match node {
        Node::Group(group) => contains_candidate(group.children()),
        Node::Text(text) => {
            quick_nested_middle_candidate(text)
                && uniform_policy_with_nested_context(text, true)
                    .is_some_and(|policy| policy.kind == PolicyKind::Middle)
        }
        Node::Path(_) | Node::Image(_) => false,
    })
}

fn quick_nested_middle_candidate(text: &Text) -> bool {
    if !text.dy().first().is_some_and(|dy| *dy == 0.0) {
        return false;
    }
    let [chunk] = text.chunks() else { return false };
    chunk.spans().iter().any(|span| {
        matches!(
            span.alignment_baseline(),
            AlignmentBaseline::Auto | AlignmentBaseline::Baseline
        ) && span.dominant_baseline() == DominantBaseline::Middle
    })
}
