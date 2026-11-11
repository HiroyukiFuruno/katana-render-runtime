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

const BLUR_CHANGE_CASES: [(&str, &[&str], &str); 4] = [
    ("untouched", &[], ">initial<"),
    ("empty-text", &[""], ">initial<"),
    ("typed-text", &["typed"], ">changed<"),
    ("typed-then-empty", &["typed", ""], ">changed<"),
];
const BLUR_CHANGE_CONTROLS: [(&str, &str); 3] = [
    ("text", "<input id=target>"),
    ("textarea", "<textarea id=target></textarea>"),
    ("search", "<input id=target type=search>"),
];

#[test]
fn blur_dispatches_change_only_after_text_changes_value() -> Result<(), String> {
    for (control, markup) in BLUR_CHANGE_CONTROLS {
        let html = blur_change_html(markup);
        for (case, texts, expected) in BLUR_CHANGE_CASES {
            let session = blur_input_session(&html, texts)?;
            assert_blur_result(&session, expected, &format!("{control}-{case}"))?;
        }
    }
    Ok(())
}

fn blur_change_html(control_markup: &str) -> String {
    format!(
        r#"{control_markup}<p id=result>initial</p><script>
document.getElementById('target').addEventListener('change', () => {{
  document.getElementById('result').textContent = 'changed';
}});</script>"#
    )
}

fn blur_input_session(html: &str, texts: &[&str]) -> Result<HtmlInteractiveSession, String> {
    use crate::renderer::backends::html_browser::HtmlBrowserInput;
    let mut session = listener_error_session(html).map_err(|error| error.to_string())?;
    let node = session
        .runtime
        .node_for_element_id("target")
        .ok_or("missing input")?;
    session
        .focus_input_without_frame(node.0)
        .map_err(|error| error.to_string())?;
    for &text in texts {
        session
            .dispatch_input(HtmlBrowserInput::Text {
                text: text.to_owned(),
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
    case: &str,
) -> Result<(), String> {
    let snapshot = session
        .runtime
        .snapshot()
        .map_err(|error| error.to_string())?;
    assert!(snapshot.contains(expected), "case={case}: {snapshot}");
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
fn focus_listener_errors_propagate_for_pointer_and_keyboard_focus() -> Result<(), String> {
    for pointer in [true, false] {
        let html = "<button id=target>Focus</button><script>document.getElementById('target').addEventListener('focus', () => { throw new Error('focus failed'); });</script>";
        let mut session = listener_error_session(html).map_err(|error| error.to_string())?;
        let node = session
            .runtime
            .node_for_element_id("target")
            .ok_or("missing focus target")?;
        let result = session.focus_node(node.0, pointer);
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
        .focus_node(node.0, false)
        .map_err(|error| error.to_string())?;
    assert_eq!(session.focused_node, Some(node.0));
    session
        .dispatch_key_down("Tab".to_string())
        .map_err(|error| error.to_string())?;
    assert_eq!(session.focused_node, None);
    assert!(!session.focus_visible);
    Ok(())
}

#[test]
fn focus_and_blur_propagate_active_element_bridge_exceptions() -> Result<(), String> {
    for phase in ["focus", "blur"] {
        let script = if phase == "focus" {
            "globalThis.__krrSetFocusedNodeId = () => { throw new Error('active bridge failed'); };"
        } else {
            "document.getElementById('target').addEventListener('focus', () => { globalThis.__krrSetFocusedNodeId = () => { throw new Error('active bridge failed'); }; });"
        };
        let html = format!("<button id=target>Focus</button><script>{script}</script>");
        let mut session = listener_error_session(&html).map_err(|error| error.to_string())?;
        let node = session
            .runtime
            .node_for_element_id("target")
            .ok_or("missing target")?;
        let result = if phase == "focus" {
            session.focus_node(node.0, true)
        } else {
            session
                .focus_node(node.0, true)
                .map_err(|error| error.to_string())?;
            session.update_focus(false)
        };
        assert!(
            matches!(result, Err(error) if error.to_string().contains("active bridge failed")),
            "{phase}"
        );
    }
    Ok(())
}
