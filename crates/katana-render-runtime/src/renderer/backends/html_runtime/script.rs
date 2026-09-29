use super::bridge::dom_callback;
use super::dom_state::HtmlDomBridgeState;
use super::execution::ExecutionBudget;
use super::types::HtmlRuntimeError;

pub(super) const DOM_BOOTSTRAP: &str = include_str!("dom_bootstrap.js");

pub(super) const BODY_ONLOAD_INSTALL: &str = "__krrInstallStaticBodyLoadHandler();";
pub(super) const DOM_CONTENT_LOADED_DISPATCH: &str = "__krrDispatchDocumentContentLoaded();";
pub(super) const WINDOW_LOAD_DISPATCH: &str = "__krrDispatchWindowLoad();";

pub(super) type HtmlTryCatchScope<'pin, 'scope, 'object, 'isolate> =
    v8::PinnedRef<'pin, v8::TryCatch<'scope, 'object, v8::HandleScope<'isolate>>>;

pub(super) fn install_dom_bridge(
    scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
    document_url: &str,
) -> Result<(), HtmlRuntimeError> {
    let context = scope.get_current_context();
    let global = context.global(scope);
    let name_error = HtmlRuntimeError::DomBridge("DOM function name allocation failed".to_string());
    let callback_error = HtmlRuntimeError::DomBridge("DOM callback allocation failed".to_string());
    let registration_error =
        HtmlRuntimeError::DomBridge("DOM callback registration failed".to_string());
    let name = v8::String::new(scope, "__krr_dom").ok_or(name_error)?;
    let callback = v8::Function::new(scope, dom_callback).ok_or(callback_error)?;
    global
        .set(scope, name.into(), callback.into())
        .ok_or(registration_error)?;
    evaluate(scope, "krr-html-dom-bootstrap", DOM_BOOTSTRAP)?;
    install_location(scope, document_url)
}

fn install_location(
    scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
    document_url: &str,
) -> Result<(), HtmlRuntimeError> {
    let location = location_value(document_url)?;
    evaluate(
        scope,
        "krr-html-location",
        &format!("globalThis.location = Object.freeze({location});"),
    )
}

fn location_value(document_url: &str) -> Result<serde_json::Value, HtmlRuntimeError> {
    let parsed = url::Url::parse(document_url)
        .map_err(|error| HtmlRuntimeError::DomBridge(format!("invalid document URL: {error}")))?;
    Ok(serde_json::json!({
        "hash": parsed.fragment().map(|value| format!("#{value}")).unwrap_or_default(),
        "host": parsed.host_str().unwrap_or_default(),
        "hostname": parsed.host_str().unwrap_or_default(),
        "href": parsed.as_str(),
        "origin": parsed.origin().ascii_serialization(),
        "pathname": parsed.path(),
        "protocol": format!("{}:", parsed.scheme()),
        "search": parsed.query().map(|value| format!("?{value}")).unwrap_or_default(),
    }))
}

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

pub(super) fn check_bridge_error(
    scope: &HtmlTryCatchScope<'_, '_, '_, '_>,
) -> Result<(), HtmlRuntimeError> {
    let state = scope
        .get_slot::<HtmlDomBridgeState>()
        .ok_or_else(dom_state_unavailable_error)?;
    match state.error.borrow_mut().take() {
        Some(error) => Err(HtmlRuntimeError::DomBridge(error)),
        None => Ok(()),
    }
}

pub(super) fn dom_state_unavailable_error() -> HtmlRuntimeError {
    HtmlRuntimeError::DomBridge("HTML DOM state is unavailable".to_string())
}

fn source_allocation_error() -> HtmlRuntimeError {
    HtmlRuntimeError::JavaScriptException("source allocation failed".to_string())
}

fn filename_allocation_error() -> HtmlRuntimeError {
    HtmlRuntimeError::JavaScriptException("filename allocation failed".to_string())
}

pub(super) fn exception_message(scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>) -> String {
    let Some(exception) = scope.exception() else {
        return unknown_v8_exception_message();
    };
    let summary = exception.to_rust_string_lossy(scope);
    let stack = scope
        .stack_trace()
        .map(|stack| stack.to_rust_string_lossy(scope))
        .filter(|stack| !stack.trim().is_empty() && stack != &summary);
    stack.unwrap_or_else(|| exception_location(scope, summary))
}

fn exception_location(scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>, summary: String) -> String {
    let context = scope.message().and_then(|message| {
        let line = message.get_line_number(scope)?;
        let column = message.get_start_column() + 1;
        let resource = message
            .get_script_resource_name(scope)
            .map(|value| value.to_rust_string_lossy(scope))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "inline-script".to_string());
        let source = message
            .get_source_line(scope)
            .map(|value| value.to_rust_string_lossy(scope))
            .filter(|value| !value.trim().is_empty())
            .map(|source| format!("\n  {source}"))
            .unwrap_or_else(String::new);
        Some(format!("  at {resource}:{line}:{column}{source}"))
    });
    context
        .map(|context| format!("{summary}\n{context}"))
        .unwrap_or(summary)
}

fn unknown_v8_exception_message() -> String {
    "unknown V8 exception".to_string()
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

    #[test]
    fn dom_state_error_helper_preserves_contract_message() {
        assert!(matches!(
            dom_state_unavailable_error(),
            HtmlRuntimeError::DomBridge(message) if message == "HTML DOM state is unavailable"
        ));
    }

    #[test]
    fn location_value_preserves_document_url_and_rejects_invalid_input() {
        let value = location_value("file:///tmp/index.html?slide=2#deck");
        assert!(matches!(
            value,
            Ok(value)
                if value["protocol"] == "file:"
                    && value["pathname"] == "/tmp/index.html"
                    && value["search"] == "?slide=2"
                    && value["hash"] == "#deck"
        ));
        assert!(matches!(
            location_value("not a URL"),
            Err(HtmlRuntimeError::DomBridge(message))
                if message.contains("invalid document URL")
        ));
    }

    #[test]
    fn exception_message_reports_empty_try_catch() {
        crate::markdown::diagram_js_runtime::DiagramV8Runtime::ensure_initialized();
        let mut isolate = v8::Isolate::new(Default::default());
        v8::scope!(let handle_scope, &mut isolate);
        let context = v8::Context::new(handle_scope, Default::default());
        let context_scope = &mut v8::ContextScope::new(handle_scope, context);
        v8::tc_scope!(let scope, &mut **context_scope);

        assert_eq!(exception_message(scope), "unknown V8 exception");
    }

    #[test]
    fn compile_error_without_resource_name_uses_diagnostic_fallback() {
        crate::markdown::diagram_js_runtime::DiagramV8Runtime::ensure_initialized();
        let mut isolate = v8::Isolate::new(Default::default());
        v8::scope!(let handle_scope, &mut isolate);
        let context = v8::Context::new(handle_scope, Default::default());
        let context_scope = &mut v8::ContextScope::new(handle_scope, context);
        v8::tc_scope!(let scope, &mut **context_scope);

        assert!(matches!(
            evaluate(scope, "", "const = ;"),
            Err(HtmlRuntimeError::JavaScriptCompile(message))
                if message.contains("inline-script:1:") && message.contains("const = ;")
        ));
    }
}
