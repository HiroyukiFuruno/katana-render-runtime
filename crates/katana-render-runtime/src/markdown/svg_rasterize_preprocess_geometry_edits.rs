use std::collections::HashMap;
use std::ops::Range;

use resvg::usvg::roxmltree::{Document, Node};

use super::super::source_edits::{SourceEdit, apply_source_edits};
use super::support::{GEOMETRY_PROPERTIES, GeometryEdit, GeometryPlan, is_geometry_element};

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
        collect_attribute_edits(node, plan, &mut edits);
    }
    (!edits.is_empty()).then(|| apply_source_edits(source, edits, true))
}

fn collect_attribute_edits(node: Node<'_, '_>, plan: &GeometryPlan, edits: &mut Vec<SourceEdit>) {
    let mut insertions = String::new();
    for property in GEOMETRY_PROPERTIES {
        let Some(value) = plan.value(property) else {
            continue;
        };
        collect_property_edit(node, property, value, &mut insertions, edits);
    }
    if !insertions.is_empty()
        && let Some(position) = opening_tag_end(node)
    {
        push_edit(edits, position..position, insertions);
    }
}

fn collect_property_edit(
    node: Node<'_, '_>,
    property: &str,
    value: &GeometryEdit,
    insertions: &mut String,
    edits: &mut Vec<SourceEdit>,
) {
    match value {
        GeometryEdit::Set(value) => {
            if let Some(attribute) = node.attribute_node(property) {
                push_edit(edits, attribute.range_value(), value.to_owned());
            } else {
                insertions.push_str(&format!(" {property}=\"{value}\""));
            }
        }
        GeometryEdit::Remove(range) => {
            push_edit(edits, remove_preceding_space(range.clone()), String::new());
        }
    }
}

fn remove_preceding_space(range: Range<usize>) -> Range<usize> {
    /* WHY: XMLの属性範囲は属性名から始まり、その直前は区切りの空白。 */
    range.start.saturating_sub(1)..range.end
}

fn opening_tag_end(node: Node<'_, '_>) -> Option<usize> {
    /* WHY: 要素の範囲は同じDocumentが解析した元のバイト列を指す。 */
    let range = node.range();
    let bytes = node.document().input_text().as_bytes();
    let mut quote = None;
    let offset = bytes[range.clone()].iter().position(|&byte| {
        match (quote, byte) {
            (Some(current), value) if current == value => quote = None,
            (None, b'\'' | b'"') => quote = Some(byte),
            _ => {}
        }
        quote.is_none() && byte == b'>'
    })?;
    let end = range.start + offset;
    Some(end - usize::from(bytes[end - 1] == b'/'))
}

fn push_edit(edits: &mut Vec<SourceEdit>, range: Range<usize>, replacement: String) {
    edits.push(SourceEdit::new(range, replacement));
}
