use super::super::{HtmlBrowserError, HtmlInteractiveSession};
use crate::renderer::backends::{HtmlBrowserSource, HtmlBrowserViewport};

const VIEWPORT_WIDTH: u32 = 320;
const VIEWPORT_HEIGHT: u32 = 240;

fn start_minimal_session() -> Result<HtmlInteractiveSession, HtmlBrowserError> {
    let source = HtmlBrowserSource::new("<p>focus coverage</p>", "https://example.com/focus.html")?;
    let viewport = HtmlBrowserViewport::new(VIEWPORT_WIDTH, VIEWPORT_HEIGHT, 1.0)?;
    HtmlInteractiveSession::start(source, viewport)
}

fn listener_error_session(html: &str) -> Result<HtmlInteractiveSession, HtmlBrowserError> {
    let source = HtmlBrowserSource::new(html, "https://example.com/focus.html")?;
    let viewport = HtmlBrowserViewport::new(VIEWPORT_WIDTH, VIEWPORT_HEIGHT, 1.0)?;
    HtmlInteractiveSession::start(source, viewport)
}

#[test]
fn commit_focused_input_is_noop_when_not_focusing() -> Result<(), HtmlBrowserError> {
    let mut session = start_minimal_session()?;
    assert_eq!(session.focused_input, None);
    session.commit_focused_input(true)?;
    assert_eq!(session.focused_input, None);
    Ok(())
}

#[test]
fn dispatch_key_down_for_unfocused_input_does_not_try_to_commit() -> Result<(), HtmlBrowserError> {
    let mut session = start_minimal_session()?;
    session.dispatch_key_down("Enter".to_string())?;
    assert_eq!(session.focused_input, None);
    Ok(())
}

#[test]
fn dispatch_key_down_propagates_listener_errors() -> Result<(), HtmlBrowserError> {
    let html = "<body><script>document.addEventListener('keydown', () => { throw new Error('keydown failed'); });</script></body>";
    let mut session = listener_error_session(html)?;
    assert!(
        matches!(session.dispatch_key_down("ArrowRight".to_string()), Err(error)
        if error.to_string().contains("keydown failed"))
    );
    Ok(())
}

#[test]
fn commit_focused_input_propagates_change_listener_errors() -> Result<(), String> {
    let html = "<input id=target><script>document.getElementById('target').addEventListener('change', () => { throw new Error('change failed'); });</script>";
    let mut session = listener_error_session(html).map_err(|error| error.to_string())?;
    let node_id = session
        .runtime
        .node_for_element_id("target")
        .ok_or_else(|| "test input element not found".to_string())?;
    session.focused_input = Some(node_id.0);
    session.dirty_inputs.insert(node_id.0);
    assert!(matches!(session.commit_focused_input(true), Err(error)
        if error.to_string().contains("change failed")));
    Ok(())
}
