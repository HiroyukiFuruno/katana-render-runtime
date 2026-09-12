use super::script::{HtmlTryCatchScope, evaluate_value};
use super::types::HtmlRuntimeError;

pub(super) fn event<'scope>(
    scope: &mut HtmlTryCatchScope<'scope, '_, '_, '_>,
    node_id: u64,
    event_type: &str,
    event_key: Option<&str>,
) -> Result<v8::Local<'scope, v8::Object>, HtmlRuntimeError> {
    let key = event_key
        .map(|key| serde_json::to_string(key).map_err(event_serialization_error))
        .transpose()?
        .unwrap_or_else(|| "null".to_string());
    let (bubbles, cancelable) = event_flags(event_type);
    let source = format!(
        "__krrDispatchHostEvent('{node_id}', {}, {key}, {bubbles}, {cancelable})",
        serde_json::to_string(event_type).map_err(event_serialization_error)?
    );
    let value = evaluate_value(scope, "krr-html-dom-event", &source)?;
    let event = v8::Local::<v8::Object>::try_from(value).map_err(event_error)?;
    Ok(event)
}

pub(super) fn dispatch_window_event(
    scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
    event_type: &str,
) -> Result<(), HtmlRuntimeError> {
    let event_type = serde_json::to_string(event_type).map_err(event_serialization_error)?;
    let source = format!(
        "window.dispatchEvent(new Event({event_type}, {{ bubbles: false, cancelable: false }}))"
    );
    evaluate_value(scope, "krr-html-window-event", &source)?;
    Ok(())
}

fn event_flags(event_type: &str) -> (bool, bool) {
    match event_type {
        "focus" | "blur" | "toggle" => (false, false),
        "load" | "DOMContentLoaded" | "readystatechange" => (false, false),
        "click" | "keydown" | "keyup" => (true, true),
        _ => (true, false),
    }
}

fn event_serialization_error(error: serde_json::Error) -> HtmlRuntimeError {
    HtmlRuntimeError::DomBridge(format!("HTML event serialization failed: {error}"))
}

pub(super) fn event_default_prevented(
    scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
    event: v8::Local<'_, v8::Object>,
) -> Result<bool, HtmlRuntimeError> {
    v8::String::new(scope, "defaultPrevented")
        .and_then(|key| event.get(scope, key.into()))
        .map(|value| value.is_true())
        .ok_or(HtmlRuntimeError::JavaScriptException(
            "click event defaultPrevented lookup failed".to_string(),
        ))
}

fn event_error<T>(_error: T) -> HtmlRuntimeError {
    HtmlRuntimeError::DomBridge("HTML event allocation failed".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::diagram_js_runtime::DiagramV8Runtime;
    use crate::renderer::backends::html_runtime::script::evaluate_value;

    #[test]
    fn interaction_error_helpers_preserve_contract_messages() {
        assert!(matches!(
            event_error(()),
            HtmlRuntimeError::DomBridge(message) if message == "HTML event allocation failed"
        ));
    }

    #[test]
    fn interaction_v8_data_error_helpers_preserve_contract_messages() {
        let error = v8::DataError::BadType {
            actual: "actual",
            expected: "expected",
        };

        assert!(matches!(
            event_error(error),
            HtmlRuntimeError::DomBridge(message) if message == "HTML event allocation failed"
        ));
    }

    #[test]
    fn interaction_serialization_error_preserves_contract_message() {
        let error = event_serialization_error(serde_json::Error::io(std::io::Error::other(
            "serialization failed",
        )));
        assert!(matches!(
            error,
            HtmlRuntimeError::DomBridge(message) if message == "HTML event serialization failed: serialization failed"
        ));
    }

    #[test]
    fn event_flags_follow_browser_bubbling_and_cancelability() {
        assert_eq!(event_flags("focus"), (false, false));
        assert_eq!(event_flags("click"), (true, true));
        assert_eq!(event_flags("input"), (true, false));
    }

    #[test]
    fn event_default_prevented_returns_js_exception_when_lookup_throws() {
        DiagramV8Runtime::ensure_initialized();

        let mut isolate = v8::Isolate::new(Default::default());
        v8::scope!(let handle_scope, &mut isolate);
        let context = v8::Context::new(handle_scope, Default::default());
        let context_scope = &mut v8::ContextScope::new(handle_scope, context);
        v8::tc_scope!(let scope, &mut **context_scope);

        let event = evaluate_value(
            scope,
            "krr-html-event-default-prevented",
            "const event = {};\nObject.defineProperty(event, 'defaultPrevented', {\n  get() { throw new Error('lookup failed'); }\n});\n\nevent;",
        )
        .ok()
        .and_then(|value| v8::Local::<v8::Object>::try_from(value).ok());
        assert!(event.is_some_and(|event| matches!(
            event_default_prevented(scope, event),
            Err(HtmlRuntimeError::JavaScriptException(message))
                if message == "click event defaultPrevented lookup failed"
        )));
    }
}
