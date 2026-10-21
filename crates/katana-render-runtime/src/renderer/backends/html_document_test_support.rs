use super::super::{EMBEDDED_SVG_MARKUP_ATTRIBUTE, HtmlDocumentNode};

pub(super) fn assert_svg_namespace_mappings(svg: &xmltree::Element) -> Result<(), String> {
    assert_eq!(svg.namespace.as_deref(), Some("http://www.w3.org/2000/svg"));
    let namespaces = svg
        .namespaces
        .as_ref()
        .ok_or("SVG namespace mappings must exist")?;
    assert_eq!(namespaces.get(""), Some("http://www.w3.org/2000/svg"));
    assert_eq!(
        namespaces.get("xlink"),
        Some("http://www.w3.org/1999/xlink")
    );
    Ok(())
}

pub(super) fn assert_svg_image_attributes(
    svg: &xmltree::Element,
    markup: &str,
) -> Result<(), String> {
    let image = svg.children.iter().find_map(|child| match child {
        xmltree::XMLNode::Element(element) if element.name == "image" => Some(element),
        _ => None,
    });
    let image = image.ok_or("image element must remain in SVG markup")?;
    assert_eq!(
        image.namespace.as_deref(),
        Some("http://www.w3.org/2000/svg")
    );
    assert_eq!(
        image.attributes.get("href").map(String::as_str),
        Some("asset.svg")
    );
    assert!(markup.contains("xlink:href=\"asset.svg\""), "{markup}");
    Ok(())
}

pub(super) fn embedded_svg_markup(nodes: &[HtmlDocumentNode]) -> Option<&str> {
    nodes.iter().find_map(|node| match node {
        HtmlDocumentNode::Element {
            tag,
            attributes,
            children,
            ..
        } => (tag == "svg")
            .then(|| {
                attributes
                    .iter()
                    .find(|(name, _)| name == EMBEDDED_SVG_MARKUP_ATTRIBUTE)
                    .map(|(_, value)| value.as_str())
            })
            .flatten()
            .or_else(|| embedded_svg_markup(children)),
        HtmlDocumentNode::Text(_) => None,
    })
}

pub(super) fn assert_missing_node<T>(result: Result<T, String>) {
    assert_eq!(
        result.err(),
        Some("HTML node 999 does not exist".to_string())
    );
}
