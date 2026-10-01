use super::exception::exception_message;
use super::execution::ExecutionBudget;
use super::script::HtmlTryCatchScope;
use super::types::HtmlRuntimeError;

pub(super) fn evaluate(
    scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
    name: &str,
    code: &str,
) -> Result<(), HtmlRuntimeError> {
    evaluate_value(scope, name, code).map(|_| ())
}

pub(super) fn evaluate_value<'scope>(
    scope: &mut HtmlTryCatchScope<'scope, '_, '_, '_>,
    name: &str,
    code: &str,
) -> Result<v8::Local<'scope, v8::Value>, HtmlRuntimeError> {
    let budget = ExecutionBudget::start(scope);
    let result = evaluate_value_unbounded(scope, name, code);
    budget.finish()?;
    result
}

fn evaluate_value_unbounded<'scope>(
    scope: &mut HtmlTryCatchScope<'scope, '_, '_, '_>,
    name: &str,
    code: &str,
) -> Result<v8::Local<'scope, v8::Value>, HtmlRuntimeError> {
    let source = v8::String::new(scope, code).ok_or_else(source_allocation_error)?;
    let origin_name = v8::String::new(scope, name).ok_or_else(filename_allocation_error)?;
    let origin = v8::ScriptOrigin::new(
        scope,
        origin_name.into(),
        0,
        0,
        false,
        0,
        Some(origin_name.into()),
        false,
        false,
        false,
        None,
    );
    let script = v8::Script::compile(scope, source, Some(&origin))
        .ok_or_else(|| HtmlRuntimeError::JavaScriptCompile(exception_message(scope)))?;
    script
        .run(scope)
        .ok_or_else(|| HtmlRuntimeError::JavaScriptException(exception_message(scope)))
}

pub(super) fn perform_microtask_checkpoint(
    scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
) -> Result<(), HtmlRuntimeError> {
    let budget = ExecutionBudget::start(scope);
    scope.as_mut().perform_microtask_checkpoint();
    budget.finish()
}

fn source_allocation_error() -> HtmlRuntimeError {
    HtmlRuntimeError::JavaScriptException("source allocation failed".to_string())
}

fn filename_allocation_error() -> HtmlRuntimeError {
    HtmlRuntimeError::JavaScriptException("filename allocation failed".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocation_error_helpers_preserve_contract_messages() {
        assert!(matches!(
            source_allocation_error(),
            HtmlRuntimeError::JavaScriptException(message)
                if message == "source allocation failed"
        ));
        assert!(matches!(
            filename_allocation_error(),
            HtmlRuntimeError::JavaScriptException(message)
                if message == "filename allocation failed"
        ));
    }
}
