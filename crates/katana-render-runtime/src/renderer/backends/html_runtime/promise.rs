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
