use super::evaluation::{evaluate_value, perform_microtask_checkpoint};
use super::execution::ExecutionBudget;
use super::script::HtmlTryCatchScope;
use super::types::HtmlRuntimeError;

const MAX_PROMISE_CHECKPOINTS: usize = 64;

pub(super) fn evaluate_and_wait_for_promise(
    scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
    name: &str,
    code: &str,
) -> Result<(), HtmlRuntimeError> {
    let value = evaluate_value(scope, name, code)?;
    let Ok(promise) = v8::Local::<v8::Promise>::try_from(value) else {
        return Ok(());
    };
    for _ in 0..MAX_PROMISE_CHECKPOINTS {
        if promise.state() != v8::PromiseState::Pending {
            break;
        }
        perform_microtask_checkpoint(scope)?;
    }
    match promise.state() {
        v8::PromiseState::Fulfilled => Ok(()),
        v8::PromiseState::Rejected => {
            /* WHY: rejection value conversion can invoke page-defined hooks, so it
             * must remain within the execution watchdog. */
            let budget = ExecutionBudget::start(scope);
            let message = promise.result(scope).to_rust_string_lossy(scope);
            budget.finish()?;
            Err(HtmlRuntimeError::JavaScriptException(message))
        }
        v8::PromiseState::Pending => Err(HtmlRuntimeError::JavaScriptException(
            "HTML lifecycle Promise did not settle".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::diagram_js_runtime::DiagramV8Runtime;

    fn evaluate(code: &str) -> Result<(), HtmlRuntimeError> {
        DiagramV8Runtime::ensure_initialized();
        let mut isolate = v8::Isolate::new(Default::default());
        v8::scope!(let handle_scope, &mut isolate);
        let context = v8::Context::new(handle_scope, Default::default());
        let context_scope = &mut v8::ContextScope::new(handle_scope, context);
        v8::tc_scope!(let scope, &mut **context_scope);

        evaluate_and_wait_for_promise(scope, "promise-test", code)
    }

    #[test]
    fn covers_non_promise_settled_and_pending_lifecycle_values() {
        assert!(evaluate("42").is_ok());
        assert!(evaluate("Promise.resolve('fulfilled')").is_ok());
        assert!(matches!(
            evaluate("Promise.reject({ toString() { return 'rejected'; } })"),
            Err(HtmlRuntimeError::JavaScriptException(message)) if message == "rejected"
        ));
        assert!(matches!(
            evaluate("new Promise(() => {})"),
            Err(HtmlRuntimeError::JavaScriptException(message))
                if message == "HTML lifecycle Promise did not settle"
        ));
    }
}
