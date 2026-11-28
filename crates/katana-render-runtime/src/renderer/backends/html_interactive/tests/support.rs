use super::super::HtmlInteractiveSession;
use super::super::document::attribute;
use crate::renderer::backends::html_browser::{
    HtmlBrowserFrame, HtmlBrowserInput, HtmlBrowserSource, HtmlBrowserViewport,
};
use crate::renderer::backends::html_document::HtmlDocumentNode;

pub(super) type TestResult<T = ()> = Result<T, String>;

pub(super) fn start(html: &str) -> TestResult<HtmlInteractiveSession> {
    start_with_viewport(html, 320, 240)
}

pub(super) fn start_with_viewport(
    html: &str,
    width: u32,
    height: u32,
) -> TestResult<HtmlInteractiveSession> {
    let source =
        HtmlBrowserSource::new(html, "https://example.test/docs/index.html").map_err(to_string)?;
    let viewport = HtmlBrowserViewport::new(width, height, 1.0).map_err(to_string)?;
    HtmlInteractiveSession::start(source, viewport).map_err(to_string)
}

pub(super) fn click_first_target(session: &mut HtmlInteractiveSession) -> TestResult {
    let target = session
        .hit_targets
        .first()
        .cloned()
        .ok_or_else(|| "interactive target must exist".to_string())?;
    click_target(session, target.x, target.y)
}

pub(super) fn click_element(session: &mut HtmlInteractiveSession, id: &str) -> TestResult {
    let node_id = session
        .runtime
        .node_for_element_id(id)
        .ok_or_else(|| format!("missing element #{id}"))?
        .0;
    let target = session
        .hit_targets
        .iter()
        .find(|target| target.node_id == node_id)
        .cloned()
        .ok_or_else(|| format!("missing interactive target #{id}"))?;
    click_target(session, target.x, target.y)
}

fn click_target(session: &mut HtmlInteractiveSession, x: f32, y: f32) -> TestResult {
    let x = x + 1.0;
    let y = y + 1.0 - session.scroll_geometry.scroll_y;
    dispatch_pointer(session, HtmlBrowserInput::PointerDown { x, y, button: 0 })?;
    dispatch_pointer(session, HtmlBrowserInput::PointerUp { x, y, button: 0 })
}

fn dispatch_pointer(session: &mut HtmlInteractiveSession, input: HtmlBrowserInput) -> TestResult {
    session.dispatch_input(input).map_err(to_string)
}

pub(super) fn has_open_details(nodes: &[HtmlDocumentNode]) -> bool {
    nodes.iter().any(has_open_details_node)
}

fn has_open_details_node(node: &HtmlDocumentNode) -> bool {
    match node {
        HtmlDocumentNode::Element {
            tag,
            attributes,
            children,
            ..
        } => {
            (tag == "details" && attribute(attributes, "open").is_some())
                || has_open_details(children)
        }
        HtmlDocumentNode::Text(_) => false,
    }
}

pub(super) fn input_value(nodes: &[HtmlDocumentNode]) -> Option<&str> {
    nodes.iter().find_map(input_value_node)
}

fn input_value_node(node: &HtmlDocumentNode) -> Option<&str> {
    match node {
        HtmlDocumentNode::Element {
            tag, attributes, ..
        } if tag == "input" => attribute(attributes, "value"),
        HtmlDocumentNode::Element { children, .. } => input_value(children),
        HtmlDocumentNode::Text(_) => None,
    }
}

pub(super) fn frame_contains_rgb(frame: &HtmlBrowserFrame, expected: [u8; 3]) -> bool {
    frame_matching_rgb_pixels(frame, expected) > 0
}

pub(super) fn frame_matching_rgb_pixels(frame: &HtmlBrowserFrame, expected: [u8; 3]) -> usize {
    frame
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| {
            pixel[0] == expected[0]
                && pixel[1] == expected[1]
                && pixel[2] == expected[2]
                && pixel[3] == 255
        })
        .count()
}

pub(super) fn to_string(error: impl ToString) -> String {
    error.to_string()
}
