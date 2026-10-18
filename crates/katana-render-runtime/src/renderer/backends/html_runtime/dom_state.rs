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
#[path = "dom_state_lookup_nodes.rs"]
mod lookup_nodes;
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
mod test_support {
    use super::HtmlDomBridgeState;

    pub(super) fn with_fixture_element(
        state: &HtmlDomBridgeState,
        id: &str,
        action: impl FnOnce(String),
    ) {
        let node_id = state.document.borrow_mut().get_element_by_id(id);
        assert!(node_id.is_some(), "fixture element #{id} must be indexed");
        if let Some(node_id) = node_id {
            action(node_id.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HtmlDomBridgeState, test_support::with_fixture_element};
    use crate::renderer::backends::html_document::HtmlDocument;
    use crate::renderer::backends::html_runtime::types::DomValue;

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
    fn tree_mutations_update_registered_document_content() {
        let state = HtmlDomBridgeState::new(HtmlDocument::parse("<div id=target>start</div>"));
        with_fixture_element(&state, "target", |target| {
            assert_dom_undefined(
                state.mutate_tree("appendText", &[target.clone(), " appended".to_string()]),
            );
            assert_dom_undefined(state.mutate_tree(
                "insertAdjacentText",
                &[
                    target.clone(),
                    "afterbegin".to_string(),
                    "prefix ".to_string(),
                ],
            ));
            assert!(matches!(
                state.lookup("textContent", &[target]),
                Ok(DomValue::String(value)) if value == "prefix start appended"
            ));
        });
    }

    #[test]
    fn resource_presence_lookup_detects_nested_image_and_frame_elements() {
        let state = HtmlDomBridgeState::new(HtmlDocument::parse(
            "<main><img src='x'><section><iframe></iframe></section></main>",
        ));
        assert!(matches!(
            state.dispatch("resourcePresence", &[]),
            Ok(super::super::types::DomValue::String(value)) if value == "1:1"
        ));
    }

    #[test]
    fn image_data_url_validation_returns_load_only_for_fully_decodable_data_images() {
        let state = state();
        let valid_png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";
        let truncated_webp = "data:image/webp;base64,UklGRggAAABXRUJQ";

        assert!(matches!(
            state.dispatch("validateImageDataUrl", &[valid_png.to_string()]),
            Ok(super::super::types::DomValue::String(value)) if value == "load"
        ));
        assert!(matches!(
            state.dispatch("validateImageDataUrl", &[truncated_webp.to_string()]),
            Ok(super::super::types::DomValue::String(value)) if value == "error"
        ));
        assert!(matches!(
            state.dispatch("validateImageDataUrl", &["https://example.test/image.png".to_string()]),
            Ok(super::super::types::DomValue::String(value)) if value == "error"
        ));
    }

    #[test]
    fn unsupported_lookup_node_operation_reports_contract_error() {
        let state = state();
        assert!(matches!(
            state.lookup_node("not_supported", &[]),
            Err(error) if error == "unsupported HTML node lookup operation: not_supported"
        ));
    }

    #[test]
    fn content_lookup_covers_markup_path_and_missing_attribute_contracts() {
        let state = HtmlDomBridgeState::new(HtmlDocument::parse(
            "<main><section id=scope><p id=target data-kind=card>Text</p></section></main>",
        ));
        with_fixture_element(&state, "target", |target| {
            assert_content_markup(&state, &target)
        });
    }

    fn assert_content_markup(state: &HtmlDomBridgeState, target: &str) {
        assert!(matches!(
            state.lookup("innerHTML", &[target.to_string()]),
            Ok(DomValue::String(value)) if value == "Text"
        ));
        assert!(matches!(
            state.lookup("outerHTML", &[target.to_string()]),
            Ok(DomValue::String(value)) if value.contains("data-kind=\"card\"")
        ));
        assert_missing_content_lookups(state, target);
    }

    fn assert_missing_content_lookups(state: &HtmlDomBridgeState, target: &str) {
        assert!(matches!(
            state.lookup("getAttribute", &[target.to_string(), "missing".to_string()],),
            Ok(DomValue::Null)
        ));
        assert!(matches!(
            state.lookup("closest", &[target.to_string(), ".missing".to_string()]),
            Ok(DomValue::Null)
        ));
        assert!(matches!(
            state.lookup("resourcePresence", &[]),
            Ok(DomValue::String(value)) if value == "0:0"
        ));
    }

    #[test]
    fn tree_and_attribute_mutations_cover_adjacent_remove_and_style_reset() {
        let state = HtmlDomBridgeState::new(HtmlDocument::parse(
            "<main><div id=target><span id=child>body</span></div></main>",
        ));
        with_fixture_element(&state, "target", |target| {
            with_fixture_element(&state, "child", |child| {
                apply_mutations(&state, &target, &child);
            });
        });
    }

    fn apply_mutations(state: &HtmlDomBridgeState, target: &str, child: &str) {
        assert_dom_undefined(state.mutate_tree(
            "insertAdjacentText",
            &[target.to_string(), "afterbegin".into(), "pre".into()],
        ));
        assert_dom_undefined(state.mutate_tree(
            "insertAdjacentHTML",
            &[target.to_string(), "beforeend".into(), "<b>tail</b>".into()],
        ));
        assert_dom_undefined(state.mutate_tree("appendText", &[target.to_string(), "!".into()]));
        assert_dom_undefined(state.set_attribute(
            "setAttribute",
            &[target.to_string(), "data-state".into(), "ready".into()],
        ));
        assert!(matches!(
            state.style("styleGet", &[target.to_string(), "color".into()]),
            Ok(DomValue::Null)
        ));
        finish_mutations(state, target, child);
    }

    fn finish_mutations(state: &HtmlDomBridgeState, target: &str, child: &str) {
        assert_dom_undefined(state.mutate_tree("remove", &[child.to_string()]));
        assert_dom_undefined(state.set_attribute(
            "removeAttribute",
            &[target.to_string(), "data-state".into()],
        ));
        assert!(
            matches!(state.lookup("textContent", &[target.to_string()]), Ok(DomValue::String(value)) if value.contains("pre") && value.contains("tail"))
        );
    }

    fn assert_dom_undefined(result: Result<DomValue, String>) {
        assert!(matches!(result, Ok(DomValue::Undefined)));
    }
}
