use std::collections::HashMap;
use std::ops::Range;

use resvg::usvg::roxmltree::{Document, Node};

use super::super::source_edits::{SourceEdit, apply_source_edits};
use super::support::{GEOMETRY_PROPERTIES, GeometryPlan, GeometryValue, is_geometry_element};

pub(super) fn apply_geometry_plans(
    document: &Document<'_>,
    plans: HashMap<usize, GeometryPlan>,
) -> Option<String> {
    let source = document.input_text();
    let mut edits = Vec::new();
    for node in document
        .descendants()
        .filter(|node| is_geometry_element(*node))
    {
        let Some(plan) = plans.get(&node.range().start) else {
            continue;
        };
        collect_attribute_edits(source, node, plan, &mut edits);
    }
    (!edits.is_empty()).then(|| apply_source_edits(source, edits, true))
}

fn collect_attribute_edits(
    source: &str,
    node: Node<'_, '_>,
    plan: &GeometryPlan,
    edits: &mut Vec<SourceEdit>,
) {
    let mut insertions = String::new();
    for property in GEOMETRY_PROPERTIES {
        let Some(value) = plan.value(property) else {
            continue;
        };
        collect_property_edit(node, property, value, &mut insertions, edits);
    }
    if !insertions.is_empty()
        && let Some(position) = opening_tag_end(source, node)
    {
        push_edit(edits, position..position, insertions);
    }
}

fn collect_property_edit(
    node: Node<'_, '_>,
    property: &str,
    value: &GeometryValue,
    insertions: &mut String,
    edits: &mut Vec<SourceEdit>,
) {
    match (node.attribute_node(property), value) {
        (Some(attribute), GeometryValue::Length(value)) => {
            push_edit(edits, attribute.range_value(), value.to_owned());
        }
        (Some(attribute), GeometryValue::Auto) => {
            push_edit(
                edits,
                remove_preceding_space(attribute.range()),
                String::new(),
            );
        }
        (None, GeometryValue::Length(value)) => {
            insertions.push_str(&format!(" {property}=\"{value}\""));
        }
        (None, GeometryValue::Auto) => {}
    }
}

fn remove_preceding_space(range: Range<usize>) -> Range<usize> {
    /* WHY: XMLの属性範囲は属性名から始まり、その直前は区切りの空白。 */
    range.start.saturating_sub(1)..range.end
}

fn opening_tag_end(source: &str, node: Node<'_, '_>) -> Option<usize> {
    let start = node.range().start;
    let bytes = source.as_bytes();
    let mut quote = None;
    for (offset, byte) in bytes.get(start..)?.iter().enumerate() {
        match (quote, byte) {
            (Some(current), value) if current == *value => quote = None,
            (None, b'\'' | b'"') => quote = Some(*byte),
            (None, b'>') => {
                let end = start + offset;
                return Some(if bytes.get(end.saturating_sub(1)) == Some(&b'/') {
                    end - 1
                } else {
                    end
                });
            }
            _ => {}
        }
    }
    None
}

fn push_edit(edits: &mut Vec<SourceEdit>, range: Range<usize>, replacement: String) {
    edits.push(SourceEdit::new(range, replacement));
}
