use super::super::{HtmlBrowserError, HtmlInteractiveSession};
use crate::renderer::backends::{HtmlBrowserSource, HtmlBrowserViewport};

const VIEWPORT_WIDTH: u32 = 320;
const VIEWPORT_HEIGHT: u32 = 240;

fn start_minimal_session() -> Result<HtmlInteractiveSession, HtmlBrowserError> {
    listener_error_session("<p>focus coverage</p>")
}

fn listener_error_session(html: &str) -> Result<HtmlInteractiveSession, HtmlBrowserError> {
    let source = HtmlBrowserSource::new(html, "https://example.com/focus.html")?;
    let viewport = HtmlBrowserViewport::new(VIEWPORT_WIDTH, VIEWPORT_HEIGHT, 1.0)?;
    HtmlInteractiveSession::start(source, viewport)
}

const BLUR_CHANGE_CASES: [(bool, &str); 2] = [(false, ">initial<"), (true, ">changed<")];
const BLUR_CHANGE_HTML: &str = r#"<input id=target><p id=result>initial</p><script>
document.getElementById('target').addEventListener('change', () => {
  document.getElementById('result').textContent = 'changed';
});</script>"#;

#[test]
fn blur_dispatches_change_only_for_a_dirty_real_input() -> Result<(), String> {
    for (dirty, expected) in BLUR_CHANGE_CASES {
        let session = blur_input_session(BLUR_CHANGE_HTML, dirty)?;
        assert_blur_result(&session, expected, dirty)?;
    }
    Ok(())
}

fn blur_input_session(html: &str, dirty: bool) -> Result<HtmlInteractiveSession, String> {
    use crate::renderer::backends::html_browser::HtmlBrowserInput;
    let mut session = listener_error_session(html).map_err(|error| error.to_string())?;
    let node = session
        .runtime
        .node_for_element_id("target")
        .ok_or("missing input")?;
    session
        .focus_input_without_frame(node.0)
        .map_err(|error| error.to_string())?;
    if dirty {
        session
            .dispatch_input(HtmlBrowserInput::Text {
                text: "typed".to_owned(),
            })
            .map_err(|error| error.to_string())?;
    }
    session
        .update_focus(false)
        .map_err(|error| error.to_string())?;
    Ok(session)
}

fn assert_blur_result(
    session: &HtmlInteractiveSession,
    expected: &str,
    dirty: bool,
) -> Result<(), String> {
    let snapshot = session
        .runtime
        .snapshot()
        .map_err(|error| error.to_string())?;
    assert!(snapshot.contains(expected), "dirty={dirty}: {snapshot}");
    assert_eq!(session.focused_node, None);
    assert_eq!(session.focused_input, None);
    Ok(())
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

#[test]
fn pointer_to_keyboard_refocus_renders_visible_focus_for_each_control() -> Result<(), String> {
    let controls = [
        ("button", "<button id=target>Focus</button>"),
        ("link", "<a id=target href=/next>Focus</a>"),
    ];
    for (kind, markup) in controls {
        assert_refocus_visibility(kind, markup)?;
    }
    Ok(())
}

fn assert_refocus_visibility(kind: &str, markup: &str) -> Result<(), String> {
    let mut session = listener_error_session(markup).map_err(|error| error.to_string())?;
    let node = session
        .runtime
        .node_for_element_id("target")
        .ok_or_else(|| format!("missing {kind}"))?;
    session
        .focus_node(node.0, true, true)
        .map_err(|error| error.to_string())?;
    assert!(!session.focus_visible, "pointer focus ring for {kind}");
    let generation = session.generation;
    session
        .focus_node(node.0, false, true)
        .map_err(|error| error.to_string())?;
    assert!(session.focus_visible, "keyboard focus for {kind}");
    assert_eq!(
        session.latest_frame().map(|frame| frame.generation),
        Some(generation + 1)
    );
    Ok(())
}

#[test]
fn focus_listener_errors_propagate_for_pointer_and_keyboard_focus() -> Result<(), String> {
    for pointer in [true, false] {
        let html = "<button id=target>Focus</button><script>document.getElementById('target').addEventListener('focus', () => { throw new Error('focus failed'); });</script>";
        let mut session = listener_error_session(html).map_err(|error| error.to_string())?;
        let node = session
            .runtime
            .node_for_element_id("target")
            .ok_or("missing focus target")?;
        let result = session.focus_node(node.0, pointer, false);
        assert!(matches!(result, Err(error) if error.to_string().contains("focus failed")));
    }
    Ok(())
}

#[test]
fn key_dispatch_without_body_ignores_keys_and_tab_renders() -> Result<(), String> {
    let html = r#"<body><script>
document.addEventListener('keydown', event => {
  if (event.key === 'ArrowRight') document.body.remove();
});
</script></body>"#;
    let mut session = listener_error_session(html).map_err(|error| error.to_string())?;
    session
        .dispatch_key_down("ArrowRight".to_string())
        .map_err(|error| error.to_string())?;
    assert!(session.runtime.body_node().is_none());
    let generation = session.generation;
    session
        .dispatch_key_down("ArrowRight".to_string())
        .map_err(|error| error.to_string())?;
    assert_eq!(session.generation, generation);
    session
        .dispatch_key_down("Tab".to_string())
        .map_err(|error| error.to_string())?;
    assert_eq!(session.generation, generation + 1);
    assert_eq!(session.focused_node, None);
    Ok(())
}

#[test]
fn tab_blurs_focus_after_its_real_dom_target_is_removed() -> Result<(), String> {
    let html = r#"<button id=target>Focus</button><script>
document.addEventListener('keydown', event => {
  if (event.key === 'Tab') document.getElementById('target').remove();
});
</script>"#;
    let mut session = listener_error_session(html).map_err(|error| error.to_string())?;
    let node = session
        .runtime
        .node_for_element_id("target")
        .ok_or("missing focus target")?;
    session
        .focus_node(node.0, false, false)
        .map_err(|error| error.to_string())?;
    assert_eq!(session.focused_node, Some(node.0));
    session
        .dispatch_key_down("Tab".to_string())
        .map_err(|error| error.to_string())?;
    assert_eq!(session.focused_node, None);
    assert!(!session.focus_visible);
    Ok(())
}
