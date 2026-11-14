use super::source_edits::direct_text_ranges;
use std::ops::Range;

use resvg::usvg::roxmltree::{Document, Node};

#[path = "svg_rasterize_preprocess_marker_selector.rs"]
mod selector;
#[path = "svg_rasterize_preprocess_marker_stylesheet.rs"]
mod stylesheet;

const MAX_SVG_BYTES: usize = 72 * 1024;
const MAX_STYLE_BYTES: usize = 48 * 1024;
const MAX_OUTPUT_BYTES: usize = 144 * 1024;
const MAX_DOCUMENT_IDS: usize = 128;
const MAX_SOURCE_EDITS: usize = 256;
const SVG_STYLE_NAMESPACE: &str = "http://www.w3.org/2000/svg";
const STYLE_OPEN: &[u8] = b"<style";
const COLON_STYLE: &[u8] = b":style";

pub(super) fn normalize_marker_selectors(svg_text: String) -> String {
    let bytes = svg_text.as_bytes();
    if svg_text.len() > MAX_SVG_BYTES || (!bytes.contains(&b'$') && !bytes.contains(&b'\\')) {
        return svg_text;
    }
    if !has_style_candidate(bytes) {
        return svg_text;
    }
    let Ok(document) = Document::parse(&svg_text) else {
        return svg_text;
    };
    let Some(ids) = collect_ids(&document) else {
        return svg_text;
    };
    let mut edits = Vec::new();
    let edits_complete = document
        .descendants()
        .filter(|node| node.is_element())
        .all(|node| collect_style_edits(node, &svg_text, &ids, &mut edits));
    drop(document);
    if !edits_complete {
        return svg_text;
    }
    apply_source_edits(svg_text, edits)
}

fn has_style_candidate(bytes: &[u8]) -> bool {
    bytes
        .windows(STYLE_OPEN.len())
        .any(|window| window == STYLE_OPEN)
        || bytes
            .windows(COLON_STYLE.len())
            .any(|window| window == COLON_STYLE)
}

fn collect_ids(document: &Document<'_>) -> Option<Vec<String>> {
    let mut ids = Vec::new();
    for node in document.descendants().filter(|node| node.is_element()) {
        for attribute in node
            .attributes()
            .filter(|attribute| attribute.name() == "id" && attribute.namespace().is_none())
        {
            let value = attribute.value();
            if ids.iter().any(|existing| existing == value) {
                continue;
            }
            if ids.len() >= MAX_DOCUMENT_IDS {
                return None;
            }
            ids.push(value.to_owned());
        }
    }
    Some(ids)
}

fn collect_style_edits(
    node: Node<'_, '_>,
    source: &str,
    ids: &[String],
    edits: &mut Vec<SourceEdit>,
) -> bool {
    let tag = node.tag_name();
    if tag.name() != "style"
        || tag
            .namespace()
            .is_some_and(|namespace| namespace != SVG_STYLE_NAMESPACE)
    {
        return true;
    }
    let Some(style_markup) = source.get(node.range()) else {
        return true;
    };
    if style_markup.contains("<![CDATA[") {
        return true;
    }
    collect_style_text_edits(node, source, ids, edits)
}

fn collect_style_text_edits(
    node: Node<'_, '_>,
    source: &str,
    ids: &[String],
    edits: &mut Vec<SourceEdit>,
) -> bool {
    for (range, text) in direct_text_ranges(node) {
        if text.len() > MAX_STYLE_BYTES || source.get(range.clone()) != Some(text) {
            continue;
        }
        if edits.len() >= MAX_SOURCE_EDITS {
            return false;
        }
        let Some(max_output_bytes) = remaining_source_output_budget(source, edits, range.len())
        else {
            return false;
        };
        let Some(replacement) = stylesheet::rewrite(text, ids, max_output_bytes) else {
            continue;
        };
        if replacement != text {
            edits.push(SourceEdit { range, replacement });
        }
    }
    true
}

fn remaining_source_output_budget(
    source: &str,
    edits: &[SourceEdit],
    range_len: usize,
) -> Option<usize> {
    let current_size = edits.iter().fold(source.len(), |size, edit| {
        size - edit.range.len() + edit.replacement.len()
    });
    MAX_OUTPUT_BYTES
        .checked_sub(current_size)
        .map(|remaining| remaining + range_len)
}

fn apply_source_edits(source: String, mut edits: Vec<SourceEdit>) -> String {
    if edits.is_empty() {
        return source;
    }
    edits.sort_by_key(|edit| edit.range.start);
    if !valid_edits(&source, &edits) {
        return source;
    }
    let output_size = edits.iter().fold(source.len(), |size, edit| {
        size - edit.range.len() + edit.replacement.len()
    });
    if output_size > MAX_OUTPUT_BYTES {
        return source;
    }
    let mut output = source;
    for edit in edits.into_iter().rev() {
        output.replace_range(edit.range, &edit.replacement);
    }
    output
}

fn valid_edits(source: &str, edits: &[SourceEdit]) -> bool {
    edits.len() <= MAX_SOURCE_EDITS
        && edits.iter().all(|edit| {
            edit.range.start < edit.range.end && source.get(edit.range.clone()).is_some()
        })
        && edits
            .windows(2)
            .all(|pair| pair[0].range.end <= pair[1].range.start)
}

struct SourceEdit {
    range: Range<usize>,
    replacement: String,
}

#[cfg(test)]
#[path = "svg_rasterize_preprocess_marker_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "svg_rasterize_preprocess_marker_limits_tests.rs"]
mod limits_tests;

#[cfg(test)]
mod boundary_tests {
    use super::{
        MAX_OUTPUT_BYTES, SourceEdit, apply_source_edits, collect_ids, collect_style_edits,
        collect_style_text_edits, normalize_marker_selectors, remaining_source_output_budget,
    };
    use resvg::usvg::roxmltree::{Document, Error};

    #[test]
    fn malformed_marker_xml_preserves_original_source() -> Result<(), String> {
        let source = "<svg><style>[id$='end']{fill:red}</svg>";
        assert_eq!(normalize_marker_selectors(source.to_owned()), source);
        let error = Document::parse(source)
            .err()
            .ok_or("malformed XML was accepted")?;
        let expected = error.to_string();
        assert!(!expected.is_empty());
        assert_eq!(test_error(error), expected);
        Ok(())
    }

    #[test]
    fn duplicate_document_ids_are_collected_once() -> Result<(), String> {
        let source = "<svg><g id='end'/><g id='end'/></svg>";
        let document = Document::parse(source).map_err(test_error)?;
        assert_eq!(collect_ids(&document), Some(vec!["end".to_owned()]));
        Ok(())
    }

    #[test]
    fn style_node_with_unrelated_source_does_not_create_an_edit() -> Result<(), String> {
        let document =
            Document::parse("<style>[id$='end']{fill:red}</style>").map_err(test_error)?;
        let mut edits = Vec::new();
        assert!(collect_style_edits(
            document.root_element(),
            "",
            &[],
            &mut edits
        ));
        assert!(edits.is_empty());
        Ok(())
    }

    #[test]
    fn exhausted_source_budget_rejects_style_edit_without_partial_mutation() -> Result<(), String> {
        let source = "<style>[id$='end']{fill:red}</style>";
        let document = Document::parse(source).map_err(test_error)?;
        let mut edits = vec![SourceEdit {
            range: 0..1,
            replacement: "x".repeat(MAX_OUTPUT_BYTES + 1),
        }];
        assert!(remaining_source_output_budget(source, &edits, 1).is_none());
        assert!(!collect_style_text_edits(
            document.root_element(),
            source,
            &[],
            &mut edits
        ));
        assert_eq!(edits.len(), 1);
        Ok(())
    }

    #[test]
    fn invalid_and_overlapping_source_edits_preserve_original_bytes() {
        let invalid = vec![SourceEdit {
            range: 0..1,
            replacement: "x".to_owned(),
        }];
        assert_eq!(apply_source_edits("é".to_owned(), invalid), "é");
        let overlapping = vec![
            SourceEdit {
                range: 0..2,
                replacement: "x".to_owned(),
            },
            SourceEdit {
                range: 1..3,
                replacement: "y".to_owned(),
            },
        ];
        assert_eq!(apply_source_edits("abc".to_owned(), overlapping), "abc");
    }

    #[test]
    fn oversized_source_edit_preserves_original_bytes() {
        let edits = vec![SourceEdit {
            range: 0..1,
            replacement: "x".repeat(MAX_OUTPUT_BYTES + 1),
        }];
        assert_eq!(apply_source_edits("a".to_owned(), edits), "a");
    }
    fn test_error(error: Error) -> String {
        error.to_string()
    }
}
