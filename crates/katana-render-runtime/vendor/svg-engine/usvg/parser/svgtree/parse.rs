// Copyright 2021 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashMap;

use roxmltree::Error;
use simplecss::Declaration;
use svgtypes::FontShorthand;

use super::{AId, Attribute, Document, EId, NodeData, NodeId, NodeKind, ShortRange};

const SVG_NS: &str = "http://www.w3.org/2000/svg";
const XLINK_NS: &str = "http://www.w3.org/1999/xlink";
const XML_NAMESPACE_NS: &str = "http://www.w3.org/XML/1998/namespace";

impl<'input> Document<'input> {
    /// Parses a [`Document`] from a [`roxmltree::Document`].
    pub fn parse_tree(
        xml: &roxmltree::Document<'input>,
        injected_stylesheet: Option<&'input str>,
    ) -> Result<Document<'input>, Error> {
        parse(xml, injected_stylesheet)
    }

    pub(crate) fn append(&mut self, parent_id: NodeId, kind: NodeKind) -> NodeId {
        let new_child_id = NodeId::from(self.nodes.len());
        self.nodes.push(NodeData {
            parent: Some(parent_id),
            next_sibling: None,
            children: None,
            kind,
        });

        let last_child_id = self.nodes[parent_id.get_usize()].children.map(|(_, id)| id);

        if let Some(id) = last_child_id {
            self.nodes[id.get_usize()].next_sibling = Some(new_child_id);
        }

        self.nodes[parent_id.get_usize()].children = Some(
            if let Some((first_child_id, _)) = self.nodes[parent_id.get_usize()].children {
                (first_child_id, new_child_id)
            } else {
                (new_child_id, new_child_id)
            },
        );

        new_child_id
    }

    fn append_attribute(
        &mut self,
        name: AId,
        value: roxmltree::StringStorage<'input>,
        important: bool,
    ) {
        self.attrs.push(Attribute {
            name,
            value,
            important,
        });
    }
}

fn parse<'input>(
    xml: &roxmltree::Document<'input>,
    injected_stylesheet: Option<&'input str>,
) -> Result<Document<'input>, Error> {
    let mut doc = Document {
        nodes: Vec::new(),
        attrs: Vec::new(),
        links: HashMap::new(),
    };

    let id_map = xml_id_map(xml);

    add_root(&mut doc);

    let style_sheet = resolve_css(xml, injected_stylesheet);

    parse_xml_node_children(
        xml.root(),
        xml.root(),
        doc.root().id,
        &style_sheet,
        false,
        0,
        &mut doc,
        &id_map,
    )?;

    require_svg_root(&doc)?;
    finalize_document(&mut doc);
    Ok(doc)
}

fn xml_id_map<'a, 'input: 'a>(xml: &'a roxmltree::Document<'input>) -> HashMap<&'a str, roxmltree::Node<'a, 'input>> {
    let mut id_map = HashMap::new();
    for node in xml.descendants() {
        if let Some(id) = node.attribute("id") {
            if !id_map.contains_key(id) {
                id_map.insert(id, node);
            }
        }
    }
    id_map
}

fn add_root(doc: &mut Document) {
    doc.nodes.push(NodeData {
        parent: None,
        next_sibling: None,
        children: None,
        kind: NodeKind::Root,
    });
}

fn require_svg_root(doc: &Document) -> Result<(), Error> {
    match doc.root().first_element_child() {
        Some(child) => {
            if child.tag_name() != Some(EId::Svg) {
                return Err(roxmltree::Error::NoRootNode);
            }
        }
        None => return Err(roxmltree::Error::NoRootNode),
    }
    Ok(())
}

fn finalize_document(doc: &mut Document) {
    // Collect all elements with `id` attribute.
    let mut links = HashMap::new();
    for node in doc.descendants() {
        if let Some(id) = node.attribute::<&str>(AId::Id) {
            links.insert(id.to_string(), node.id);
        }
    }
    doc.links = links;

    fix_recursive_patterns(doc);
    fix_recursive_links(EId::ClipPath, AId::ClipPath, doc);
    fix_recursive_links(EId::Mask, AId::Mask, doc);
    fix_recursive_links(EId::Filter, AId::Filter, doc);
    fix_recursive_fe_image(doc);
}

pub(crate) fn parse_tag_name(node: roxmltree::Node) -> Option<EId> {
    if !node.is_element() {
        return None;
    }

    if !matches!(node.tag_name().namespace(), None | Some(SVG_NS)) {
        return None;
    }

    EId::from_str(node.tag_name().name())
}

fn parse_xml_node_children<'input>(
    parent: roxmltree::Node<'_, 'input>,
    origin: roxmltree::Node,
    parent_id: NodeId,
    style_sheet: &simplecss::StyleSheet,
    ignore_ids: bool,
    depth: u32,
    doc: &mut Document<'input>,
    id_map: &HashMap<&str, roxmltree::Node<'_, 'input>>,
) -> Result<(), Error> {
    for node in parent.children() {
        parse_xml_node(
            node,
            origin,
            parent_id,
            style_sheet,
            ignore_ids,
            depth,
            doc,
            id_map,
        )?;
    }

    Ok(())
}

fn parse_xml_node<'input>(
    node: roxmltree::Node<'_, 'input>,
    origin: roxmltree::Node,
    parent_id: NodeId,
    style_sheet: &simplecss::StyleSheet,
    ignore_ids: bool,
    depth: u32,
    doc: &mut Document<'input>,
    id_map: &HashMap<&str, roxmltree::Node<'_, 'input>>,
) -> Result<(), Error> {
    if depth > 1024 {
        return Err(Error::NodesLimitReached);
    }

    let tag_name = match renderable_tag(node) {
        Some(id) => id,
        None => return Ok(()),
    };

    let node_id = parse_svg_element(node, parent_id, tag_name, style_sheet, ignore_ids, doc)?;
    if tag_name == EId::Text {
        super::text::parse_svg_text_element(node, node_id, style_sheet, doc)?;
    } else if tag_name == EId::Use {
        parse_svg_use_element(node, origin, node_id, style_sheet, depth + 1, doc, id_map)?;
    } else {
        parse_xml_node_children(
            node,
            origin,
            node_id,
            style_sheet,
            ignore_ids,
            depth + 1,
            doc,
            id_map,
        )?;
    }

    Ok(())
}

fn renderable_tag(node: roxmltree::Node) -> Option<EId> {
    let tag_name = parse_tag_name(node)?;
    if tag_name == EId::Style {
        return None;
    }
    // Treat links as groups.
    Some(if tag_name == EId::A { EId::G } else { tag_name })
}

pub(crate) fn parse_svg_element<'input>(
    xml_node: roxmltree::Node<'_, 'input>,
    parent_id: NodeId,
    tag_name: EId,
    style_sheet: &simplecss::StyleSheet,
    ignore_ids: bool,
    doc: &mut Document<'input>,
) -> Result<NodeId, Error> {
    let attrs_start_idx = doc.attrs.len();
    copy_presentational_attributes(xml_node, parent_id, tag_name, ignore_ids, doc);
    apply_element_styles(xml_node, parent_id, tag_name, style_sheet, attrs_start_idx, doc);
    if doc.nodes.len() > 1_000_000 {
        return Err(Error::NodesLimitReached);
    }
    let node_id = doc.append(
        parent_id,
        NodeKind::Element {
            tag_name,
            attributes: ShortRange::new(attrs_start_idx as u32, doc.attrs.len() as u32),
        },
    );
    Ok(node_id)
}

fn copy_presentational_attributes<'input>(
    node: roxmltree::Node<'_, 'input>,
    parent_id: NodeId,
    tag_name: EId,
    ignore_ids: bool,
    doc: &mut Document<'input>,
) {
    let mut href_idx = None;
    for attr in node.attributes() {
        copy_presentational_attribute(attr, parent_id, tag_name, ignore_ids, doc, &mut href_idx);
    }
}

fn copy_presentational_attribute<'input>(
    attr: roxmltree::Attribute<'_, 'input>,
    parent_id: NodeId,
    tag_name: EId,
    ignore_ids: bool,
    doc: &mut Document<'input>,
    href_idx: &mut Option<usize>,
) {
    let aid = match presentational_attribute_id(attr, ignore_ids) {
        Some(v) => v,
        None => return,
    };
    if aid == AId::Href && skip_href_attribute(attr, *href_idx, doc) {
        return;
    }
    let added = append_attribute(parent_id, tag_name, aid, attr.value_storage().clone(), false, doc);
    if added && aid == AId::Href {
        *href_idx = Some(doc.attrs.len() - 1);
    }
}

fn presentational_attribute_id(attr: roxmltree::Attribute, ignore_ids: bool) -> Option<AId> {
    match attr.namespace() {
        None | Some(SVG_NS) | Some(XLINK_NS) | Some(XML_NAMESPACE_NS) => {}
        _ => return None,
    }
    let aid = AId::from_str(attr.name())?;
    if ignore_ids && aid == AId::Id {
        return None;
    }
    if matches!(aid, AId::MixBlendMode | AId::Isolation | AId::FontKerning) {
        return None;
    }
    Some(aid)
}

// hrefが重複した場合は無名前空間の値を優先する。
fn skip_href_attribute<'input>(
    attr: roxmltree::Attribute<'_, 'input>,
    href_idx: Option<usize>,
    doc: &mut Document<'input>,
) -> bool {
    let is_unprefixed = attr.namespace().is_none();
    let is_xlink = attr.namespace() == Some(XLINK_NS);
    if !is_unprefixed && !is_xlink {
        return true;
    }
    if let Some(idx) = href_idx {
        if is_unprefixed {
            doc.attrs[idx].value = attr.value_storage().clone();
        }
        return true;
    }
    false
}

fn apply_element_styles<'input>(
    node: roxmltree::Node<'_, 'input>,
    parent_id: NodeId,
    tag_name: EId,
    style_sheet: &simplecss::StyleSheet,
    attrs_start_idx: usize,
    doc: &mut Document<'input>,
) {
    let mut insert = |aid, value: &str, important| {
        insert_style_attribute(parent_id, tag_name, aid, value, important, attrs_start_idx, doc);
    };
    let mut write = |declaration: &Declaration| write_style_declaration(declaration, &mut insert);
    for rule in &style_sheet.rules {
        if rule.selector.matches(&XmlNode(node)) {
            for declaration in &rule.declarations {
                write(declaration);
            }
        }
    }
    if let Some(value) = node.attribute("style") {
        for_each_inline_declaration(value, &mut write);
    }
}

fn insert_style_attribute(
    parent_id: NodeId,
    tag_name: EId,
    aid: AId,
    value: &str,
    important: bool,
    attrs_start_idx: usize,
    doc: &mut Document,
) {
    let idx = doc.attrs[attrs_start_idx..].iter_mut().position(|a| a.name == aid);
    let added = append_attribute(
        parent_id, tag_name, aid, roxmltree::StringStorage::new_owned(value), important, doc,
    );
    if added {
        if let Some(idx) = idx {
            apply_style_precedence(doc, attrs_start_idx + idx, important);
        }
    }
}

fn apply_style_precedence(doc: &mut Document, existing_idx: usize, important: bool) {
    let last_idx = doc.attrs.len() - 1;
    // 同じCSS優先度では後の宣言を採用し、importantだけはnormalより優先する。
    let has_precedence = !doc.attrs[existing_idx].important || important;
    if has_precedence {
        doc.attrs.swap(existing_idx, last_idx);
    }
    doc.attrs.pop();
}

fn write_style_declaration(declaration: &Declaration, insert: &mut impl FnMut(AId, &str, bool)) {
    let imp = declaration.important;
    let val = declaration.value;
    if declaration.name == "marker" {
        insert(AId::MarkerStart, val, imp);
        insert(AId::MarkerMid, val, imp);
        insert(AId::MarkerEnd, val, imp);
    } else if declaration.name == "font" {
        write_font_declaration(val, imp, insert);
    } else if let Some(aid) = AId::from_str(declaration.name) {
        if aid.is_presentation() {
            insert(aid, val, imp);
        }
    }
}

fn write_font_declaration(value: &str, important: bool, insert: &mut impl FnMut(AId, &str, bool)) {
    if let Ok(shorthand) = FontShorthand::from_str(value) {
        reset_font_properties(important, insert);
        apply_font_shorthand(shorthand, important, insert);
    } else {
        log::warn!("Failed to parse {} value: '{}'", AId::Font, value);
    }
}

fn reset_font_properties(important: bool, insert: &mut impl FnMut(AId, &str, bool)) {
    insert(AId::FontStyle, "normal", important);
    insert(AId::FontVariant, "normal", important);
    insert(AId::FontWeight, "normal", important);
    insert(AId::FontStretch, "normal", important);
    insert(AId::LineHeight, "normal", important);
    insert(AId::FontSizeAdjust, "none", important);
    insert(AId::FontKerning, "auto", important);
    insert(AId::FontVariantCaps, "normal", important);
    insert(AId::FontVariantLigatures, "normal", important);
    insert(AId::FontVariantNumeric, "normal", important);
    insert(AId::FontVariantEastAsian, "normal", important);
    insert(AId::FontVariantPosition, "normal", important);
}

fn apply_font_shorthand(
    shorthand: FontShorthand<'_>,
    important: bool,
    insert: &mut impl FnMut(AId, &str, bool),
) {
    shorthand.font_stretch.map(|s| insert(AId::FontStretch, s, important));
    shorthand.font_weight.map(|s| insert(AId::FontWeight, s, important));
    shorthand.font_variant.map(|s| insert(AId::FontVariant, s, important));
    shorthand.font_style.map(|s| insert(AId::FontStyle, s, important));
    insert(AId::FontSize, shorthand.font_size, important);
    insert(AId::FontFamily, shorthand.font_family, important);
}

fn for_each_inline_declaration(style: &str, mut write: impl FnMut(&Declaration<'_>)) {
    let mut parser = cssparser::Parser::new(style);
    while !parser.is_exhausted() {
        let segment = parser.parse_until_before(cssparser::Delimiter::Semicolon, |input| {
            let start = input.position();
            crate::css_parser_shared::CssParserShared::consume_nested(input)?;
            Ok::<_, cssparser::ParseError<()>>(input.slice_from(start))
        });
        if let Ok(segment) = segment {
            for declaration in simplecss::DeclarationTokenizer::from(segment) {
                write(&declaration);
            }
        }
        if parser.next_including_whitespace_and_comments().is_err() {
            break;
        }
    }
}

fn append_attribute<'input>(
    parent_id: NodeId,
    tag_name: EId,
    aid: AId,
    value: roxmltree::StringStorage<'input>,
    important: bool,
    doc: &mut Document<'input>,
) -> bool {
    match aid {
        // The `style` attribute will be split into attributes, so we don't need it.
        AId::Style |
        // No need to copy a `class` attribute since CSS were already resolved.
        AId::Class => return false,
        _ => {}
    }

    // Ignore `xlink:href` on `tspan` (which was originally `tref` or `a`),
    // because we will convert `tref` into `tspan` anyway.
    if tag_name == EId::Tspan && aid == AId::Href {
        return false;
    }

    if aid.allows_inherit_value() && &*value == "inherit" {
        return resolve_inherit(parent_id, aid, doc);
    }

    doc.append_attribute(aid, value, important);
    true
}

fn resolve_inherit(parent_id: NodeId, aid: AId, doc: &mut Document) -> bool {
    if copy_inherited_attribute(parent_id, aid, doc) {
        return true;
    }
    let value = match inherited_default(aid) {
        Some(value) => value,
        None => return false,
    };
    doc.append_attribute(aid, roxmltree::StringStorage::Borrowed(value), false);
    true
}

fn copy_inherited_attribute(parent_id: NodeId, aid: AId, doc: &mut Document) -> bool {
    let source = if aid.is_inheritable() {
        doc.get(parent_id).ancestors().find(|n| n.has_attribute(aid)).map(|n| n.id)
    } else {
        Some(parent_id)
    };
    let attr = source.and_then(|id| {
        doc.get(id).attributes().iter().find(|a| a.name == aid).cloned()
    });
    if let Some(attr) = attr {
        doc.attrs.push(Attribute { name: aid, value: attr.value, important: attr.important });
        return true;
    }
    false
}

fn inherited_default(aid: AId) -> Option<&'static str> {
    inherited_group_default(aid)
        .or_else(|| inherited_paint_default(aid))
        .or_else(|| inherited_property_default(aid))
}

fn inherited_group_default(aid: AId) -> Option<&'static str> {
    match aid {
        AId::ImageRendering
        | AId::ShapeRendering
        | AId::TextRendering => Some("auto"),
        AId::ClipPath
        | AId::Filter
        | AId::MarkerEnd
        | AId::MarkerMid
        | AId::MarkerStart
        | AId::Mask
        | AId::Stroke
        | AId::StrokeDasharray
        | AId::TextDecoration => Some("none"),
        AId::FontStretch
        | AId::FontStyle
        | AId::FontVariant
        | AId::FontWeight
        | AId::LetterSpacing
        | AId::WordSpacing => Some("normal"),
        _ => None,
    }
}

fn inherited_paint_default(aid: AId) -> Option<&'static str> {
    match aid {
        AId::Fill
        | AId::FloodColor
        | AId::StopColor => Some("black"),
        AId::FillOpacity
        | AId::FloodOpacity
        | AId::Opacity
        | AId::StopOpacity
        | AId::StrokeOpacity => Some("1"),
        AId::ClipRule
        | AId::FillRule => Some("nonzero"),
        _ => None,
    }
}

fn inherited_property_default(aid: AId) -> Option<&'static str> {
    match aid {
        AId::BaselineShift => Some("baseline"),
        AId::ColorInterpolationFilters => Some("linearRGB"),
        AId::Direction => Some("ltr"),
        AId::Display => Some("inline"),
        AId::FontSize => Some("medium"),
        AId::Overflow => Some("visible"),
        AId::StrokeDashoffset => Some("0"),
        AId::StrokeLinecap => Some("butt"),
        AId::StrokeLinejoin => Some("miter"),
        AId::StrokeMiterlimit => Some("4"),
        AId::StrokeWidth => Some("1"),
        AId::TextAnchor => Some("start"),
        AId::Visibility => Some("visible"),
        AId::WritingMode => Some("lr-tb"),
        _ => None,
    }
}

fn resolve_href<'a, 'input: 'a>(
    node: roxmltree::Node<'a, 'input>,
    id_map: &HashMap<&str, roxmltree::Node<'a, 'input>>,
) -> Option<roxmltree::Node<'a, 'input>> {
    // See the comment in `parse_svg_element` about `href` precedence.
    //
    // Note: `roxmltree::Node::attribute("href")` matches by local name only and
    // would return whichever `href`/`xlink:href` comes first, so we have to
    // filter by namespace explicitly.
    let link_value = node
        .attributes()
        .find(|a| a.name() == "href" && a.namespace().is_none())
        .or_else(|| {
            node.attributes()
                .find(|a| a.name() == "href" && a.namespace() == Some(XLINK_NS))
        })
        .map(|a| a.value())?;

    let link_id = svgtypes::IRI::from_str(link_value).ok()?.0;

    id_map.get(link_id).copied()
}

fn parse_svg_use_element<'input>(
    node: roxmltree::Node<'_, 'input>,
    origin: roxmltree::Node,
    parent_id: NodeId,
    style_sheet: &simplecss::StyleSheet,
    depth: u32,
    doc: &mut Document<'input>,
    id_map: &HashMap<&str, roxmltree::Node<'_, 'input>>,
) -> Result<(), Error> {
    let link = match resolve_href(node, id_map) {
        Some(v) => v,
        None => return Ok(()),
    };

    if !valid_use_link(node, origin, link, id_map) {
        return Ok(());
    }

    parse_xml_node(
        link,
        node,
        parent_id,
        style_sheet,
        true,
        depth + 1,
        doc,
        id_map,
    )
}

fn valid_use_link<'a, 'input: 'a>(
    node: roxmltree::Node<'a, 'input>,
    origin: roxmltree::Node,
    link: roxmltree::Node<'a, 'input>,
    id_map: &HashMap<&str, roxmltree::Node<'a, 'input>>,
) -> bool {
    if link == node || link == origin {
        log::warn!(
            "Recursive 'use' detected. '{}' will be skipped.",
            node.attribute((SVG_NS, "id")).unwrap_or_default()
        );
        return false;
    }

    // Make sure we're linked to an SVG element.
    if parse_tag_name(link).is_none() {
        return false;
    }

    if use_link_is_recursive(node, link, id_map) {
        log::warn!(
            "Recursive 'use' detected. '{}' will be skipped.",
            node.attribute((SVG_NS, "id")).unwrap_or_default()
        );
        return false;
    }
    true
}

// Check that none of the linked node's children reference current `use` node
    // via other `use` node.
    //
    // Example:
    // <g id="g1">
    //     <use xlink:href="#use1" id="use2"/>
    // </g>
    // <use xlink:href="#g1" id="use1"/>
    //
    // `use2` should be removed.
    //
    // Also, child should not reference its parent:
    // <g id="g1">
    //     <use xlink:href="#g1" id="use1"/>
    // </g>
    //
    // `use1` should be removed.
fn use_link_is_recursive<'a, 'input: 'a>(
    node: roxmltree::Node<'a, 'input>,
    link: roxmltree::Node<'a, 'input>,
    id_map: &HashMap<&str, roxmltree::Node<'a, 'input>>,
) -> bool {
    let mut is_recursive = false;
    for link_child in link
        .descendants()
        .skip(1)
        .filter(|n| n.has_tag_name((SVG_NS, "use")))
    {
        if let Some(link2) = resolve_href(link_child, id_map) {
            if link2 == node || link2 == link {
                is_recursive = true;
                break;
            }
        }
    }

    is_recursive
}

fn resolve_css<'a>(
    xml: &'a roxmltree::Document<'a>,
    style_sheet: Option<&'a str>,
) -> simplecss::StyleSheet<'a> {
    let mut sheet = simplecss::StyleSheet::new();

    // Injected style sheets do not override internal ones (we mimic the logic of rsvg-convert),
    // so we need to parse it first.
    if let Some(style_sheet) = style_sheet {
        sheet.parse_more(style_sheet);
    }

    for node in xml.descendants().filter(|n| n.has_tag_name("style")) {
        match node.attribute("type") {
            Some("text/css") => {}
            Some(_) => continue,
            None => {}
        }

        let text = match node.text() {
            Some(v) => v,
            None => continue,
        };

        sheet.parse_more(text);
    }

    sheet
}

struct XmlNode<'a, 'input: 'a>(roxmltree::Node<'a, 'input>);

impl simplecss::Element for XmlNode<'_, '_> {
    fn parent_element(&self) -> Option<Self> {
        self.0.parent_element().map(XmlNode)
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        self.0.prev_sibling_element().map(XmlNode)
    }

    fn has_local_name(&self, local_name: &str) -> bool {
        self.0.tag_name().name() == local_name
    }

    fn attribute_matches(&self, local_name: &str, operator: simplecss::AttributeOperator) -> bool {
        match self.0.attribute(local_name) {
            Some(value) => operator.matches(value),
            None => false,
        }
    }

    fn pseudo_class_matches(&self, class: simplecss::PseudoClass) -> bool {
        match class {
            simplecss::PseudoClass::FirstChild => self.prev_sibling_element().is_none(),
            // TODO: lang
            _ => false, // Since we are querying a static SVG we can ignore other pseudo-classes.
        }
    }
}

fn fix_recursive_patterns(doc: &mut Document) {
    while let Some(node_id) = find_recursive_pattern(AId::Fill, doc) {
        let idx = doc.get(node_id).attribute_id(AId::Fill).unwrap();
        doc.attrs[idx].value = roxmltree::StringStorage::Borrowed("none");
    }

    while let Some(node_id) = find_recursive_pattern(AId::Stroke, doc) {
        let idx = doc.get(node_id).attribute_id(AId::Stroke).unwrap();
        doc.attrs[idx].value = roxmltree::StringStorage::Borrowed("none");
    }
}

fn find_recursive_pattern(aid: AId, doc: &mut Document) -> Option<NodeId> {
    for pattern_node in doc
        .root()
        .descendants()
        .filter(|n| n.tag_name() == Some(EId::Pattern))
    {
        for node in pattern_node.descendants() {
            if let Some(id) = recursive_pattern_node(aid, doc, pattern_node, node) {
                return Some(id);
            }
        }
    }
    None
}

fn recursive_pattern_node(
    aid: AId,
    doc: &Document,
    pattern: super::SvgNode,
    node: super::SvgNode,
) -> Option<NodeId> {
    let value = node.attribute(aid)?;
    if let Ok(svgtypes::Paint::FuncIRI(link_id, _)) = svgtypes::Paint::from_str(value) {
        if link_id == pattern.element_id() {
            return Some(node.id);
        }
        if let Some(linked_node) = doc.element_by_id(link_id) {
            for node2 in linked_node.descendants() {
                let value2 = match node2.attribute(aid) {
                    Some(v) => v,
                    None => continue,
                };
                if let Ok(svgtypes::Paint::FuncIRI(link_id2, _)) = svgtypes::Paint::from_str(value2) {
                    if link_id2 == pattern.element_id() {
                        return Some(node2.id);
                    }
                }
            }
        }
    }
    None
}

fn fix_recursive_links(eid: EId, aid: AId, doc: &mut Document) {
    while let Some(node_id) = find_recursive_link(eid, aid, doc) {
        let idx = doc.get(node_id).attribute_id(aid).unwrap();
        doc.attrs[idx].value = roxmltree::StringStorage::Borrowed("none");
    }
}

fn find_recursive_link(eid: EId, aid: AId, doc: &Document) -> Option<NodeId> {
    for node in doc
        .root()
        .descendants()
        .filter(|n| n.tag_name() == Some(eid))
    {
        for child in node.descendants() {
            if let Some(link) = child.node_attribute(aid) {
                if link == node {
                    // If an element child has a link to the element itself
                    // then we have to replace it with `none`.
                    // Otherwise we will get endless loop/recursion and stack overflow.
                    return Some(child.id);
                } else {
                    // Check that linked node children doesn't link this element.
                    for node2 in link.descendants() {
                        if let Some(link2) = node2.node_attribute(aid) {
                            if link2 == node {
                                return Some(node2.id);
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

/// Detects cases like:
///
/// ```xml
/// <filter id="filter1">
///   <feImage xlink:href="#rect1"/>
/// </filter>
/// <rect id="rect1" x="36" y="36" width="120" height="120" fill="green" filter="url(#filter1)"/>
/// ```
fn fix_recursive_fe_image(doc: &mut Document) {
    let mut ids = Vec::new();
    for fe_node in doc
        .root()
        .descendants()
        .filter(|n| n.tag_name() == Some(EId::FeImage))
    {
        if let Some(link) = fe_node.node_attribute(AId::Href) {
            if let Some(filter_uri) = link.attribute::<&str>(AId::Filter) {
                let filter_id = fe_node.parent().unwrap().element_id();
                for func in svgtypes::FilterValueListParser::from(filter_uri).flatten() {
                    if let svgtypes::FilterValue::Url(url) = func {
                        if url == filter_id {
                            ids.push(link.id);
                        }
                    }
                }
            }
        }
    }

    for id in ids {
        let idx = doc.get(id).attribute_id(AId::Filter).unwrap();
        doc.attrs[idx].value = roxmltree::StringStorage::Borrowed("none");
    }
}

#[cfg(test)]
crate::svg_engine::owned_svg_model_tests!("../../../../../src/svg_engine_svgtree_parse_model_tests.rs");
