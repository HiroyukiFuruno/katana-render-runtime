use super::bridge::uncaught_dom_error;
use super::exception::exception_message;
use super::execution::ExecutionBudget;
use super::script::HtmlTryCatchScope;
use super::types::HtmlRuntimeError;
use crate::v8_script::ScriptCompiler;

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

pub(super) fn evaluate_value_unbounded<'scope>(
    scope: &mut HtmlTryCatchScope<'scope, '_, '_, '_>,
    name: &str,
    code: &str,
) -> Result<v8::Local<'scope, v8::Value>, HtmlRuntimeError> {
    let script = compile_unbounded(scope, name, code)?;
    run_unbounded(scope, script)
}

pub(super) fn compile_unbounded<'scope>(
    scope: &mut HtmlTryCatchScope<'scope, '_, '_, '_>,
    name: &str,
    code: &str,
) -> Result<v8::Local<'scope, v8::Script>, HtmlRuntimeError> {
    let source = v8::String::new(scope, code).ok_or_else(source_allocation_error)?;
    let origin_name = v8::String::new(scope, name).ok_or_else(filename_allocation_error)?;
    ScriptCompiler::compile_with_origin(scope, source, origin_name)
        .ok_or_else(|| HtmlRuntimeError::JavaScriptCompile(exception_message(scope)))
}

pub(super) fn run_unbounded<'scope>(
    scope: &mut HtmlTryCatchScope<'scope, '_, '_, '_>,
    script: v8::Local<'scope, v8::Script>,
) -> Result<v8::Local<'scope, v8::Value>, HtmlRuntimeError> {
    script
        .run(scope)
        .ok_or_else(|| script_execution_error(scope))
}

fn script_execution_error(scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>) -> HtmlRuntimeError {
    if scope.has_terminated() {
        return HtmlRuntimeError::ExecutionTimeout;
    }
    /* WHY: native例外のthrowで評価が先に失敗しても、実保存されたDOMエラーの分類を維持する。 */
    match uncaught_dom_error(scope) {
        Some(error) => HtmlRuntimeError::DomBridge(error),
        None => HtmlRuntimeError::JavaScriptException(exception_message(scope)),
    }
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
