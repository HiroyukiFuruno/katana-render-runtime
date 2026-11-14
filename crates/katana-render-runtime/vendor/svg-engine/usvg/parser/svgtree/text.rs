// Copyright 2021 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(clippy::comparison_chain)]

use roxmltree::Error;

use super::{AId, Document, EId, NodeId, NodeKind, SvgNode};

const XLINK_NS: &str = "http://www.w3.org/1999/xlink";

pub(crate) fn parse_svg_text_element<'input>(
    parent: roxmltree::Node<'_, 'input>,
    parent_id: NodeId,
    style_sheet: &simplecss::StyleSheet,
    doc: &mut Document<'input>,
) -> Result<(), Error> {
    debug_assert_eq!(parent.tag_name().name(), "text");

    let space = if doc.get(parent_id).has_attribute(AId::Space) {
        get_xmlspace(doc, parent_id, XmlSpace::Default)
    } else {
        if let Some(node) = doc
            .get(parent_id)
            .ancestors()
            .find(|n| n.has_attribute(AId::Space))
        {
            get_xmlspace(doc, node.id, XmlSpace::Default)
        } else {
            XmlSpace::Default
        }
    };

    parse_svg_text_element_impl(parent, parent_id, style_sheet, space, doc)?;

    trim_text_nodes(parent_id, space, doc);
    Ok(())
}

fn parse_svg_text_element_impl<'input>(
    parent: roxmltree::Node<'_, 'input>,
    parent_id: NodeId,
    style_sheet: &simplecss::StyleSheet,
    space: XmlSpace,
    doc: &mut Document<'input>,
) -> Result<(), Error> {
    for node in parent.children() {
        if node.is_text() {
            let text = trim_text(node.text().unwrap(), space);
            doc.append(parent_id, NodeKind::Text(text));
            continue;
        }

        let (tag_name, is_tref) = match text_tag(node, parent) {
            Some(v) => v,
            None => continue,
        };

        let node_id =
            super::parse::parse_svg_element(node, parent_id, tag_name, style_sheet, false, doc)?;
        let space = get_xmlspace(doc, node_id, space);

        if is_tref {
            append_tref_text(node, node_id, space, doc);
        } else {
            parse_svg_text_element_impl(node, node_id, style_sheet, space, doc)?;
        }
    }

    Ok(())
}

fn text_tag(node: roxmltree::Node, parent: roxmltree::Node) -> Option<(EId, bool)> {
    let mut tag_name = super::parse::parse_tag_name(node)?;
    if tag_name == EId::A {
        tag_name = EId::Tspan;
    }
    if !matches!(tag_name, EId::Tspan | EId::Tref | EId::TextPath) {
        return None;
    }
    if tag_name == EId::TextPath && parent.tag_name().name() != "text" {
        return None;
    }
    let is_tref = tag_name == EId::Tref;
    if is_tref {
        tag_name = EId::Tspan;
    }
    Some((tag_name, is_tref))
}

fn append_tref_text(node: roxmltree::Node, node_id: NodeId, space: XmlSpace, doc: &mut Document) {
    let link_value = node
        .attribute((XLINK_NS, "href"))
        .or_else(|| node.attribute("href"));
    if let Some(href) = link_value {
        if let Some(text) = resolve_tref_text(node.document(), href) {
            let text = trim_text(&text, space);
            doc.append(node_id, NodeKind::Text(text));
        }
    }
}

fn resolve_tref_text(xml: &roxmltree::Document, href: &str) -> Option<String> {
    let id = svgtypes::IRI::from_str(href).ok()?.0;

    // Find linked element in the original tree.
    let node = xml.descendants().find(|n| n.attribute("id") == Some(id))?;

    // `tref` should be linked to an SVG element.
    super::parse::parse_tag_name(node)?;

    // 'All character data within the referenced element, including character data enclosed
    // within additional markup, will be rendered.'
    //
    // So we don't care about attributes and everything. Just collecting text nodes data.
    //
    // Note: we have to filter nodes by `is_text()` first since `text()` will look up
    // for text nodes in element children therefore we will get duplicates.
    let text: String = node
        .descendants()
        .filter(|n| n.is_text())
        .filter_map(|n| n.text())
        .collect();
    if text.is_empty() { None } else { Some(text) }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum XmlSpace {
    Default,
    Preserve,
}

fn get_xmlspace(doc: &Document, node_id: NodeId, default: XmlSpace) -> XmlSpace {
    match doc.get(node_id).attribute(AId::Space) {
        Some("preserve") => XmlSpace::Preserve,
        Some(_) => XmlSpace::Default,
        _ => default,
    }
}

trait StrTrim {
    fn remove_first_space(&mut self);
    fn remove_last_space(&mut self);
}

impl StrTrim for String {
    fn remove_first_space(&mut self) {
        debug_assert_eq!(self.chars().next().unwrap(), ' ');
        self.drain(0..1);
    }

    fn remove_last_space(&mut self) {
        debug_assert_eq!(self.chars().next_back().unwrap(), ' ');
        self.pop();
    }
}

/// Prepares text nodes according to the spec: https://www.w3.org/TR/SVG11/text.html#WhiteSpace
///
/// This function handles:
/// - 'xml:space' processing
/// - tabs and newlines removing/replacing
/// - spaces trimming
fn trim_text_nodes(text_elem_id: NodeId, xmlspace: XmlSpace, doc: &mut Document) {
    let mut nodes = Vec::new();
    collect_text_nodes(doc.get(text_elem_id), 0, &mut nodes);
    if nodes.len() == 1 {
        trim_single_text_node(nodes[0].0, xmlspace, doc);
    } else if nodes.len() > 1 {
        trim_text_pairs(&nodes, xmlspace, doc);
    }
}

fn trim_single_text_node(node_id: NodeId, xmlspace: XmlSpace, doc: &mut Document) {
    if xmlspace == XmlSpace::Default
        && let NodeKind::Text(ref mut text) = doc.nodes[node_id.get_usize()].kind
    {
        if text.starts_with(' ') {
            text.remove_first_space();
        }
        if text.ends_with(' ') {
            text.remove_last_space();
        }
    }
}

struct TextPair {
    first: NodeId,
    second: NodeId,
    first_depth: usize,
    second_depth: usize,
    first_space: XmlSpace,
    second_space: XmlSpace,
    chars: [Option<u8>; 4],
    is_first: bool,
    is_last: bool,
}

fn trim_text_pairs(nodes: &[(NodeId, usize)], xmlspace: XmlSpace, doc: &mut Document) {
    let mut last_non_empty: Option<NodeId> = None;
    for i in 0..nodes.len() - 1 {
        let pair = text_pair(nodes, i, last_non_empty, xmlspace, doc);
        trim_pair_boundary(&pair, doc);
        trim_pair_edges(&pair, doc);
        trim_pair_empty_tail(&pair, doc);
        if !doc.get(pair.first).text().trim().is_empty() {
            last_non_empty = Some(pair.first);
        }
    }
}

fn text_pair(
    nodes: &[(NodeId, usize)],
    i: usize,
    last_non_empty: Option<NodeId>,
    xmlspace: XmlSpace,
    doc: &Document,
) -> TextPair {
    let (mut first, first_depth) = nodes[i];
    let (second, second_depth) = nodes[i + 1];
    if doc.get(first).text().is_empty() {
        if let Some(n) = last_non_empty {
            first = n;
        }
    }
    let first_space = get_xmlspace(doc, doc.get(first).parent().unwrap().id, xmlspace);
    let second_space = get_xmlspace(doc, doc.get(second).parent().unwrap().id, xmlspace);
    let chars = text_pair_characters(doc, first, second);
    TextPair {
        first, second, first_depth, second_depth, first_space, second_space,
        chars, is_first: i == 0, is_last: i == nodes.len() - 2,
    }
}

fn text_pair_characters(doc: &Document, first: NodeId, second: NodeId) -> [Option<u8>; 4] {
    let text1 = doc.get(first).text();
    let text2 = doc.get(second).text();
    let bytes1 = text1.as_bytes();
    let bytes2 = text2.as_bytes();
    [bytes1.first().cloned(), bytes1.last().cloned(),
     bytes2.first().cloned(), bytes2.last().cloned()]
}

fn trim_pair_boundary(pair: &TextPair, doc: &mut Document) {
    let [_, c2, c3, _] = pair.chars;
    if pair.first_depth < pair.second_depth {
        if c3 == Some(b' ') && pair.second_space == XmlSpace::Default {
            if let NodeKind::Text(ref mut text) = doc.nodes[pair.second.get_usize()].kind {
                text.remove_first_space();
            }
        }
    } else if c2 == Some(b' ') && c2 == c3 {
        if pair.first_space == XmlSpace::Default && pair.second_space == XmlSpace::Default {
            if let NodeKind::Text(ref mut text) = doc.nodes[pair.first.get_usize()].kind {
                text.remove_last_space();
            }
        } else if pair.first_space == XmlSpace::Preserve && pair.second_space == XmlSpace::Default {
            if let NodeKind::Text(ref mut text) = doc.nodes[pair.second.get_usize()].kind {
                text.remove_first_space();
            }
        }
    }
}

fn trim_pair_edges(pair: &TextPair, doc: &mut Document) {
    let [c1, _, _, c4] = pair.chars;
    if pair.is_first
        && c1 == Some(b' ')
        && pair.first_space == XmlSpace::Default
        && !doc.get(pair.first).text().is_empty()
    {
        if let NodeKind::Text(ref mut text) = doc.nodes[pair.first.get_usize()].kind {
            text.remove_first_space();
        }
    } else if pair.is_last
        && c4 == Some(b' ')
        && !doc.get(pair.second).text().is_empty()
        && pair.second_space == XmlSpace::Default
    {
        if let NodeKind::Text(ref mut text) = doc.nodes[pair.second.get_usize()].kind {
            text.remove_last_space();
        }
    }
}

fn trim_pair_empty_tail(pair: &TextPair, doc: &mut Document) {
    let [_, c2, _, _] = pair.chars;
    if pair.is_last
        && c2 == Some(b' ')
        && !doc.get(pair.first).text().is_empty()
        && doc.get(pair.second).text().is_empty()
        && doc.get(pair.first).text().ends_with(' ')
    {
        if let NodeKind::Text(ref mut text) = doc.nodes[pair.first.get_usize()].kind {
            text.remove_last_space();
        }
    }
}

fn collect_text_nodes(parent: SvgNode, depth: usize, nodes: &mut Vec<(NodeId, usize)>) {
    for child in parent.children() {
        if child.is_text() {
            nodes.push((child.id, depth));
        } else if child.is_element() {
            collect_text_nodes(child, depth + 1, nodes);
        }
    }
}

fn trim_text(text: &str, space: XmlSpace) -> String {
    let mut s = String::with_capacity(text.len());

    let mut prev = '0';
    for c in text.chars() {
        // \r, \n and \t should be converted into spaces.
        let c = match c {
            '\r' | '\n' | '\t' => ' ',
            _ => c,
        };

        // Skip continuous spaces.
        if space == XmlSpace::Default && c == ' ' && c == prev {
            continue;
        }

        prev = c;

        s.push(c);
    }

    s
}
