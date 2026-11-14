use std::io::{self, Cursor, Write};

use super::{AId, Document, NodeData, NodeKind, Opacity, ShapeRendering, SvgNode, TextRendering};

#[path = "svg_engine_svgtree_href_iterator_tests.rs"]
mod href_iterator_tests;

const SVG_NS: &str = "http://www.w3.org/2000/svg";
const XLINK_NS: &str = "http://www.w3.org/1999/xlink";
const DASH_ARRAY_SIZE: usize = 3;
const EXPECTED_QUARTER_OPACITY: f32 = 0.25;
const TAIL_CYCLE_EXPECTED_NODES: usize = 3;
const CYCLE_EXTRA_NODE_PROBE: usize = 1;

fn parse_document(source: &str) -> Result<Document<'_>, String> {
    let xml = roxmltree::Document::parse(source).map_err(|error| error.to_string())?;
    Document::parse_tree(&xml, None).map_err(|error| error.to_string())
}

fn write_debug<T: std::fmt::Debug>(value: &T, output: &mut [u8]) -> io::Result<usize> {
    let mut cursor = Cursor::new(output);
    write!(&mut cursor, "{value:?}")?;
    Ok(cursor.position() as usize)
}

#[test]
fn document_debug_reports_authored_elements_attributes_and_text() -> Result<(), String> {
    let document = parse_document(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="marker">hello</text></svg>"#,
    )?;
    let debug = format!("{document:?}");
    let element = format!(
        "{:?}",
        document
            .element_by_id("marker")
            .ok_or_else(|| "id-linked element missing".to_owned())?
    );
    let text = document
        .descendants()
        .find(|node| node.is_text())
        .ok_or_else(|| "text child missing".to_owned())?;

    assert!(debug.contains("tag_name: Some(svg)"));
    assert!(debug.contains("value: marker"));
    assert!(debug.contains("Text(\"hello\")"));
    assert!(element.contains("tag_name: Some(text)"));
    assert!(element.contains("value: marker"));
    assert_eq!(format!("{text:?}"), "Text(\"hello\")");
    Ok(())
}

#[test]
fn empty_document_debug_uses_the_document_empty_contract() {
    let document = Document {
        nodes: vec![NodeData {
            parent: None,
            next_sibling: None,
            children: None,
            kind: NodeKind::Root,
        }],
        attrs: Vec::new(),
        links: Default::default(),
    };

    assert_eq!(format!("{document:?}"), "Document []");
}

#[test]
fn document_debug_propagates_fixed_buffer_write_failures() -> Result<(), String> {
    let document = parse_document(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><rect id="wide" width="123" height="45"/></svg>"#,
    )?;
    let expected = format!("{document:?}");

    for capacity in 0..expected.len() {
        let mut buffer = vec![0; capacity];
        assert!(write_debug(&document, &mut buffer).is_err());
    }

    let mut buffer = vec![0; expected.len()];
    let written = write_debug(&document, &mut buffer).map_err(|error| error.to_string())?;
    assert_eq!(written, expected.len());
    assert_eq!(&buffer[..written], expected.as_bytes());
    Ok(())
}

#[test]
fn node_text_and_root_debug_follow_node_kinds() -> Result<(), String> {
    let source = format!(
        "<svg xmlns=\"{SVG_NS}\"><g id=\"empty\"/><g id=\"element-first\"><rect/>tail</g><text id=\"plain\">hello</text></svg>"
    );
    let document = parse_document(&source)?;
    let root = document.root();
    assert_eq!(root.text(), "");
    assert_eq!(format!("{root:?}"), "Root");
    assert_non_element_has_no_attributes(root);
    assert_eq!(node_by_id(&document, "empty")?.text(), "");
    assert_eq!(node_by_id(&document, "element-first")?.text(), "");
    let plain = node_by_id(&document, "plain")?;
    let text = plain.first_child().ok_or("text child missing")?;
    assert_eq!(plain.text(), "hello");
    assert_eq!(text.text(), "hello");
    assert_non_element_has_no_attributes(text);
    Ok(())
}

fn assert_non_element_has_no_attributes(node: SvgNode<'_, '_>) {
    assert!(!node.is_element());
    assert!(node.attributes().is_empty());
    assert_eq!(node.element_id(), "");
    assert_eq!(node.attribute_id(AId::Id), None);
}

fn node_by_id<'a, 'input: 'a>(
    document: &'a Document<'input>,
    id: &str,
) -> Result<SvgNode<'a, 'input>, String> {
    document
        .element_by_id(id)
        .ok_or_else(|| format!("missing element #{id}"))
}

#[test]
fn href_iterator_follows_links_and_stops_self_and_origin_cycles() -> Result<(), String> {
    let source = format!(
        "<svg xmlns=\"{SVG_NS}\" xmlns:xlink=\"{XLINK_NS}\"><g id=\"a\" xlink:href=\"#b\"/><g id=\"b\" xlink:href=\"#c\"/><g id=\"c\"/><g id=\"self\" xlink:href=\"#self\"/><g id=\"loop-a\" xlink:href=\"#loop-b\"/><g id=\"loop-b\" xlink:href=\"#loop-a\"/></svg>"
    );
    let document = parse_document(&source)?;
    let chain = node_by_id(&document, "a")?
        .href_iter()
        .map(|node| node.element_id())
        .collect::<Vec<_>>();
    assert_eq!(chain, vec!["a", "b", "c"]);
    assert_href_cycle(&document, "self", &["self"])?;
    assert_href_cycle(&document, "loop-a", &["loop-a", "loop-b"])
}

#[test]
fn href_iterator_stops_a_cycle_that_excludes_its_origin() -> Result<(), String> {
    let source = format!(
        "<svg xmlns=\"{SVG_NS}\" xmlns:xlink=\"{XLINK_NS}\"><g id=\"tail-a\" xlink:href=\"#tail-b\"/><g id=\"tail-b\" xlink:href=\"#tail-c\"/><g id=\"tail-c\" xlink:href=\"#tail-b\"/></svg>"
    );
    let document = parse_document(&source)?;
    let mut iterator = node_by_id(&document, "tail-a")?.href_iter();
    let links = iterator
        .by_ref()
        .take(TAIL_CYCLE_EXPECTED_NODES + CYCLE_EXTRA_NODE_PROBE)
        .map(|node| node.element_id())
        .collect::<Vec<_>>();
    assert_eq!(links, vec!["tail-a", "tail-b", "tail-c"]);
    assert!(iterator.next().is_none());
    assert!(iterator.next().is_none());
    Ok(())
}

fn assert_href_cycle(
    document: &Document<'_>,
    origin: &str,
    expected: &[&str],
) -> Result<(), String> {
    let mut links = node_by_id(document, origin)?.href_iter();
    for id in expected {
        assert_eq!(links.next().map(|node| node.element_id()), Some(*id));
    }
    assert!(links.next().is_none());
    assert!(links.next().is_none());
    Ok(())
}

#[test]
fn typed_attributes_parse_valid_units_aliases_and_paints() -> Result<(), String> {
    let source = format!(
        "<svg xmlns=\"{SVG_NS}\"><g id=\"values\" opacity=\"25%\" enable-background=\"accumulate\" fill=\"none\" stroke=\"none\" stroke-dasharray=\"2 3px 50%\" visibility=\"collapse\" shape-rendering=\"auto\" text-rendering=\"auto\" image-rendering=\"optimizeSpeed\"/><g id=\"paint\" fill=\"red\"/></svg>"
    );
    let document = parse_document(&source)?;
    let values = node_by_id(&document, "values")?;
    assert_valid_value_attributes(values);
    assert_valid_rendering_attributes(values);
    assert!(
        node_by_id(&document, "paint")?
            .try_attribute::<svgtypes::Paint<'_>>(AId::Fill)
            .is_some()
    );
    Ok(())
}

fn assert_valid_value_attributes(values: SvgNode<'_, '_>) {
    assert_eq!(
        values
            .try_attribute::<Opacity>(AId::Opacity)
            .map(Opacity::get),
        Some(EXPECTED_QUARTER_OPACITY)
    );
    assert!(
        values
            .try_attribute::<svgtypes::EnableBackground>(AId::EnableBackground)
            .is_some()
    );
    assert!(
        values
            .try_attribute::<svgtypes::Paint<'_>>(AId::Fill)
            .is_some()
    );
    assert!(
        values
            .attribute::<svgtypes::Paint<'_>>(AId::Stroke)
            .is_none()
    );
    assert_eq!(
        values
            .try_attribute::<Vec<svgtypes::Length>>(AId::StrokeDasharray)
            .map(|lengths| lengths.len()),
        Some(DASH_ARRAY_SIZE)
    );
}

fn assert_valid_rendering_attributes(values: SvgNode<'_, '_>) {
    assert_eq!(
        values.try_attribute::<super::Visibility>(AId::Visibility),
        Some(super::Visibility::Collapse)
    );
    assert_eq!(
        values.try_attribute::<ShapeRendering>(AId::ShapeRendering),
        Some(ShapeRendering::GeometricPrecision)
    );
    assert_eq!(
        values.try_attribute::<TextRendering>(AId::TextRendering),
        Some(TextRendering::OptimizeLegibility)
    );
    assert_eq!(
        values.try_attribute::<super::ImageRendering>(AId::ImageRendering),
        Some(super::ImageRendering::OptimizeSpeed)
    );
}

#[test]
fn typed_attributes_reject_invalid_units_and_enum_values() -> Result<(), String> {
    let source = format!(
        "<svg xmlns=\"{SVG_NS}\"><g id=\"bad\" opacity=\"5px\" enable-background=\"unknown\" fill=\"not-a-paint\" stroke-dasharray=\"2 bad\" visibility=\"unknown\" shape-rendering=\"unknown\" text-rendering=\"unknown\" image-rendering=\"unknown\"/></svg>"
    );
    let document = parse_document(&source)?;
    let node = node_by_id(&document, "bad")?;
    assert_invalid_value_attributes(node);
    assert_invalid_rendering_attributes(node);
    Ok(())
}

fn assert_invalid_value_attributes(node: SvgNode<'_, '_>) {
    assert!(node.try_attribute::<Opacity>(AId::Opacity).is_none());
    assert!(
        node.try_attribute::<svgtypes::EnableBackground>(AId::EnableBackground)
            .is_none()
    );
    assert!(
        node.try_attribute::<svgtypes::Paint<'_>>(AId::Fill)
            .is_none()
    );
    assert!(
        node.try_attribute::<Vec<svgtypes::Length>>(AId::StrokeDasharray)
            .is_none()
    );
}

fn assert_invalid_rendering_attributes(node: SvgNode<'_, '_>) {
    assert!(
        node.try_attribute::<super::Visibility>(AId::Visibility)
            .is_none()
    );
    assert!(
        node.try_attribute::<ShapeRendering>(AId::ShapeRendering)
            .is_none()
    );
    assert!(
        node.try_attribute::<TextRendering>(AId::TextRendering)
            .is_none()
    );
    assert!(
        node.try_attribute::<super::ImageRendering>(AId::ImageRendering)
            .is_none()
    );
}
