use std::io::{self, Cursor, Write};

use super::{Document, NodeData, NodeKind};

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
