use super::super::DOM_EVENT_CONTRACT_DOCUMENT;
use super::{must_node, must_result, must_session};
use crate::renderer::backends::HtmlRuntimeEvent;
use crate::renderer::backends::html_runtime::HtmlRuntimeDispatch;

#[test]
fn dom_event_options_order_cancellation_and_default_action_follow_the_event_path() {
    let mut session = must_session(DOM_EVENT_CONTRACT_DOCUMENT);
    let link = must_node(&mut session, "link");
    let first = must_result(session.dispatch(HtmlRuntimeEvent::Click { target: link }));
    let second = must_result(session.dispatch(HtmlRuntimeEvent::Click { target: link }));
    let snapshot = must_result(session.snapshot());

    assert_dom_event_contract(&first, &second, &snapshot);
}

#[test]
fn returns_navigation_intent_without_a_click_handler() {
    let mut session = must_session("<a id=link href=\"/next\">Open</a>");
    let link = must_node(&mut session, "link");
    let dispatch = must_result(session.dispatch(HtmlRuntimeEvent::Click { target: link }));

    assert_eq!(
        dispatch
            .navigation
            .as_ref()
            .map(|intent| intent.href.as_str()),
        Some("/next")
    );
}

fn assert_dom_event_contract(
    first: &HtmlRuntimeDispatch,
    second: &HtmlRuntimeDispatch,
    snapshot: &str,
) {
    assert!(first.navigation.is_none());
    assert!(second.navigation.is_none());
    let expected = "window:1:true|document:1:true|parent:1:true|once|target:2:true:true|window:1:true|document:1:true|parent:1:true|target:2:true:true|";
    assert!(
        snapshot.contains(&format!(r#"<p id="status">{expected}</p>"#)),
        "{snapshot}"
    );
    for forbidden in ["removed|", "after-immediate|", "bubble|"] {
        assert!(!snapshot.contains(forbidden), "{snapshot}");
    }
}
