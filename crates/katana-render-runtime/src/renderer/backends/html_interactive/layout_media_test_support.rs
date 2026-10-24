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

pub(super) fn capture_frame_pixels(
    html: &str,
    url: &str,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, String> {
    let source = crate::HtmlBrowserSource::new(html, url).map_err(|error| error.to_string())?;
    let viewport =
        crate::HtmlBrowserViewport::new(width, height, 1.0).map_err(|error| error.to_string())?;
    let session =
        crate::HtmlBrowserSession::new(source, viewport).map_err(|error| error.to_string())?;
    session
        .latest_frame()
        .map(|frame| frame.pixels.clone())
        .ok_or_else(|| "missing SVG frame".to_owned())
}
