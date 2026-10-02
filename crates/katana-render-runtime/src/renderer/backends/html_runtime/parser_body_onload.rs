use super::{HtmlTryCatchScope, evaluate_value};
use crate::renderer::backends::html_runtime::exception::exception_message;
use crate::renderer::backends::html_runtime::execution::ExecutionBudget;
use crate::renderer::backends::html_runtime::types::HtmlRuntimeError;

pub(in crate::renderer::backends::html_runtime) struct ParserBodyOnloadInstaller {
    callback: v8::Global<v8::Function>,
}

impl ParserBodyOnloadInstaller {
    pub(in crate::renderer::backends::html_runtime) fn capture(
        scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
    ) -> Result<Self, HtmlRuntimeError> {
        let callback = evaluate_value(
            scope,
            "krr-parser-body-onload",
            include_str!("parser_body_onload.js"),
        )?;
        Self::from_callback_value(scope, callback)
    }

    fn from_callback_value(
        scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
        callback: v8::Local<'_, v8::Value>,
    ) -> Result<Self, HtmlRuntimeError> {
        let callback = v8::Local::<v8::Function>::try_from(callback).map_err(|error| {
            HtmlRuntimeError::DomBridge(format!(
                "parser body-onload installer unavailable: {error}"
            ))
        })?;
        Ok(Self {
            callback: v8::Global::new(scope, callback),
        })
    }

    pub(in crate::renderer::backends::html_runtime) fn install_initial(
        &self,
        scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
        payload: &str,
    ) -> Result<(), HtmlRuntimeError> {
        let source = serde_json::from_str::<Option<String>>(payload).map_err(|error| {
            HtmlRuntimeError::DomBridge(format!("invalid parser body-onload source: {error}"))
        })?;
        self.install(scope, source.as_deref(), false)
    }

    pub(in crate::renderer::backends::html_runtime) fn install(
        &self,
        scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
        source: Option<&str>,
        later: bool,
    ) -> Result<(), HtmlRuntimeError> {
        let source: v8::Local<v8::Value> = match source {
            Some(source) => v8::String::new(scope, source)
                .ok_or_else(source_allocation_error)?
                .into(),
            None => v8::undefined(scope).into(),
        };
        let callback = v8::Local::new(scope, &self.callback);
        let receiver = v8::undefined(scope).into();
        let later = v8::Boolean::new(scope, later).into();
        let budget = ExecutionBudget::start(scope);
        let result = callback
            .call(scope, receiver, &[source, later])
            .ok_or_else(|| HtmlRuntimeError::JavaScriptException(exception_message(scope)));
        budget.finish()?;
        result.map(|_| ())
    }
}

fn source_allocation_error() -> HtmlRuntimeError {
    HtmlRuntimeError::JavaScriptException("parser source allocation failed".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::diagram_js_runtime::DiagramV8Runtime;
    use crate::renderer::backends::html_document::HtmlDocument;
    use crate::renderer::backends::html_runtime::dom_state::HtmlDomBridgeState;
    use crate::renderer::backends::html_runtime::script::{evaluate, install_dom_bridge};

    #[test]
    fn parser_body_onload_capture_requires_initialized_dom_bridge() {
        DiagramV8Runtime::ensure_initialized();
        let mut isolate = v8::Isolate::new(Default::default());
        v8::scope!(let handle_scope, &mut isolate);
        let context = v8::Context::new(handle_scope, Default::default());
        let context_scope = &mut v8::ContextScope::new(handle_scope, context);
        v8::tc_scope!(let scope, &mut **context_scope);
        assert!(matches!(
            ParserBodyOnloadInstaller::capture(scope),
            Err(HtmlRuntimeError::JavaScriptException(message))
                if message.contains("ReferenceError") && message.contains("__krrNativeDom")
        ));
    }

    #[test]
    fn parser_body_onload_callback_boundary_rejects_non_function_and_preserves_arguments()
    -> Result<(), HtmlRuntimeError> {
        DiagramV8Runtime::ensure_initialized();
        let mut isolate = v8::Isolate::new(Default::default());
        v8::scope!(let handle_scope, &mut isolate);
        let context = v8::Context::new(handle_scope, Default::default());
        let context_scope = &mut v8::ContextScope::new(handle_scope, context);
        v8::tc_scope!(let scope, &mut **context_scope);
        let undefined = v8::undefined(scope).into();
        assert!(matches!(
            ParserBodyOnloadInstaller::from_callback_value(scope, undefined),
            Err(HtmlRuntimeError::DomBridge(message))
                if message.starts_with("parser body-onload installer unavailable:")
                    && message.contains("Function")
        ));
        let source = "(source, later) => { globalThis.parserArguments = source === 'second()' && later === true; }";
        let callback = evaluate_value(scope, "parser-callback", source)?;
        let installer = ParserBodyOnloadInstaller::from_callback_value(scope, callback)?;
        installer.install(scope, Some("second()"), true)?;
        assert!(evaluate_value(scope, "parser-arguments", "globalThis.parserArguments")?.is_true());
        Ok(())
    }

    #[test]
    fn parser_body_onload_callback_exception_preserves_real_v8_message()
    -> Result<(), HtmlRuntimeError> {
        DiagramV8Runtime::ensure_initialized();
        let mut isolate = v8::Isolate::new(Default::default());
        v8::scope!(let handle_scope, &mut isolate);
        let context = v8::Context::new(handle_scope, Default::default());
        let context_scope = &mut v8::ContextScope::new(handle_scope, context);
        v8::tc_scope!(let scope, &mut **context_scope);
        let source = "() => { throw new Error('parser callback exploded'); }";
        let callback = evaluate_value(scope, "parser-callback", source)?;
        let installer = ParserBodyOnloadInstaller::from_callback_value(scope, callback)?;
        assert!(matches!(
            installer.install(scope, Some("second()"), true),
            Err(HtmlRuntimeError::JavaScriptException(message))
                if message.contains("Error: parser callback exploded")
        ));
        Ok(())
    }

    #[test]
    fn parser_body_onload_source_allocation_error_preserves_contract() {
        assert!(matches!(
            source_allocation_error(),
            HtmlRuntimeError::JavaScriptException(message)
                if message == "parser source allocation failed"
        ));
    }

    #[test]
    fn parser_body_onload_initial_source_fallback_and_invalid_payload()
    -> Result<(), HtmlRuntimeError> {
        DiagramV8Runtime::ensure_initialized();
        let mut isolate = v8::Isolate::new(Default::default());
        isolate.set_slot(HtmlDomBridgeState::new(HtmlDocument::parse(
            "<body onload='globalThis.parserInstalled = true'>",
        )));
        v8::scope!(let handle_scope, &mut isolate);
        let context = v8::Context::new(handle_scope, Default::default());
        let context_scope = &mut v8::ContextScope::new(handle_scope, context);
        v8::tc_scope!(let scope, &mut **context_scope);
        let installer = install_dom_bridge(scope, "about:blank")?;
        installer.install_initial(scope, "null")?;
        evaluate(scope, "parser-onload-test", "window.onload();")?;
        assert!(
            evaluate_value(scope, "parser-onload-result", "globalThis.parserInstalled")?.is_true()
        );
        assert!(
            matches!(installer.install_initial(scope, "invalid"), Err(HtmlRuntimeError::DomBridge(message)) if message.contains("invalid parser body-onload source"))
        );
        Ok(())
    }
}
