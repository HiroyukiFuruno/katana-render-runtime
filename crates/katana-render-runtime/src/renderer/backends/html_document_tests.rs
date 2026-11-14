use self::test_support::{
    assert_missing_node, assert_svg_image_attributes, assert_svg_namespace_mappings,
    embedded_svg_markup,
};
use super::{HtmlDocument, HtmlDocumentNode};
use std::collections::HashMap;

#[path = "html_document_test_support.rs"]
mod test_support;

#[test]
fn document_root_is_never_projected_as_an_interactive_node() -> Result<(), String> {
    let document = HtmlDocument::parse("<p>Visible</p>");
    let nodes = document.interactive_nodes_with_styles(&HashMap::new());
    let [HtmlDocumentNode::Element { node_id, tag, .. }] = nodes.as_slice() else {
        return Err("the HTML element must remain the rendered root".to_string());
    };
    assert_eq!(tag, "html");
    assert_eq!(document.tag_name(*node_id).as_deref(), Ok("HTML"));
    assert_ne!(
        document
            .node_ids
            .get(&(std::rc::Rc::as_ptr(&document.document) as usize)),
        Some(node_id)
    );
    Ok(())
}

#[test]
fn embedded_svg_projection_preserves_vector_markup_and_attribute_case() -> Result<(), String> {
    let document = HtmlDocument::parse(
        r##"<style>svg { max-width: 90px; }</style><svg viewBox="0 0 120 80" preserveAspectRatio="xMidYMid meet" xmlns:xlink="http://www.w3.org/1999/xlink"><!-- marker --><defs><marker id="dot" markerHeight="8"><path d="M0 0L8 4L0 8Z"/></marker></defs><use xlink:href="#dot"/><circle cx="40" cy="40" r="20" fill="#22c55e"/></svg>"##,
    );
    let nodes = document.interactive_nodes_with_styles(&HashMap::new());
    let markup =
        embedded_svg_markup(&nodes).ok_or_else(|| "embedded SVG markup must exist".to_string())?;

    assert!(markup.contains("viewBox=\"0 0 120 80\""), "{markup}");
    assert!(markup.contains("preserveAspectRatio=\"xMidYMid meet\""));
    assert!(markup.contains("markerHeight=\"8\""));
    assert!(markup.contains("xlink:href=\"#dot\""));
    assert!(markup.contains("<path"));
    assert!(markup.contains("max-width: 90px"));
    assert_eq!(
        embedded_svg_markup(&[HtmlDocumentNode::Text("plain".to_string())]),
        None
    );
    Ok(())
}

#[test]
fn natural_svg_projection_preserves_xml_namespaces_and_xlink_attributes() -> Result<(), String> {
    let document = HtmlDocument::parse(
        r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"><title>diagram icon</title><image xlink:href="asset.svg"/></svg>"##,
    );
    let nodes = document.interactive_nodes_with_styles(&HashMap::new());
    let markup =
        embedded_svg_markup(&nodes).ok_or_else(|| "embedded SVG markup must exist".to_string())?;
    assert!(markup.contains("<title>diagram icon</title>"), "{markup}");
    let parsed_svg = xmltree::Element::parse(markup.as_bytes());
    assert!(
        parsed_svg.is_ok(),
        "embedded SVG should parse; markup={markup}"
    );
    let parsed_svg = parsed_svg.map_err(|error| format!("unexpected test error: {error}"))?;
    assert_svg_namespace_mappings(&parsed_svg)?;
    assert_svg_image_attributes(&parsed_svg, markup)?;
    Ok(())
}

#[test]
fn rejects_container_and_attribute_operations_on_text_nodes() -> Result<(), String> {
    let mut document = HtmlDocument::parse("<p id=target>Visible</p>");
    let target = document
        .get_element_by_id("target")
        .ok_or_else(|| "target element must exist".to_string())?;
    document
        .set_text_content(target, "replacement")
        .map_err(|error| format!("unexpected test error: {error}"))?;
    let text_node = document.next_node_id - 1;

    let append = document.append_child(text_node, target);
    let attribute = document.attribute(text_node, "class");
    let set_attribute = document.set_attribute(text_node, "class", "note");

    assert!(matches!(append, Err(error) if error.contains("not a container")));
    assert!(matches!(attribute, Err(error) if error.contains("not an element")));
    assert!(matches!(set_attribute, Err(error) if error.contains("not an element")));
    Ok(())
}

#[test]
fn rejects_external_scripts_and_missing_selectors() {
    let mut document =
        HtmlDocument::parse("<script src=\"app.js\"></script><p class=visible>Visible</p>");

    assert!(matches!(
        document.inline_scripts(),
        Err(error) if error.contains("external script is not supported: app.js")
    ));
    assert_eq!(document.query_selector(".missing"), None);
    assert!(document.query_selector_all("main + p").is_empty());
    assert_eq!(document.get_element_by_id("missing"), None);
}

#[test]
fn closest_selector_returns_no_match_for_invalid_selector_syntax() -> Result<(), String> {
    let mut document = HtmlDocument::parse("<main id=target>Visible</main>");
    let target = document
        .get_element_by_id("target")
        .ok_or_else(|| "target must exist".to_string())?;

    assert_eq!(document.closest_selector(target, "main +"), Ok(None));
    Ok(())
}

#[test]
fn rejects_operations_on_missing_nodes() -> Result<(), String> {
    let mut document = HtmlDocument::parse("<p id=target>Visible</p>");
    let target = document
        .get_element_by_id("target")
        .ok_or_else(|| "target must exist".to_string())?;

    assert_missing_node(document.append_child(999, target));
    assert_missing_node(document.append_child(target, 999));
    assert_missing_node(document.remove(999));
    assert_missing_node(document.text_content(999));
    assert_missing_node(document.set_text_content(999, "replacement"));
    assert_missing_node(document.set_attribute(999, "class", "note"));
    Ok(())
}
