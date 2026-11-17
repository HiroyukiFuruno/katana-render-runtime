use super::super::dom_state::HtmlDomBridgeState;
use super::super::interaction::{dispatch_window_event, event, event_default_prevented};
use super::super::script::{HtmlTryCatchScope, check_bridge_error, dom_state_unavailable_error};
use super::super::types::HtmlNodeId;
use super::super::types::{
    HtmlNavigationIntent, HtmlRuntimeDispatch, HtmlRuntimeError, HtmlRuntimeEvent,
    HtmlRuntimeEventKind,
};
use super::StaticHtmlRuntimeSession;

#[path = "session_interaction_focus.rs"]
mod focus;
#[path = "session_interaction_navigation.rs"]
mod navigation;

pub(super) use focus::set_focused_element;
use navigation::{navigation_intent, scriptless_event_navigation};

impl StaticHtmlRuntimeSession {
    pub(crate) fn set_active_element(
        &mut self,
        node_id: Option<u64>,
    ) -> Result<(), HtmlRuntimeError> {
        if self.scriptless_document.is_some() {
            #[cfg(test)]
            if let Some(observer) = &mut self.scriptless_observer {
                observer.focused_node = node_id;
            }
            return Ok(());
        }
        let result = self.run_active_element_update(node_id);
        self.discard_on_timeout(result)
    }

    fn run_active_element_update(&mut self, node_id: Option<u64>) -> Result<(), HtmlRuntimeError> {
        let isolate = self.isolate.as_mut().ok_or_else(discarded_runtime_error)?;
        let context = self.context.as_ref().ok_or_else(discarded_runtime_error)?;
        v8::scope!(let handle_scope, isolate);
        let context = v8::Local::new(handle_scope, context);
        let context_scope = &mut v8::ContextScope::new(handle_scope, context);
        v8::tc_scope!(let scope, &mut **context_scope);
        set_focused_element(scope, node_id)
    }

    pub(crate) fn body_node(&mut self) -> Option<HtmlNodeId> {
        self.dom_document()
            .ok()?
            .borrow_mut()
            .query_selector("body")
            .map(HtmlNodeId)
    }

    #[cfg(test)]
    pub(crate) fn node_for_element_id(&mut self, id: &str) -> Option<HtmlNodeId> {
        self.dom_document()
            .ok()?
            .borrow_mut()
            .get_element_by_id(id)
            .map(HtmlNodeId)
    }

    pub(crate) fn dispatch(
        &mut self,
        event: HtmlRuntimeEvent,
    ) -> Result<HtmlRuntimeDispatch, HtmlRuntimeError> {
        self.dispatch_with_default_prevented(event)
            .map(|(dispatch, _)| dispatch)
    }

    pub(crate) fn dispatch_with_default_prevented(
        &mut self,
        event: HtmlRuntimeEvent,
    ) -> Result<(HtmlRuntimeDispatch, bool), HtmlRuntimeError> {
        let result = self.run_event(event);
        self.discard_on_timeout(result)
    }

    pub(crate) fn dispatch_window_scroll(
        &mut self,
    ) -> Result<HtmlRuntimeDispatch, HtmlRuntimeError> {
        if self.scriptless_document.is_some() {
            return self.current_dispatch(None);
        }
        let result = self.run_window_event(HtmlRuntimeEventKind::Scroll);
        self.discard_on_timeout(result)
    }

    pub(in crate::renderer::backends) fn node_path(
        &self,
        node_id: u64,
    ) -> Result<std::collections::HashSet<u64>, HtmlRuntimeError> {
        self.dom_document()?
            .borrow()
            .event_path(node_id)
            .map(|path| path.into_iter().collect())
            .map_err(HtmlRuntimeError::DomBridge)
    }

    fn run_event(
        &mut self,
        event: HtmlRuntimeEvent,
    ) -> Result<(HtmlRuntimeDispatch, bool), HtmlRuntimeError> {
        let kind = event.kind();
        let target = event.target();
        if let Some(document) = &self.scriptless_document {
            let navigation = scriptless_event_navigation(&document.borrow(), target, kind)?;
            return Ok((self.current_dispatch(navigation)?, false));
        }
        let key = event.key();
        let (navigation, default_prevented) = {
            let isolate = self.isolate.as_mut().ok_or_else(discarded_runtime_error)?;
            let context = self.context.as_ref().ok_or_else(discarded_runtime_error)?;
            v8::scope!(let handle_scope, isolate);
            let context = v8::Local::new(handle_scope, context);
            let context_scope = &mut v8::ContextScope::new(handle_scope, context);
            v8::tc_scope!(let scope, &mut **context_scope);
            dispatch_event(scope, target.0, kind, key)?
        };
        Ok((self.current_dispatch(navigation)?, default_prevented))
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
        self.current_dispatch(None)
    }

    fn current_dispatch(
        &self,
        navigation: Option<HtmlNavigationIntent>,
    ) -> Result<HtmlRuntimeDispatch, HtmlRuntimeError> {
        Ok(HtmlRuntimeDispatch {
            content: self.snapshot()?,
            navigation,
        })
    }

    pub(super) fn discard_on_timeout<T>(
        &mut self,
        result: Result<T, HtmlRuntimeError>,
    ) -> Result<T, HtmlRuntimeError> {
        if matches!(result, Err(HtmlRuntimeError::ExecutionTimeout)) {
            /* WHY: isolateを破棄する前に、そこに属するGlobal参照を解放する。 */
            self.context.take();
            self.isolate.take();
        }
        result
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
) -> Result<(Option<HtmlNavigationIntent>, bool), HtmlRuntimeError> {
    let event = event(scope, node_id, kind.as_str(), key)?;
    check_bridge_error(scope)?;
    let default_prevented = event_default_prevented(scope, event)?;
    let navigation = if kind == HtmlRuntimeEventKind::Click && !default_prevented {
        let state = scope
            .get_slot::<HtmlDomBridgeState>()
            .ok_or_else(dom_state_unavailable_error)?;
        navigation_intent(&state.document.borrow(), node_id)?
    } else {
        None
    };
    Ok((navigation, default_prevented))
}

use crate::private_v8 as v8;
