use std::collections::{HashMap, HashSet};

#[path = "session_interaction.rs"]
mod session_interaction;
#[path = "session_interactive_start.rs"]
mod session_interactive_start;
#[path = "session_layout_metrics.rs"]
mod session_layout_metrics;
#[path = "session_runtime.rs"]
mod session_runtime;
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
