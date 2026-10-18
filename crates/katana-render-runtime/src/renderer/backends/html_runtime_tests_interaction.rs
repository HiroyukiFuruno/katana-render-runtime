#[path = "html_runtime_tests_interaction_events.rs"]
mod interaction_events;

#[path = "html_runtime_tests_interaction_navigation.rs"]
mod interaction_navigation;

#[path = "html_runtime_tests_interaction_session.rs"]
mod interaction_session;

use crate::renderer::backends::HtmlNodeId;
use crate::renderer::backends::html_runtime::{StaticHtmlRuntime, StaticHtmlRuntimeSession};

fn must_session(source: &str) -> StaticHtmlRuntimeSession {
    must_result(StaticHtmlRuntime.start(source))
}

fn must_node(session: &mut StaticHtmlRuntimeSession, element_id: &str) -> HtmlNodeId {
    let node = session.node_for_element_id(element_id);
    assert!(node.is_some());
    let mut nodes = node.into_iter().collect::<Vec<_>>();
    nodes.remove(0)
}

fn must_result<T, E>(result: Result<T, E>) -> T {
    assert!(result.is_ok());
    let mut values = result.into_iter().collect::<Vec<_>>();
    values.remove(0)
}
