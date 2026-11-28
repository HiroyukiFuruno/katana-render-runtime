use super::super::super::super::super::html_document::HtmlDocumentNode;
use super::attributes::{has_attribute, style_value};
use super::{TabCandidate, is_editable, record_candidate};

#[derive(Clone, Copy, Default)]
pub(super) struct TraversalState {
    pub(super) hidden: bool,
    pub(super) visibility_hidden: bool,
    pub(super) disabled_fieldset: bool,
    pub(super) editable: bool,
    pub(super) summary_node: Option<u64>,
}

pub(super) fn collect(
    nodes: &[HtmlDocumentNode],
    inherited: TraversalState,
    order: &mut usize,
    result: &mut Vec<TabCandidate>,
) {
    for node in nodes {
        collect_node(node, inherited, order, result);
    }
}

fn collect_node(
    node: &HtmlDocumentNode,
    inherited: TraversalState,
    order: &mut usize,
    result: &mut Vec<TabCandidate>,
) {
    let Some(element) = element_node(node) else {
        return;
    };
    *order += 1;
    let child_state = traversal_state(element.tag, element.attributes, inherited);
    record_candidate(
        element.node_id,
        element.tag,
        element.attributes,
        inherited,
        child_state,
        *order,
        result,
    );
    collect_children(element, inherited, child_state, order, result);
}

struct ElementNode<'a> {
    node_id: u64,
    tag: &'a str,
    attributes: &'a [(String, String)],
    children: &'a [HtmlDocumentNode],
}

fn element_node(node: &HtmlDocumentNode) -> Option<ElementNode<'_>> {
    match node {
        HtmlDocumentNode::Element {
            node_id,
            tag,
            attributes,
            children,
        } => Some(ElementNode {
            node_id: *node_id,
            tag,
            attributes,
            children,
        }),
        HtmlDocumentNode::Text(_) => None,
    }
}

fn traversal_state(
    tag: &str,
    attributes: &[(String, String)],
    inherited: TraversalState,
) -> TraversalState {
    let node_hidden = has_attribute(attributes, "hidden")
        || has_attribute(attributes, "inert")
        || style_value(attributes, "display")
            .is_some_and(|value| value.eq_ignore_ascii_case("none"));
    let visibility_hidden = match style_value(attributes, "visibility") {
        Some(value) if value.eq_ignore_ascii_case("hidden") => true,
        Some(value) if value.eq_ignore_ascii_case("visible") => false,
        _ => inherited.visibility_hidden,
    };
    TraversalState {
        hidden: inherited.hidden || node_hidden,
        visibility_hidden,
        disabled_fieldset: inherited.disabled_fieldset
            || (tag == "fieldset" && has_attribute(attributes, "disabled")),
        editable: inherited.editable || is_editable(attributes),
        summary_node: inherited.summary_node,
    }
}

fn collect_children(
    element: ElementNode<'_>,
    inherited: TraversalState,
    child_state: TraversalState,
    order: &mut usize,
    result: &mut Vec<TabCandidate>,
) {
    let exceptions = child_exceptions(element.tag, element.attributes, element.children);
    for child in element.children {
        let state = state_for_child(element.tag, child, inherited, child_state, exceptions);
        collect(std::slice::from_ref(child), state, order, result);
    }
}

#[derive(Clone, Copy)]
struct ChildExceptions {
    closed_details: bool,
    first_legend: Option<u64>,
    first_summary: Option<u64>,
}

fn child_exceptions(
    tag: &str,
    attributes: &[(String, String)],
    children: &[HtmlDocumentNode],
) -> ChildExceptions {
    ChildExceptions {
        closed_details: tag == "details" && !has_attribute(attributes, "open"),
        first_legend: (tag == "fieldset" && has_attribute(attributes, "disabled"))
            .then(|| first_child_tag(children, "legend"))
            .flatten(),
        first_summary: (tag == "details")
            .then(|| first_child_tag(children, "summary"))
            .flatten(),
    }
}

fn state_for_child(
    tag: &str,
    child: &HtmlDocumentNode,
    inherited: TraversalState,
    child_state: TraversalState,
    exceptions: ChildExceptions,
) -> TraversalState {
    let child_is_exception = matches!(child, HtmlDocumentNode::Element { node_id, .. }
        if Some(*node_id) == exceptions.first_legend || Some(*node_id) == exceptions.first_summary);
    let mut state = child_state;
    if exceptions.first_legend.is_some() && child_is_exception {
        state.disabled_fieldset = inherited.disabled_fieldset;
    }
    if exceptions.closed_details && !child_is_exception {
        state.hidden = true;
    }
    if tag == "details"
        && exceptions.first_summary.is_some()
        && element_id(child) == exceptions.first_summary
    {
        state.summary_node = exceptions.first_summary;
    }
    state
}

fn first_child_tag(nodes: &[HtmlDocumentNode], expected: &str) -> Option<u64> {
    nodes.iter().find_map(|node| match node {
        HtmlDocumentNode::Element { node_id, tag, .. } if tag == expected => Some(*node_id),
        _ => None,
    })
}

fn element_id(node: &HtmlDocumentNode) -> Option<u64> {
    match node {
        HtmlDocumentNode::Element { node_id, .. } => Some(*node_id),
        HtmlDocumentNode::Text(_) => None,
    }
}
