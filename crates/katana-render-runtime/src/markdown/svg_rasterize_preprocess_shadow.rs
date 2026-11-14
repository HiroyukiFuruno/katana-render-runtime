use super::source_edits::{SourceEdit, apply_source_edits, direct_text_ranges};
use std::ops::Range;

use resvg::usvg::roxmltree::{Document, Node};

#[path = "svg_rasterize_preprocess_shadow_css.rs"]
mod css;

const DROP_SHADOW: &str = "drop-shadow(";
const RGBA: &str = "rgba(";

pub(super) fn normalize_filter_shadow_colors(svg_text: impl Into<String>) -> String {
    let svg_text = svg_text.into();
    if !contains_ascii_case_insensitive(&svg_text, DROP_SHADOW)
        || !contains_ascii_case_insensitive(&svg_text, RGBA)
    {
        return svg_text;
    }
    let Ok(document) = Document::parse(&svg_text) else {
        return svg_text;
    };
    let mut edits = Vec::new();
    for node in document.descendants().filter(|node| node.is_element()) {
        collect_node_edits(node, &svg_text, &mut edits);
    }
    if edits.is_empty() {
        return svg_text;
    }
    apply_source_edits(&svg_text, edits, false)
}

fn contains_ascii_case_insensitive(source: &str, needle: &str) -> bool {
    source
        .as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

fn collect_node_edits(node: Node<'_, '_>, source: &str, edits: &mut Vec<SourceEdit>) {
    collect_style_attribute_edits(node, source, edits);
    if node.tag_name().name() == "style" {
        collect_style_element_edits(node, source, edits);
    }
}

fn collect_style_attribute_edits(node: Node<'_, '_>, source: &str, edits: &mut Vec<SourceEdit>) {
    for attribute in node
        .attributes()
        .filter(|attribute| attribute.name() == "style" && attribute.namespace().is_none())
    {
        let range = attribute.range_value();
        if source.get(range.clone()) != Some(attribute.value()) {
            continue;
        }
        let Some(replacement) = css::normalize_inline_style(attribute.value()) else {
            continue;
        };
        push_changed_edit(edits, range, attribute.value(), replacement);
    }
}

fn collect_style_element_edits(node: Node<'_, '_>, source: &str, edits: &mut Vec<SourceEdit>) {
    for (range, text) in direct_text_ranges(node) {
        if source.get(range.clone()) != Some(text) {
            continue;
        }
        let Some(replacement) = css::normalize_stylesheet(text) else {
            continue;
        };
        push_changed_edit(edits, range, text, replacement);
    }
}

fn push_changed_edit(
    edits: &mut Vec<SourceEdit>,
    range: Range<usize>,
    source: &str,
    replacement: String,
) {
    if replacement != source {
        edits.push(SourceEdit::new(range, replacement));
    }
}

#[cfg(test)]
#[path = "svg_rasterize_preprocess_shadow_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "svg_rasterize_preprocess_shadow_natural_tests.rs"]
mod natural_tests;

#[cfg(test)]
mod boundary_tests {
    use super::normalize_filter_shadow_colors;
    use crate::markdown::svg_rasterize::SvgRasterizeOps;

    #[test]
    fn malformed_shadow_xml_preserves_original_source() {
        let source = "<svg><style>g{filter:drop-shadow(1px 2px rgba(0,0,0,.5))}</svg>";
        assert_eq!(normalize_filter_shadow_colors(source), source);
        assert_eq!(SvgRasterizeOps::preprocess_for_rasterizer(source), source);
    }

    #[test]
    fn shadow_preprocess_preserves_source_when_either_trigger_is_absent() {
        for source in [
            r##"<svg xmlns="http://www.w3.org/2000/svg"><rect style="filter:drop-shadow(1px 2px #0008)"/></svg>"##,
            r#"<svg xmlns="http://www.w3.org/2000/svg"><rect style="fill:rgba(0,0,0,.5)"/></svg>"#,
        ] {
            assert_eq!(normalize_filter_shadow_colors(source), source);
            assert_eq!(SvgRasterizeOps::preprocess_for_rasterizer(source), source);
        }
    }
}
