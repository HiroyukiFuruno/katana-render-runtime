use std::collections::{HashMap, HashSet};

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
#[path = "session_snapshot.rs"]
mod session_snapshot;

type HtmlNodeIds = HashSet<u64>;

#[derive(Debug, Clone, Default)]
pub(crate) struct StaticHtmlRuntime;

pub(crate) struct StaticHtmlRuntimeSession {
    context: Option<v8::Global<v8::Context>>,
    isolate: Option<v8::OwnedIsolate>,
    external_stylesheets: HashMap<String, String>,
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
