use std::ops::Range;

use cssparser::{Parser, Token};
use resvg::usvg::roxmltree::{Document, Node};

use super::super::escape_xml_attribute;
use super::source_edits::{SourceEdit, apply_source_edits};

const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";

pub(super) fn normalize_inline_style_prefixes(svg: &str) -> Option<String> {
    if !svg.contains("style") || !svg.contains(';') {
        return None;
    }
    let Ok(document) = Document::parse(svg) else {
        return None;
    };
    let edits = document
        .descendants()
        .filter(|node| is_svg_element(*node))
        .filter_map(normalize_style_attribute)
        .collect::<Vec<_>>();
    if edits.is_empty() {
        return None;
    }
    Some(apply_source_edits(svg, edits, false))
}

fn is_svg_element(node: Node<'_, '_>) -> bool {
    node.is_element() && matches!(node.tag_name().namespace(), None | Some(SVG_NAMESPACE))
}

fn normalize_style_attribute(node: Node<'_, '_>) -> Option<SourceEdit> {
    let attribute = node.attribute_node("style")?;
    let normalized = remove_leading_empty_declarations(attribute.value())?;
    Some(SourceEdit::new(
        attribute.range_value(),
        escape_xml_attribute(&normalized),
    ))
}

fn remove_leading_empty_declarations(value: &str) -> Option<String> {
    let mut parser = Parser::new(value);
    let mut semicolons = Vec::new();
    let has_declaration = loop {
        let start = parser.position().byte_index();
        let Ok(token) = parser.next_including_whitespace_and_comments() else {
            break false;
        };
        let token = token.clone();
        let end = parser.position().byte_index();
        match token {
            Token::WhiteSpace(_) | Token::Comment(_) => {}
            Token::Semicolon => semicolons.push(start..end),
            _ => break true,
        }
    };
    if semicolons.is_empty() || !has_declaration {
        return None;
    }
    Some(remove_ranges(value, &semicolons))
}

fn remove_ranges(value: &str, ranges: &[Range<usize>]) -> String {
    let mut normalized = String::with_capacity(value.len() - ranges.len());
    let mut cursor = 0;
    for range in ranges {
        normalized.push_str(&value[cursor..range.start]);
        cursor = range.end;
    }
    normalized.push_str(&value[cursor..]);
    normalized
}

#[cfg(test)]
#[path = "svg_rasterize_preprocess_style_prefix_tests.rs"]
mod tests;
