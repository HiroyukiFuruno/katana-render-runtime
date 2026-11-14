use super::{must_node, must_session};
use crate::renderer::backends::{HtmlNodeId, HtmlRuntimeError, HtmlRuntimeEvent};

#[test]
fn invalid_click_target_reports_dom_bridge_error() {
    let mut session = must_session("<button id=action>Run</button>");
    let dispatch = session.dispatch(HtmlRuntimeEvent::Click {
        target: HtmlNodeId(9999),
    });
    assert!(matches!(dispatch, Err(HtmlRuntimeError::DomBridge(_))));
}

#[test]
fn returns_v8_exceptions_from_click_handlers() {
    let mut session = must_session(
        "<button id=action>Run</button><script>document.getElementById('action').addEventListener('click', () => { throw new Error('listener'); });</script>",
    );
    let action = must_node(&mut session, "action");
    let dispatch = session.dispatch(HtmlRuntimeEvent::Click { target: action });
    assert!(matches!(
        dispatch,
        Err(HtmlRuntimeError::JavaScriptException(message)) if message.contains("listener")
    ));
}

#[test]
fn returns_v8_exceptions_from_inline_click_handlers() {
    let mut session =
        must_session("<button id=action onclick=\"throw new Error('inline')\">Run</button>");
    let action = must_node(&mut session, "action");
    let dispatch = session.dispatch(HtmlRuntimeEvent::Click { target: action });
    assert!(matches!(
        dispatch,
        Err(HtmlRuntimeError::JavaScriptException(message)) if message.contains("inline")
    ));
}

#[test]
fn inline_click_handler_timeout_discards_the_v8_session() {
    assert_timeout_discards_session(
        "<button id=action onclick=\"for (;;) {}\">Run</button>",
        "action",
    );
}

#[test]
fn discards_the_v8_session_after_event_timeout() {
    assert_timeout_discards_session(
        "<button id=action>Run</button><script>document.getElementById('action').addEventListener('click', () => { for (;;) {} });</script>",
        "action",
    );
}

#[test]
fn window_scroll_timeout_discards_the_v8_session() {
    let mut session =
        must_session("<script>window.addEventListener('scroll', () => { for (;;) {} });</script>");
    let dispatch = session.dispatch_window_scroll();
    assert!(matches!(dispatch, Err(HtmlRuntimeError::ExecutionTimeout)));
    assert!(matches!(
        session.snapshot(),
        Err(HtmlRuntimeError::DomBridge(message)) if message.contains("discarded")
    ));
}

fn assert_timeout_discards_session(source: &str, element_id: &str) {
    let mut session = must_session(source);
    let action = must_node(&mut session, element_id);
    let dispatch = session.dispatch(HtmlRuntimeEvent::Click { target: action });
    assert!(matches!(dispatch, Err(HtmlRuntimeError::ExecutionTimeout)));
    assert!(session.node_for_element_id(element_id).is_none());
    assert!(matches!(
        session.snapshot(),
        Err(HtmlRuntimeError::DomBridge(message)) if message.contains("discarded")
    ));
}
