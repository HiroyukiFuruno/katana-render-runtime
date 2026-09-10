use super::super::dom_state::HtmlDomBridgeState;
use super::super::interaction::{dispatch_window_event, event, event_default_prevented};
use super::super::script::{HtmlTryCatchScope, check_bridge_error, dom_state_unavailable_error};
use super::super::types::HtmlNodeId;
use super::super::types::{
    HtmlNavigationIntent, HtmlRuntimeDispatch, HtmlRuntimeError, HtmlRuntimeEvent,
    HtmlRuntimeEventKind,
};
use super::StaticHtmlRuntimeSession;

impl StaticHtmlRuntimeSession {
    pub(crate) fn body_node(&mut self) -> Option<HtmlNodeId> {
        self.isolate
            .as_mut()?
            .get_slot::<HtmlDomBridgeState>()?
            .document
            .borrow_mut()
            .query_selector("body")
            .map(HtmlNodeId)
    }

    #[cfg(test)]
    pub(crate) fn node_for_element_id(&mut self, id: &str) -> Option<HtmlNodeId> {
        self.isolate
            .as_mut()?
            .get_slot::<HtmlDomBridgeState>()?
            .document
            .borrow_mut()
            .get_element_by_id(id)
            .map(HtmlNodeId)
    }

    pub(crate) fn dispatch(
        &mut self,
        event: HtmlRuntimeEvent,
    ) -> Result<HtmlRuntimeDispatch, HtmlRuntimeError> {
        let result = self.run_event(event);
        if matches!(result, Err(HtmlRuntimeError::ExecutionTimeout)) {
            self.discard();
        }
        result
    }

    pub(crate) fn dispatch_window_scroll(
        &mut self,
    ) -> Result<HtmlRuntimeDispatch, HtmlRuntimeError> {
        let result = self.run_window_event(HtmlRuntimeEventKind::Scroll);
        if matches!(result, Err(HtmlRuntimeError::ExecutionTimeout)) {
            self.discard();
        }
        result
    }

    pub(in crate::renderer::backends) fn node_path(
        &self,
        node_id: u64,
    ) -> Result<std::collections::HashSet<u64>, HtmlRuntimeError> {
        self.isolate
            .as_ref()
            .ok_or_else(discarded_runtime_error)?
            .get_slot::<HtmlDomBridgeState>()
            .ok_or_else(dom_state_unavailable_error)?
            .document
            .borrow()
            .event_path(node_id)
            .map(|path| path.into_iter().collect())
            .map_err(HtmlRuntimeError::DomBridge)
    }

    fn run_event(
        &mut self,
        event: HtmlRuntimeEvent,
    ) -> Result<HtmlRuntimeDispatch, HtmlRuntimeError> {
        let kind = event.kind();
        let target = event.target();
        let key = event.key();
        let navigation = {
            let isolate = self.isolate.as_mut().ok_or_else(discarded_runtime_error)?;
            let context = self.context.as_ref().ok_or_else(discarded_runtime_error)?;
            v8::scope!(let handle_scope, isolate);
            let context = v8::Local::new(handle_scope, context);
            let context_scope = &mut v8::ContextScope::new(handle_scope, context);
            v8::tc_scope!(let scope, &mut **context_scope);
            dispatch_event(scope, target.0, kind, key)?
        };
        Ok(HtmlRuntimeDispatch {
            content: self.snapshot()?,
            navigation,
        })
    }

    fn run_window_event(
        &mut self,
        kind: HtmlRuntimeEventKind,
    ) -> Result<HtmlRuntimeDispatch, HtmlRuntimeError> {
        {
            let isolate = self.isolate.as_mut().ok_or_else(discarded_runtime_error)?;
            let context = self.context.as_ref().ok_or_else(discarded_runtime_error)?;
            v8::scope!(let handle_scope, isolate);
            let context = v8::Local::new(handle_scope, context);
            let context_scope = &mut v8::ContextScope::new(handle_scope, context);
            v8::tc_scope!(let scope, &mut **context_scope);
            dispatch_window_event(scope, kind.as_str())?;
        }
        Ok(HtmlRuntimeDispatch {
            content: self.snapshot()?,
            navigation: None,
        })
    }

    fn discard(&mut self) {
        self.context.take();
        self.isolate.take();
    }
}

pub(super) fn discarded_runtime_error() -> HtmlRuntimeError {
    HtmlRuntimeError::DomBridge("HTML runtime session was discarded after timeout".to_string())
}

fn dispatch_event(
    scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
    node_id: u64,
    kind: HtmlRuntimeEventKind,
    key: Option<&str>,
) -> Result<Option<HtmlNavigationIntent>, HtmlRuntimeError> {
    let event = event(scope, node_id, kind.as_str(), key)?;
    check_bridge_error(scope)?;
    if kind == HtmlRuntimeEventKind::Click {
        navigation_intent(scope, node_id, event)
    } else {
        Ok(None)
    }
}

fn navigation_intent(
    scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
    node_id: u64,
    event: v8::Local<v8::Object>,
) -> Result<Option<HtmlNavigationIntent>, HtmlRuntimeError> {
    if event_default_prevented(scope, event)? {
        return Ok(None);
    }
    scope
        .get_slot::<HtmlDomBridgeState>()
        .ok_or_else(dom_state_unavailable_error)?
        .document
        .borrow()
        .attribute(node_id, "href")
        .map_err(HtmlRuntimeError::DomBridge)
        .map(|href| href.map(|href| HtmlNavigationIntent { href }))
}
