use xmltree::{Element, XMLNode};

pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

pub(super) fn find_svg(root: &Element) -> Option<&Element> {
    root.children.iter().find_map(|node| match node {
        XMLNode::Element(element) if element.name == "svg" => Some(element),
        XMLNode::Element(element) => find_svg(element),
        _ => None,
    })
}

pub(super) fn svg_coordinate(svg: &Element, name: &str) -> TestResult<f32> {
    Ok(svg
        .attributes
        .get(name)
        .ok_or_else(|| format!("missing SVG {name}"))?
        .parse::<f32>()?)
}

pub(super) fn capture_frame_pixels(
    html: &str,
    url: &str,
    width: u32,
    height: u32,
) -> TestResult<Vec<u8>> {
    let source = crate::HtmlBrowserSource::new(html, url)?;
    let viewport = crate::HtmlBrowserViewport::new(width, height, 1.0)?;
    let session = crate::HtmlBrowserSession::new(source, viewport)?;
    session_frame_pixels(&session)
}

pub(super) fn session_frame_pixels(session: &crate::HtmlBrowserSession) -> TestResult<Vec<u8>> {
    session
        .latest_frame()
        .map(|frame| frame.pixels.clone())
        .ok_or_else(|| "missing SVG frame".into())
}

#[cfg(test)]
#[path = "layout_media_test_support_tests.rs"]
mod tests;
