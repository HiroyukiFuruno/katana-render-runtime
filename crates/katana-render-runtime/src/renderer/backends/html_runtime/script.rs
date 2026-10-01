use super::bridge::dom_callback;
use super::dom_state::HtmlDomBridgeState;
use super::types::HtmlRuntimeError;

pub(super) use super::evaluation::{evaluate, evaluate_value, perform_microtask_checkpoint};
pub(super) use super::promise::evaluate_and_wait_for_promise;

pub(super) const DOM_BOOTSTRAP: &str = include_str!("dom_bootstrap.js");

pub(super) const BODY_ONLOAD_INSTALL: &str = "__krrInstallStaticBodyLoadHandler();";
pub(super) const DOM_CONTENT_LOADED_DISPATCH: &str = "__krrDispatchDocumentContentLoaded();";
pub(super) const WINDOW_LOAD_DISPATCH: &str = "__krrDispatchWindowLoad();";

pub(super) fn body_onload_install_script(source: Option<&str>) -> String {
    let Some(source) = source else {
        return BODY_ONLOAD_INSTALL.to_string();
    };
    let source = serde_json::Value::String(source.to_owned()).to_string();
    format!("__krrInstallStaticBodyLoadHandler({source});")
}
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

#[cfg(test)]
mod tests {
    use super::*;

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
