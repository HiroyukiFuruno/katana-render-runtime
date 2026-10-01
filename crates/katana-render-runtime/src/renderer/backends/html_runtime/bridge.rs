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
    /* WHY: 引数の文字列化はページのJavaScriptを呼べるため通常の実行予算で計測する。 */
    let operation = args.get(0).to_rust_string_lossy(scope);
    let arguments = (1..args.length())
        .map(|index| args.get(index).to_rust_string_lossy(scope))
        .collect::<Vec<_>>();
    /* WHY: 同期DOM処理だけを別枠で計測し、低速CPUで画像検証を誤検知せず、
     * ページの引数変換にはホスト処理用の実行猶予を与えない。 */
    let host_operation = scope
        .get_slot::<HtmlDomBridgeState>()
        .map(HtmlDomBridgeState::host_io_active)
        .map(HostOperationGuard::enter);
    let result = scope
        .get_slot::<HtmlDomBridgeState>()
        .ok_or(MISSING_DOM_STATE_ERROR.to_string())
        .and_then(|state| state.dispatch(&operation, &arguments));
    drop(host_operation);
    match result {
        Ok(value) => return_value.set(dom_value(scope, value)),
        Err(error) => set_bridge_error(scope, &mut return_value, error),
    }
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
        assert!(installed.is_ok(), "{:?}", installed.err());
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
        assert!(installed.is_ok(), "{:?}", installed.err());
        assert!(
            evaluate_value(
                scope,
                "krr-html-dom-missing-state",
                "__krr_dom('unsupported');"
            )
            .is_ok_and(|value| value.is_undefined())
        );
    }

    #[test]
    fn callback_operation_and_argument_coercion_stay_in_javascript_budget() -> Result<(), String> {
        for expression in [
            "__krr_dom({toString(){active = __krr_host_active(); return 'unsupported';}})",
            "__krr_dom('unsupported', {toString(){active = __krr_host_active(); return 'arg';}})",
        ] {
            assert_callback_coercion(expression, false)?;
        }
        Ok(())
    }

    #[test]
    fn callback_coercion_cannot_use_host_timeout_allowance() -> Result<(), String> {
        for expression in [
            "__krr_dom({toString(){const end = Date.now() + 750; while(Date.now() < end) {} return 'unsupported';}})",
            "__krr_dom('unsupported', {toString(){const end = Date.now() + 750; while(Date.now() < end) {} return 'arg';}})",
        ] {
            assert_callback_coercion(expression, true)?;
        }
        Ok(())
    }

    fn assert_callback_coercion(expression: &str, expect_timeout: bool) -> Result<(), String> {
        DiagramV8Runtime::ensure_initialized();
        let mut isolate = v8::Isolate::new(Default::default());
        isolate.set_slot(HtmlDomBridgeState::new(HtmlDocument::parse("<p>Test</p>")));
        v8::scope!(let handle_scope, &mut isolate);
        let context = v8::Context::new(handle_scope, Default::default());
        let context_scope = &mut v8::ContextScope::new(handle_scope, context);
        v8::tc_scope!(let scope, &mut **context_scope);
        assert!(install_dom_bridge(scope, "about:blank").is_ok());
        install_host_activity_probe(scope)?;
        let source = format!("let active = true; {expression}; active");
        let result = evaluate_value(scope, "bridge-coercion-budget", &source);
        let timed_out = matches!(
            result,
            Err(super::super::types::HtmlRuntimeError::ExecutionTimeout)
        );
        assert_eq!(timed_out, expect_timeout, "{result:?}");
        if !expect_timeout {
            assert!(result.is_ok_and(|value| value.is_false()));
        }
        assert!(
            scope
                .get_slot::<HtmlDomBridgeState>()
                .is_some_and(|state| !state.host_io_active().load(Ordering::SeqCst))
        );
        Ok(())
    }

    fn install_host_activity_probe(scope: &mut v8::PinScope) -> Result<(), String> {
        let name = v8::String::new(scope, "__krr_host_active").ok_or("name allocation failed")?;
        let callback =
            v8::Function::new(scope, host_active_for_test).ok_or("function allocation failed")?;
        let context = scope.get_current_context();
        let global = context.global(scope);
        assert_eq!(global.set(scope, name.into(), callback.into()), Some(true));
        Ok(())
    }

    fn host_active_for_test(
        scope: &mut v8::PinScope,
        _args: v8::FunctionCallbackArguments,
        mut return_value: v8::ReturnValue<v8::Value>,
    ) {
        let active = scope
            .get_slot::<HtmlDomBridgeState>()
            .is_some_and(|state| state.host_io_active().load(Ordering::SeqCst));
        return_value.set(v8::Boolean::new(scope, active).into());
    }
}
