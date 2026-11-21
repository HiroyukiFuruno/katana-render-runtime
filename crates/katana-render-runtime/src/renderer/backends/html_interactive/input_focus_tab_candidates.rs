use super::super::super::super::html_document::HtmlDocumentNode;
use std::collections::HashSet;

#[path = "input_focus_tab_traversal.rs"]
mod traversal;

#[path = "input_focus_tab_attributes.rs"]
mod attributes;

use attributes::{attribute, has_attribute};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct TabCandidate {
    pub(super) node_id: u64,
    pub(super) tab_index: i32,
    pub(super) document_order: usize,
}

fn candidates(nodes: &[HtmlDocumentNode]) -> Vec<TabCandidate> {
    let mut result = Vec::new();
    let mut order = 0;
    traversal::collect(
        nodes,
        traversal::TraversalState::default(),
        &mut order,
        &mut result,
    );
    result.sort_by_key(|candidate| {
        if candidate.tab_index > 0 {
            (0, candidate.tab_index, candidate.document_order)
        } else if candidate.tab_index == 0 {
            (1, 0, candidate.document_order)
        } else {
            (2, 0, candidate.document_order)
        }
    });
    result
}

fn record_candidate(
    node_id: u64,
    tag: &str,
    attributes: &[(String, String)],
    inherited: traversal::TraversalState,
    current: traversal::TraversalState,
    order: usize,
    result: &mut Vec<TabCandidate>,
) {
    if current.hidden || current.visibility_hidden {
        return;
    }
    if let Some(tab_index) = tab_index(tag, node_id, attributes, inherited) {
        result.push(TabCandidate {
            node_id,
            tab_index,
            document_order: order,
        });
    }
}

fn tab_index(
    tag: &str,
    node_id: u64,
    attributes: &[(String, String)],
    inherited: traversal::TraversalState,
) -> Option<i32> {
    let disabled_control = is_form_control(tag)
        && (has_attribute(attributes, "disabled") || inherited.disabled_fieldset);
    if disabled_control || input_is_hidden(tag, attributes) {
        return None;
    }
    if let Some(value) = attribute(attributes, "tabindex") {
        let Ok(tab_index) = value.trim().parse::<i32>() else {
            return implicit_tab_index(tag, node_id, attributes, inherited);
        };
        return Some(tab_index);
    }
    implicit_tab_index(tag, node_id, attributes, inherited)
}

fn implicit_tab_index(
    tag: &str,
    node_id: u64,
    attributes: &[(String, String)],
    inherited: traversal::TraversalState,
) -> Option<i32> {
    let implicit = match tag {
        "a" | "area" => attribute(attributes, "href").is_some(),
        "input" | "button" | "select" | "textarea" | "iframe" => true,
        "summary" => inherited.summary_node == Some(node_id),
        _ => is_editable(attributes) && !inherited.editable,
    };
    implicit.then_some(0)
}

fn is_form_control(tag: &str) -> bool {
    matches!(tag, "button" | "input" | "select" | "textarea")
}

pub(super) fn is_editable(attributes: &[(String, String)]) -> bool {
    attribute(attributes, "contenteditable").is_some_and(|value| {
        value.trim().is_empty()
            || value.trim().eq_ignore_ascii_case("true")
            || value.trim().eq_ignore_ascii_case("plaintext-only")
    })
}

fn input_is_hidden(tag: &str, attributes: &[(String, String)]) -> bool {
    tag == "input"
        && attribute(attributes, "type").is_some_and(|value| value.eq_ignore_ascii_case("hidden"))
}

pub(in crate::renderer::backends::html_interactive::input_focus) fn focus_candidates(
    nodes: &[HtmlDocumentNode],
) -> (Vec<u64>, HashSet<u64>) {
    let candidates = candidates(nodes);
    let pointer_focusable = candidates
        .iter()
        .map(|candidate| candidate.node_id)
        .collect();
    let tab_order = candidates
        .iter()
        .filter(|candidate| candidate.tab_index >= 0)
        .map(|candidate| candidate.node_id)
        .collect();
    (tab_order, pointer_focusable)
}
