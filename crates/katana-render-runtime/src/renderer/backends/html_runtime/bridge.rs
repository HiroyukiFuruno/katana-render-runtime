use super::dom_state::HtmlDomBridgeState;
use super::types::DomValue;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const MISSING_DOM_STATE_ERROR: &str = "HTML DOM state is unavailable";

struct HostOperationGuard {
    active: Arc<AtomicBool>,
    was_active: bool,
}

impl HostOperationGuard {
    fn enter(active: Arc<AtomicBool>) -> Self {
        let was_active = active.swap(true, Ordering::SeqCst);
        Self { active, was_active }
    }
}

impl Drop for HostOperationGuard {
    fn drop(&mut self) {
        self.active.store(self.was_active, Ordering::SeqCst);
    }
}

pub(super) fn dom_callback(
    scope: &mut v8::PinScope,
    args: v8::FunctionCallbackArguments,
    mut return_value: v8::ReturnValue<v8::Value>,
) {
    /* WHY: 同期DOM処理はExecutionBudgetの別枠で計測する。低速CPUで画像検証を
     * 誤検知しない余裕を持たせつつ、別枠の上限でページからの反復も止める。 */
    let host_operation = scope
        .get_slot::<HtmlDomBridgeState>()
        .map(HtmlDomBridgeState::host_io_active)
        .map(HostOperationGuard::enter);
    let operation = args.get(0).to_rust_string_lossy(scope);
    let arguments = (1..args.length())
        .map(|index| args.get(index).to_rust_string_lossy(scope))
        .collect::<Vec<_>>();
    let result = scope
        .get_slot::<HtmlDomBridgeState>()
        .ok_or(MISSING_DOM_STATE_ERROR.to_string())
        .and_then(|state| state.dispatch(&operation, &arguments));
    match result {
        Ok(value) => return_value.set(dom_value(scope, value)),
        Err(error) => set_bridge_error(scope, &mut return_value, error),
    }
    drop(host_operation);
}

fn set_bridge_error(
    scope: &mut v8::PinScope,
    return_value: &mut v8::ReturnValue<v8::Value>,
    error: String,
) {
    if let Some(state) = scope.get_slot::<HtmlDomBridgeState>() {
        let _previous_error = state.error.replace(Some(error));
    }
    return_value.set(v8::undefined(scope).into());
}

fn dom_value<'scope>(
    scope: &mut v8::PinScope<'scope, '_>,
    value: DomValue,
) -> v8::Local<'scope, v8::Value> {
    match value {
        DomValue::Undefined => v8::undefined(scope).into(),
        DomValue::Null => v8::null(scope).into(),
        DomValue::String(value) => {
            let undefined = v8::undefined(scope).into();
            v8::String::new(scope, &value).map_or(undefined, |value| value.into())
        }
        DomValue::NodeId(value) => {
            let undefined = v8::undefined(scope).into();
            v8::String::new(scope, &value.to_string()).map_or(undefined, |value| value.into())
        }
        DomValue::NodeIds(values) => node_id_array(scope, &values).into(),
    }
}

fn node_id_array<'scope>(
    scope: &mut v8::PinScope<'scope, '_>,
    values: &[u64],
) -> v8::Local<'scope, v8::Array> {
    let length = i32::try_from(values.len()).unwrap_or(i32::MAX);
    let array = v8::Array::new(scope, length);
    for (index, value) in values.iter().enumerate() {
        let _assigned = v8::String::new(scope, &value.to_string())
            .map(|value| array.set_index(scope, index as u32, value.into()));
    }
    array
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::diagram_js_runtime::DiagramV8Runtime;
    use crate::renderer::backends::html_document::HtmlDocument;
    use crate::renderer::backends::html_runtime::script::{evaluate_value, install_dom_bridge};

    #[test]
    fn missing_dom_state_error_preserves_contract_message() {
        assert_eq!(MISSING_DOM_STATE_ERROR, "HTML DOM state is unavailable");
    }

    #[test]
    fn dom_callback_stores_unsupported_operation_error() {
        DiagramV8Runtime::ensure_initialized();

        let state = HtmlDomBridgeState::new(HtmlDocument::parse("<button id=action>Run</button>"));
        let mut isolate = v8::Isolate::new(Default::default());
        isolate.set_slot(state);

        v8::scope!(let handle_scope, &mut isolate);
        let context = v8::Context::new(handle_scope, Default::default());
        let context_scope = &mut v8::ContextScope::new(handle_scope, context);
        v8::tc_scope!(let scope, &mut **context_scope);

        let installed = install_dom_bridge(scope, "about:blank");
        assert!(installed.is_ok(), "{installed:?}");
        let callback = evaluate_value(scope, "krr-html-dom-callback", "__krr_dom('unsupported');");
        assert!(callback.is_ok(), "{callback:?}");

        let error = scope
            .get_slot::<HtmlDomBridgeState>()
            .and_then(|state| state.error.borrow().clone());
        assert_eq!(
            error,
            Some("unsupported DOM operation: unsupported".to_string())
        );
    }

    #[test]
    fn dom_callback_without_state_returns_undefined() {
        DiagramV8Runtime::ensure_initialized();

        let mut isolate = v8::Isolate::new(Default::default());
        v8::scope!(let handle_scope, &mut isolate);
        let context = v8::Context::new(handle_scope, Default::default());
        let context_scope = &mut v8::ContextScope::new(handle_scope, context);
        v8::tc_scope!(let scope, &mut **context_scope);

        let installed = install_dom_bridge(scope, "about:blank");
        assert!(installed.is_ok(), "{installed:?}");
        assert!(
            evaluate_value(
                scope,
                "krr-html-dom-missing-state",
                "__krr_dom('unsupported');"
            )
            .is_ok_and(|value| value.is_undefined())
        );
    }
}
