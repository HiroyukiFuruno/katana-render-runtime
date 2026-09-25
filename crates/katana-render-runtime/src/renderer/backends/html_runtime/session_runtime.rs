use crate::markdown::diagram_js_runtime::DiagramV8Runtime;
#[cfg(test)]
use crate::renderer::backends::html_browser::HtmlBrowserSource;
#[cfg(test)]
use crate::renderer::backends::html_debug_trace::HtmlDebugTrace;
use crate::renderer::backends::html_document::HtmlDocument;
use crate::renderer::backends::html_runtime::dom_state::HtmlDomBridgeState;
use crate::renderer::backends::html_runtime::types::HtmlRuntimeError;
use markup5ever_rcdom::{Handle, NodeData};
use std::collections::HashMap;

use super::super::script::{
    DOM_CONTENT_LOADED_DISPATCH, WINDOW_LOAD_DISPATCH, check_bridge_error, evaluate,
    install_dom_bridge, perform_microtask_checkpoint,
};
use super::{StaticHtmlRuntime, StaticHtmlRuntimeSession};

type ScriptEvaluator<'a> = dyn FnMut(&str, &str) -> Result<(), HtmlRuntimeError> + 'a;

impl StaticHtmlRuntime {
    pub(crate) fn render(&self, source: &str) -> Result<String, HtmlRuntimeError> {
        let document = HtmlDocument::parse(source);
        let scripts = document
            .inline_scripts()
            .map_err(HtmlRuntimeError::ExternalScript)?;
        if !Self::requires_runtime(&document, &scripts) {
            return Ok(document.render());
        }
        self.start(source)?.snapshot()
    }

    fn requires_runtime(document: &HtmlDocument, scripts: &[String]) -> bool {
        !scripts.is_empty() || contains_lifecycle_handler(&document.document)
    }

    pub(crate) fn start(&self, source: &str) -> Result<StaticHtmlRuntimeSession, HtmlRuntimeError> {
        let document = HtmlDocument::parse(source);
        let scripts = document
            .inline_scripts()
            .map_err(HtmlRuntimeError::ExternalScript)?;

        DiagramV8Runtime::ensure_initialized();
        let mut isolate = v8::Isolate::new(Default::default());
        isolate.set_slot(HtmlDomBridgeState::new(document));

        let context = Self::execute_inline_scripts(&mut isolate, &scripts, "about:blank")?;
        Ok(StaticHtmlRuntimeSession {
            context: Some(context),
            isolate: Some(isolate),
            external_stylesheets: HashMap::new(),
        })
    }

    #[cfg(test)]
    pub(in crate::renderer::backends) fn start_interactive(
        &self,
        source: &HtmlBrowserSource,
    ) -> Result<StaticHtmlRuntimeSession, HtmlRuntimeError> {
        self.start_interactive_traced(source, &HtmlDebugTrace::disabled())
    }

    fn execute_inline_scripts(
        isolate: &mut v8::OwnedIsolate,
        scripts: &[String],
        document_url: &str,
    ) -> Result<v8::Global<v8::Context>, HtmlRuntimeError> {
        v8::scope!(let handle_scope, isolate);
        let context = v8::Context::new(handle_scope, Default::default());
        let context_scope = &mut v8::ContextScope::new(handle_scope, context);
        v8::tc_scope!(let scope, &mut **context_scope);
        install_dom_bridge(scope, document_url)?;
        let mut execute_script = |name: &str, script: &str| -> Result<(), HtmlRuntimeError> {
            evaluate(scope, name, script)
                .and_then(|()| perform_microtask_checkpoint(scope))
                .and_then(|()| check_bridge_error(scope))
        };
        Self::run_static_scripts(
            document_url,
            scripts,
            ("krr-html-dom-content-loaded", DOM_CONTENT_LOADED_DISPATCH),
            ("krr-html-window-load", WINDOW_LOAD_DISPATCH),
            &mut execute_script,
        )?;
        Ok(v8::Global::new(scope, context))
    }

    pub(super) fn execute_interactive_scripts(
        isolate: &mut v8::OwnedIsolate,
        scripts: &[String],
        document_url: &str,
    ) -> Result<v8::Global<v8::Context>, HtmlRuntimeError> {
        v8::scope!(let handle_scope, isolate);
        let context = v8::Context::new(handle_scope, Default::default());
        let context_scope = &mut v8::ContextScope::new(handle_scope, context);
        {
            v8::tc_scope!(let scope, &mut **context_scope);
            install_dom_bridge(scope, document_url)?;
        }
        let mut evaluate = |name: &str, script: &str| -> Result<(), HtmlRuntimeError> {
            v8::tc_scope!(let scope, &mut **context_scope);
            evaluate(scope, name, script)
                .and_then(|()| perform_microtask_checkpoint(scope))
                .and_then(|()| check_bridge_error(scope))
        };
        Self::run_inline_interactive_scripts(document_url, scripts, &mut evaluate)?;
        Self::run_interactive_lifecycle_scripts(document_url, &mut evaluate)?;
        Ok(v8::Global::new(context_scope, context))
    }

    fn run_static_scripts(
        _document_url: &str,
        scripts: &[String],
        content_loaded: (&str, &str),
        window_load: (&str, &str),
        execute_script: &mut ScriptEvaluator<'_>,
    ) -> Result<(), HtmlRuntimeError> {
        for script in scripts {
            execute_script("inline-script", script)?;
        }
        execute_script(content_loaded.0, content_loaded.1)?;
        execute_script(window_load.0, window_load.1)?;
        Ok(())
    }

    fn run_inline_interactive_scripts(
        document_url: &str,
        scripts: &[String],
        evaluate: &mut ScriptEvaluator<'_>,
    ) -> Result<(), HtmlRuntimeError> {
        for (script_index, script) in scripts.iter().enumerate() {
            let result = evaluate("inline-script", script);
            Self::accept_interactive_script_result(
                result,
                document_url,
                &format!("inline-script[{script_index}]"),
            )?;
        }
        Ok(())
    }

    fn run_interactive_lifecycle_scripts(
        document_url: &str,
        evaluate: &mut ScriptEvaluator<'_>,
    ) -> Result<(), HtmlRuntimeError> {
        Self::accept_interactive_script_result(
            evaluate("krr-html-dom-content-loaded", DOM_CONTENT_LOADED_DISPATCH),
            document_url,
            "krr-html-dom-content-loaded",
        )?;
        Self::accept_interactive_script_result(
            evaluate("krr-html-window-load", WINDOW_LOAD_DISPATCH),
            document_url,
            "krr-html-window-load",
        )?;
        Ok(())
    }

    fn accept_interactive_script_result(
        result: Result<(), HtmlRuntimeError>,
        document_url: &str,
        script: &str,
    ) -> Result<(), HtmlRuntimeError> {
        match result {
            Ok(()) => Ok(()),
            Err(error @ HtmlRuntimeError::JavaScriptException(_)) => {
                tracing::error!(
                    document_url,
                    script,
                    error = %error,
                    "Interactive HTML script failed; continuing document execution"
                );
                Ok(())
            }
            Err(error) => Err(error),
        }
    }
}

fn contains_lifecycle_handler(node: &Handle) -> bool {
    if let NodeData::Element { attrs, .. } = &node.data
        && attrs.borrow().iter().any(|attribute| {
            let name = attribute.name.local.as_str();
            name.eq_ignore_ascii_case("onload") || name.eq_ignore_ascii_case("onreadystatechange")
        })
    {
        return true;
    }
    node.children
        .borrow()
        .iter()
        .any(contains_lifecycle_handler)
}

#[cfg(test)]
mod tests {
    use super::{DiagramV8Runtime, HtmlDocument, StaticHtmlRuntime};
    use crate::renderer::backends::HtmlBrowserSource;
    use crate::renderer::backends::html_runtime::types::HtmlRuntimeError;

    fn must_result<T, E>(result: Result<T, E>) -> T {
        assert!(result.is_ok());
        let mut values = result.into_iter().collect::<Vec<_>>();
        values.remove(0)
    }

    #[test]
    fn lifecycle_listener_exceptions_are_logged_as_non_fatal() {
        let source = must_result(HtmlBrowserSource::new(
            "<body><p id=marker></p><script>document.addEventListener('DOMContentLoaded', () => { \
             document.getElementById('marker').textContent = 'doc'; throw new Error('doc'); });\
             window.addEventListener('load', () => { document.getElementById('marker').textContent += 'load'; throw new Error('load'); });\
             </script>",
            "https://example.test/path/index.html",
        ));
        let session = must_result(StaticHtmlRuntime.start_interactive(&source));
        let snapshot = must_result(session.snapshot());
        assert!(snapshot.contains("<p id=\"marker\">docload</p>"));
    }

    #[test]
    fn scriptless_local_iframe_onload_uses_the_runtime_lifecycle_dispatch() {
        let snapshot = must_result(StaticHtmlRuntime.render(
            r#"<p id=status>Waiting</p><iframe data-krr-local-frame onload="document.getElementById('status').textContent = 'Ready'"></iframe>"#,
        ));

        assert!(
            snapshot.contains(r#"<p id="status">Ready</p>"#),
            "{snapshot}"
        );
    }

    #[test]
    fn scriptless_document_without_lifecycle_handlers_skips_the_runtime() {
        let document = HtmlDocument::parse("<p>Static</p>");
        let scripts = must_result(document.inline_scripts());

        assert!(!StaticHtmlRuntime::requires_runtime(&document, &scripts));
    }

    #[test]
    fn run_interactive_lifecycle_scripts_executes_both_handlers() {
        let mut called = Vec::new();
        let result = StaticHtmlRuntime::run_interactive_lifecycle_scripts(
            "https://example.test/index.html",
            &mut |script, _source| {
                called.push(script.to_string());
                Ok(())
            },
        );
        assert!(result.is_ok());
        assert_eq!(
            called,
            vec!["krr-html-dom-content-loaded", "krr-html-window-load"]
        );
    }

    #[test]
    fn script_context_setup_rejects_invalid_document_urls_in_both_modes() {
        DiagramV8Runtime::ensure_initialized();
        let mut static_isolate = v8::Isolate::new(Default::default());
        let static_result =
            StaticHtmlRuntime::execute_inline_scripts(&mut static_isolate, &[], "http://[");
        assert!(matches!(
            static_result,
            Err(HtmlRuntimeError::DomBridge(message))
                if message.starts_with("invalid document URL:")
        ));

        let mut interactive_isolate = v8::Isolate::new(Default::default());
        let interactive_result = StaticHtmlRuntime::execute_interactive_scripts(
            &mut interactive_isolate,
            &[],
            "http://[",
        );
        assert!(matches!(
            interactive_result,
            Err(HtmlRuntimeError::DomBridge(message))
                if message.starts_with("invalid document URL:")
        ));
    }

    #[test]
    fn static_microtasks_and_interactive_lifecycle_handlers_enforce_the_execution_budget() {
        let static_timeout = StaticHtmlRuntime
            .start("<script>Promise.resolve().then(() => { for (;;) {} });</script>");
        assert_eq!(
            static_timeout.err(),
            Some(HtmlRuntimeError::ExecutionTimeout)
        );

        let source = must_result(HtmlBrowserSource::new(
            "<script>document.addEventListener('DOMContentLoaded', () => { for (;;) {} });</script>",
            "https://example.test/index.html",
        ));
        let lifecycle_timeout = StaticHtmlRuntime.start_interactive(&source);
        assert_eq!(
            lifecycle_timeout.err(),
            Some(HtmlRuntimeError::ExecutionTimeout)
        );
    }

    #[test]
    fn run_static_scripts_forwards_window_load_error() {
        let mut calls = Vec::new();
        let result = StaticHtmlRuntime::run_static_scripts(
            "https://example.test/index.html",
            &[],
            ("content-loaded", ""),
            ("window-load", ""),
            &mut |name, _source| {
                calls.push(name.to_string());
                if name == "window-load" {
                    return Err(HtmlRuntimeError::JavaScriptCompile(name.to_string()));
                }
                Ok(())
            },
        );

        assert_eq!(calls, ["content-loaded", "window-load"]);
        assert!(matches!(
            result,
            Err(HtmlRuntimeError::JavaScriptCompile(message)) if message == "window-load"
        ));
    }

    #[test]
    fn run_interactive_lifecycle_scripts_forwards_dom_content_loaded_error() {
        let result = StaticHtmlRuntime::run_interactive_lifecycle_scripts(
            "https://example.test/index.html",
            &mut |script, _source| Err(HtmlRuntimeError::JavaScriptCompile(script.to_string())),
        );
        assert!(matches!(
            result,
            Err(HtmlRuntimeError::JavaScriptCompile(message))
                if message == "krr-html-dom-content-loaded"
        ));
    }

    #[test]
    fn run_interactive_lifecycle_scripts_forwards_window_load_error() {
        let result = StaticHtmlRuntime::run_interactive_lifecycle_scripts(
            "https://example.test/index.html",
            &mut |script, _source| match script {
                "krr-html-dom-content-loaded" => Ok(()),
                _ => Err(HtmlRuntimeError::JavaScriptCompile(script.to_string())),
            },
        );
        assert!(matches!(
            result,
            Err(HtmlRuntimeError::JavaScriptCompile(message))
                if message == "krr-html-window-load"
        ));
    }

    #[test]
    fn accept_interactive_script_result_forwards_non_javascript_errors() {
        assert!(matches!(
            StaticHtmlRuntime::accept_interactive_script_result(
                Err(HtmlRuntimeError::JavaScriptCompile("compile failed".to_string())),
                "https://example.test/index.html",
                "inline-script",
            ),
            Err(HtmlRuntimeError::JavaScriptCompile(message))
                if message == "compile failed"
        ));
    }

    #[test]
    fn accept_interactive_script_result_logs_javascript_exception_as_non_fatal() {
        assert_eq!(
            StaticHtmlRuntime::accept_interactive_script_result(
                Err(HtmlRuntimeError::JavaScriptException(
                    "listener failed".to_string()
                )),
                "https://example.test/index.html",
                "inline-script",
            ),
            Ok(())
        );
    }
}
