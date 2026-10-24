use xmltree::{Element, XMLNode};

pub(super) fn find_svg(root: &Element) -> Option<&Element> {
    root.children.iter().find_map(|node| match node {
        XMLNode::Element(element) if element.name == "svg" => Some(element),
        XMLNode::Element(element) => find_svg(element),
        _ => None,
    })
}

pub(super) fn svg_coordinate(svg: &Element, name: &str) -> Result<f32, String> {
    svg.attributes
        .get(name)
        .ok_or_else(|| format!("missing SVG {name}"))?
        .parse::<f32>()
        .map_err(|error| error.to_string())
}
