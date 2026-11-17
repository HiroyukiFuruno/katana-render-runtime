use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use super::super::html_document::HtmlDocument;
use super::dom_state::HtmlDomBridgeState;
use super::types::HtmlRuntimeError;

#[cfg(test)]
#[path = "session_feature_diagnostic_tests.rs"]
mod session_feature_diagnostic;

#[path = "session_interaction.rs"]
mod session_interaction;
#[path = "session_interactive_start.rs"]
mod session_interactive_start;
#[cfg(test)]
#[path = "session_intersection_constructor_tests.rs"]
mod session_intersection_constructor_tests;
#[path = "session_javascript_requirement.rs"]
mod session_javascript_requirement;
#[path = "session_layout_metrics.rs"]
mod session_layout_metrics;
#[cfg(test)]
#[path = "session_layout_metrics_intersection_registry_tests.rs"]
mod session_layout_metrics_intersection_registry_tests;
#[cfg(test)]
#[path = "session_layout_metrics_intersection_tests.rs"]
mod session_layout_metrics_intersection_tests;
#[path = "session_runtime.rs"]
mod session_runtime;
#[path = "session_runtime_interactive.rs"]
mod session_runtime_interactive;
#[path = "session_runtime_static.rs"]
mod session_runtime_static;
#[cfg(test)]
#[path = "session_scriptless_observer.rs"]
mod session_scriptless_observer;
#[cfg(test)]
#[path = "session_scriptless_tests.rs"]
mod session_scriptless_tests;
#[path = "session_snapshot.rs"]
mod session_snapshot;
#[path = "session_timer_pump.rs"]
mod session_timer_pump;

type HtmlNodeIds = HashSet<u64>;
#[cfg(not(test))]
type ScriptlessDocument = Box<RefCell<HtmlDocument>>;
#[cfg(test)]
type ScriptlessDocument = std::rc::Rc<RefCell<HtmlDocument>>;

#[derive(Debug, Clone, Default)]
pub(crate) struct StaticHtmlRuntime;

pub(crate) struct StaticHtmlRuntimeSession {
    context: Option<v8::Global<v8::Context>>,
    isolate: Option<v8::OwnedIsolate>,
    scriptless_document: Option<ScriptlessDocument>,
    #[cfg(test)]
    scriptless_observer: Option<Box<session_scriptless_observer::ScriptlessObserver>>,
    external_stylesheets: HashMap<String, String>,
}

impl StaticHtmlRuntimeSession {
    fn scripted(
        context: v8::Global<v8::Context>,
        isolate: v8::OwnedIsolate,
        external_stylesheets: HashMap<String, String>,
    ) -> Self {
        Self {
            context: Some(context),
            isolate: Some(isolate),
            scriptless_document: None,
            #[cfg(test)]
            scriptless_observer: None,
            external_stylesheets,
        }
    }

    fn dom_document(&self) -> Result<&RefCell<HtmlDocument>, HtmlRuntimeError> {
        if let Some(document) = &self.scriptless_document {
            return Ok(document);
        }
        Ok(&self.dom_bridge_state()?.document)
    }

    fn dom_bridge_state(&self) -> Result<&HtmlDomBridgeState, HtmlRuntimeError> {
        self.isolate
            .as_ref()
            .ok_or_else(session_interaction::discarded_runtime_error)?
            .get_slot::<HtmlDomBridgeState>()
            .ok_or_else(super::script::dom_state_unavailable_error)
    }
}

impl StaticHtmlRuntime {
    fn with_new_context(
        isolate: &mut v8::OwnedIsolate,
        execute: impl for<'borrow, 'scope, 'isolate> FnOnce(
            &mut v8::ContextScope<'borrow, 'scope, v8::HandleScope<'isolate>>,
            v8::Local<'scope, v8::Context>,
        ) -> Result<
            v8::Global<v8::Context>,
            super::types::HtmlRuntimeError,
        >,
    ) -> Result<v8::Global<v8::Context>, super::types::HtmlRuntimeError> {
        v8::scope!(let handle_scope, isolate);
        let context = v8::Context::new(handle_scope, Default::default());
        let context_scope = &mut v8::ContextScope::new(handle_scope, context);
        execute(context_scope, context)
    }
}

use crate::private_v8 as v8;
