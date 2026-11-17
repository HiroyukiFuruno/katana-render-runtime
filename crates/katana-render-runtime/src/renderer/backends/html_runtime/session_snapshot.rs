use super::session_interaction::discarded_runtime_error;
use super::{HtmlNodeIds, StaticHtmlRuntimeSession};
use crate::private_v8 as v8;
use crate::renderer::backends::html_document::HtmlDocumentNode;
use crate::renderer::backends::html_runtime::types::HtmlRuntimeError;
use std::collections::HashSet;

impl StaticHtmlRuntimeSession {
    pub(crate) fn update_document_url(&mut self, url: &str) -> Result<(), HtmlRuntimeError> {
        if self.scriptless_document.is_some() {
            super::super::script::parse_document_url(url)?;
            #[cfg(test)]
            if let Some(observer) = &mut self.scriptless_observer {
                url.clone_into(&mut observer.document_url);
            }
            return Ok(());
        }
        let result = self.run_document_url_update(url);
        self.discard_on_timeout(result)
    }

    fn run_document_url_update(&mut self, url: &str) -> Result<(), HtmlRuntimeError> {
        let (Some(isolate), Some(context)) = (&mut self.isolate, &self.context) else {
            return Err(discarded_runtime_error());
        };
        v8::scope!(let handle_scope, isolate);
        let context = v8::Local::new(handle_scope, context);
        let context_scope = &mut v8::ContextScope::new(handle_scope, context);
        v8::tc_scope!(let scope, &mut **context_scope);
        super::super::script::update_location(scope, url)
    }

    pub(crate) fn snapshot(&self) -> Result<String, HtmlRuntimeError> {
        Ok(self.dom_document()?.borrow().render())
    }

    #[cfg(test)]
    pub(in crate::renderer::backends) fn interactive_nodes(
        &self,
    ) -> Result<Vec<HtmlDocumentNode>, HtmlRuntimeError> {
        self.interactive_nodes_at_width(1024.0)
    }

    #[cfg(test)]
    pub(in crate::renderer::backends) fn interactive_nodes_at_width(
        &self,
        viewport_width: f32,
    ) -> Result<Vec<HtmlDocumentNode>, HtmlRuntimeError> {
        let no_nodes = HashSet::new();
        self.interactive_nodes_at_width_with_focus(viewport_width, &no_nodes, &no_nodes, &no_nodes)
    }

    pub(in crate::renderer::backends) fn interactive_nodes_at_width_with_focus(
        &self,
        viewport_width: f32,
        hovered_nodes: &HtmlNodeIds,
        focused_nodes: &HtmlNodeIds,
        focus_visible_nodes: &HtmlNodeIds,
    ) -> Result<Vec<HtmlDocumentNode>, HtmlRuntimeError> {
        Ok(self
            .dom_document()?
            .borrow()
            .interactive_nodes_with_styles_at_width_and_focus(
                &self.external_stylesheets,
                viewport_width,
                hovered_nodes,
                focused_nodes,
                focus_visible_nodes,
            ))
    }

    pub(in crate::renderer::backends) fn event_target_ids(
        &self,
        event_type: &str,
    ) -> Result<HashSet<u64>, HtmlRuntimeError> {
        if self.scriptless_document.is_some() {
            return Ok(HashSet::new());
        }
        Ok(self.dom_bridge_state()?.event_target_ids(event_type))
    }

    pub(crate) fn set_value(&mut self, node_id: u64, value: &str) -> Result<(), HtmlRuntimeError> {
        self.dom_document()?
            .borrow_mut()
            .set_text_control_value(node_id, value)
            .map_err(HtmlRuntimeError::DomBridge)
    }

    pub(in crate::renderer::backends) fn textarea_value(
        &self,
        node_id: u64,
    ) -> Result<String, HtmlRuntimeError> {
        self.dom_document()?
            .borrow()
            .textarea_value(node_id)
            .map_err(HtmlRuntimeError::DomBridge)
    }

    pub(crate) fn toggle_open(&mut self, node_id: u64) -> Result<bool, HtmlRuntimeError> {
        self.dom_document()?
            .borrow_mut()
            .toggle_boolean_attribute(node_id, "open")
            .map_err(HtmlRuntimeError::DomBridge)
    }

    pub(crate) fn toggle_checked(&mut self, node_id: u64) -> Result<bool, HtmlRuntimeError> {
        self.dom_document()?
            .borrow_mut()
            .toggle_boolean_attribute(node_id, "checked")
            .map_err(HtmlRuntimeError::DomBridge)
    }
}

#[cfg(test)]
mod tests {
    use super::StaticHtmlRuntimeSession;
    use crate::markdown::diagram_js_runtime::DiagramV8Runtime;
    use crate::private_v8 as v8;
    use crate::renderer::backends::html_runtime::types::HtmlRuntimeError;
    use std::collections::HashMap;

    #[test]
    fn snapshot_reports_unavailable_dom_state_when_bridge_slot_is_missing() {
        DiagramV8Runtime::ensure_initialized();
        let session = StaticHtmlRuntimeSession {
            context: None,
            isolate: Some(v8::Isolate::new(Default::default())),
            scriptless_document: None,
            scriptless_observer: None,
            external_stylesheets: HashMap::new(),
        };

        assert!(matches!(
            session.snapshot(),
            Err(HtmlRuntimeError::DomBridge(message))
                if message == "HTML DOM state is unavailable"
        ));
    }
}
