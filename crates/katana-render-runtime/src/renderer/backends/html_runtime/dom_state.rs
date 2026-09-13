use self::geometry::HtmlLayoutMetrics;
use super::super::html_document::HtmlDocument;
use super::super::html_subresources::HtmlSubresourceLoader;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[path = "dom_state_content_lookup.rs"]
mod content_lookup;
#[path = "dom_state_geometry.rs"]
mod geometry;
#[path = "dom_state_lookup.rs"]
mod lookup;
#[path = "dom_state_mutation.rs"]
mod mutation;
#[path = "dom_state_request.rs"]
mod request;

pub(super) struct HtmlDomBridgeState {
    pub(super) document: RefCell<HtmlDocument>,
    pub(super) error: RefCell<Option<String>>,
    layout_metrics: RefCell<HtmlLayoutMetrics>,
    event_targets: RefCell<HashMap<String, HashSet<u64>>>,
    resource_loader: Option<HtmlSubresourceLoader>,
    host_io_active: Arc<AtomicBool>,
}

impl HtmlDomBridgeState {
    pub(super) fn new(document: HtmlDocument) -> Self {
        Self {
            document: RefCell::new(document),
            error: RefCell::new(None),
            layout_metrics: RefCell::new(HtmlLayoutMetrics::default()),
            event_targets: RefCell::new(HashMap::new()),
            resource_loader: None,
            host_io_active: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(super) fn new_interactive(
        document: HtmlDocument,
        resource_loader: HtmlSubresourceLoader,
    ) -> Self {
        Self {
            document: RefCell::new(document),
            error: RefCell::new(None),
            layout_metrics: RefCell::new(HtmlLayoutMetrics::default()),
            event_targets: RefCell::new(HashMap::new()),
            resource_loader: Some(resource_loader),
            host_io_active: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(super) fn host_io_active(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.host_io_active)
    }
}

pub(super) fn argument(arguments: &[String], index: usize) -> Result<&str, String> {
    arguments
        .get(index)
        .map(String::as_str)
        .ok_or_else(|| format!("missing DOM argument {index}"))
}

pub(super) fn node_id(value: &str) -> Result<u64, String> {
    value
        .parse()
        .map_err(|_| format!("invalid HTML node id: {value}"))
}

#[cfg(test)]
mod tests {
    use super::HtmlDomBridgeState;
    use crate::renderer::backends::html_document::HtmlDocument;

    fn state() -> HtmlDomBridgeState {
        HtmlDomBridgeState::new(HtmlDocument::parse("<p id=target>Text</p>"))
    }

    #[test]
    fn unsupported_operation_errors_are_explicit() {
        let state = state();
        assert!(state.dispatch("unsupported", &[]).is_err());
        assert!(state.lookup("unsupported", &[]).is_err());
        assert!(state.mutate_tree("unsupported", &[]).is_err());
        assert!(
            state
                .mutate_content("unsupported", &["1".to_string(), "x".to_string()])
                .is_err()
        );
        assert!(state.style("unsupported", &[]).is_err());
        assert!(
            state
                .set_attribute("unsupported", &["1".to_string(), "state".to_string()])
                .is_err()
        );
    }

    #[test]
    fn unsupported_lookup_node_operation_reports_contract_error() {
        let state = state();
        assert!(matches!(
            state.lookup_node("not_supported", &[]),
            Err(error) if error == "unsupported HTML node lookup operation: not_supported"
        ));
    }
}
