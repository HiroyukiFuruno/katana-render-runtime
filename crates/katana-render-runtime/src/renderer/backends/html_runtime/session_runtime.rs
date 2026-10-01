use crate::markdown::diagram_js_runtime::DiagramV8Runtime;
#[cfg(test)]
use crate::renderer::backends::html_browser::HtmlBrowserSource;
#[cfg(test)]
use crate::renderer::backends::html_debug_trace::HtmlDebugTrace;
use crate::renderer::backends::html_document::{HtmlDocument, HtmlDocumentScript};
use crate::renderer::backends::html_runtime::dom_state::HtmlDomBridgeState;
use crate::renderer::backends::html_runtime::types::HtmlRuntimeError;
use markup5ever_rcdom::{Handle, NodeData};
use std::collections::HashMap;

use super::{StaticHtmlRuntime, StaticHtmlRuntimeSession};

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

    fn requires_runtime(document: &HtmlDocument, scripts: &[HtmlDocumentScript]) -> bool {
        !scripts.is_empty() || contains_lifecycle_handler(&document.document)
    }

    pub(crate) fn start(&self, source: &str) -> Result<StaticHtmlRuntimeSession, HtmlRuntimeError> {
        let document = HtmlDocument::parse(source);
        let scripts = document
            .inline_scripts()
            .map_err(HtmlRuntimeError::ExternalScript)?;
        let body_onload_script_index = document.body_onload_script_index();
        let body_onload_source = document.body_onload_source().map(str::to_owned);

        DiagramV8Runtime::ensure_initialized();
        let mut isolate = v8::Isolate::new(Default::default());
        isolate.set_slot(HtmlDomBridgeState::new(document));

        let context = Self::execute_inline_scripts(
            &mut isolate,
            &scripts,
            body_onload_script_index,
            body_onload_source.as_deref(),
            "about:blank",
        )?;
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
}

fn contains_lifecycle_handler(node: &Handle) -> bool {
    if let NodeData::Element { attrs, .. } = &node.data
        && attrs.borrow().iter().any(|attribute| {
            let name = attribute.name.local.as_str();
            name.eq_ignore_ascii_case("onerror")
                || name.eq_ignore_ascii_case("onload")
                || name.eq_ignore_ascii_case("onreadystatechange")
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
    fn static_window_load_rejection_is_forwarded_to_the_renderer() {
        let result = StaticHtmlRuntime.start(
            "<script>window.addEventListener('load', () => { throw new Error('load'); });</script>",
        );

        assert!(matches!(
            result,
            Err(HtmlRuntimeError::JavaScriptException(message)) if message.contains("load")
        ));
    }

    #[test]
    fn interactive_browser_body_onload_follows_parser_order() {
        let source = must_result(HtmlBrowserSource::new(
            r#"<head><script>window.onload = () => { document.getElementById('status').textContent = 'head'; };</script></head><body onload="document.getElementById('status').textContent = 'body'"><p id=status></p><script>window.onload = () => { document.getElementById('status').textContent = 'body-script'; };</script></body>"#,
            "https://example.test/index.html",
        ));

        let snapshot =
            must_result(must_result(StaticHtmlRuntime.start_interactive(&source)).snapshot());

        assert!(
            snapshot.contains(r#"<p id="status">body-script</p>"#),
            "{snapshot}"
        );
        assert!(
            !snapshot.contains(r#"<p id="status">body</p>"#),
            "{snapshot}"
        );
        assert!(
            !snapshot.contains(r#"<p id="status">head</p>"#),
            "{snapshot}"
        );
    }

    #[test]
    fn interactive_duplicate_body_onload_survives_earlier_attribute_removal() {
        let source = must_result(HtmlBrowserSource::new(
            r#"<body onload="document.getElementById('status').textContent = 'first-body'"><p id=status>Waiting</p><script>document.body.removeAttribute('onload'); globalThis.__krrInstallStaticBodyLoadHandler = () => {};</script><body onload="document.getElementById('status').textContent = 'later-body'">"#,
            "https://example.test/index.html",
        ));

        let snapshot =
            must_result(must_result(StaticHtmlRuntime.start_interactive(&source)).snapshot());

        assert!(
            snapshot.contains(r#"<p id="status">later-body</p>"#),
            "{snapshot}"
        );
    }

    const LATER_BODY_ONLOAD_CASES: &[(&str, &str, &str)] = &[
        (
            r#""#,
            r#"<body onload="document.getElementById('status').textContent = 'second'">"#,
            "first",
        ),
        (
            r#"window.onload = () => document.getElementById('status').textContent = 'author';"#,
            r#"<body onload="document.getElementById('status').textContent = 'first'">"#,
            "author",
        ),
        (
            r#"document.body.removeAttribute('onload');"#,
            r#"<body onload="document.getElementById('status').textContent = 'second'"><script>document.body.removeAttribute('onload');</script><body onload="document.getElementById('status').textContent = 'third'">"#,
            "third",
        ),
        (
            r#"document.body.removeAttribute('onload');"#,
            r#"<template><body onload="document.getElementById('status').textContent = 'second'"></template><select><body onload="document.getElementById('status').textContent = 'second'"></select>"#,
            "Waiting",
        ),
        (
            r#"document.body.removeAttribute('onload'); document.body.getAttribute = () => 'spoof'; document.body.setAttribute = () => {}; Object.defineProperty(document, 'body', { get: () => null }); document.querySelector = () => null; globalThis.__krr_dom = () => null;"#,
            r#"<body onload="document.getElementById('status').textContent = 'second'">"#,
            "second",
        ),
        (
            r#"const savedFunction = Function; const savedString = String; const savedApply = Reflect.apply; const statusCaptured = document.getElementById('status'); document.body.removeAttribute('onload'); globalThis.__krrInstallStaticBodyLoadHandler = () => {}; globalThis.Function = () => null; globalThis.String = () => 'spoof'; Reflect.apply = () => null;"#,
            r#"<body onload="statusCaptured.textContent = 'second'"><script>globalThis.Function = savedFunction; globalThis.String = savedString; Reflect.apply = savedApply;</script>"#,
            "second",
        ),
        (
            r#"const savedMapGet = Map.prototype.get; const savedMapSet = Map.prototype.set; const savedMapDelete = Map.prototype.delete; const savedWeakGet = WeakMap.prototype.get; const savedWeakSet = WeakMap.prototype.set; const statusCaptured = document.getElementById('status'); document.body.removeAttribute('onload'); Map.prototype.get = () => undefined; Map.prototype.set = () => {}; Map.prototype.delete = () => {}; WeakMap.prototype.get = () => undefined; WeakMap.prototype.set = () => {};"#,
            r#"<body onload="statusCaptured.textContent = 'second'"><script>Map.prototype.get = savedMapGet; Map.prototype.set = savedMapSet; Map.prototype.delete = savedMapDelete; WeakMap.prototype.get = savedWeakGet; WeakMap.prototype.set = savedWeakSet;</script>"#,
            "second",
        ),
        (
            r#"document.body.removeAttribute('onload'); window.authorCalls = 0; globalThis.__krrInstallStaticBodyLoadHandler = () => { window.authorCalls += 1; };"#,
            r#"<body onload="document.getElementById('status').textContent = `second:${window.authorCalls}`"><script>__krrInstallStaticBodyLoadHandler('author-source', true);</script>"#,
            "second:1",
        ),
    ];

    #[test]
    fn interactive_later_duplicate_body_onload_preserves_live_parser_state() {
        let first = "document.getElementById('status').textContent = 'first'";
        for (middle, later, expected) in LATER_BODY_ONLOAD_CASES {
            let source = must_result(HtmlBrowserSource::new(
                format!(
                    r#"<body onload="{first}"><p id=status>Waiting</p><script>{middle}</script>{later}"#
                ),
                "https://example.test/index.html",
            ));
            let snapshot =
                must_result(must_result(StaticHtmlRuntime.start_interactive(&source)).snapshot());
            assert!(
                snapshot.contains(&format!(">{expected}</p>")),
                "{}: {snapshot}",
                source.raw_html
            );
        }
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
    fn scriptless_image_onerror_uses_the_runtime_lifecycle_dispatch() {
        let snapshot = must_result(StaticHtmlRuntime.render(
            r#"<p id=status>Waiting</p><img src="data:image/png;base64,!" onerror="document.getElementById('status').textContent = 'Ready'">"#,
        ));

        assert!(
            snapshot.contains(r#"<p id="status">Ready</p>"#),
            "{snapshot}"
        );
    }

    #[test]
    fn null_lifecycle_property_does_not_fall_back_to_content_attribute() {
        let snapshot = must_result(StaticHtmlRuntime.render(
            r#"<p id=status>Waiting</p><iframe id=frame data-krr-local-frame onload="document.getElementById('status').textContent = 'Unexpected'"></iframe><script>document.getElementById('frame').onload = null;</script>"#,
        ));

        assert!(
            snapshot.contains(r#"<p id="status">Waiting</p>"#),
            "{snapshot}"
        );
    }

    #[test]
    fn empty_lifecycle_attribute_exposes_a_callable_noop_property() {
        let snapshot = must_result(StaticHtmlRuntime.render(
            r#"<p id=status>Waiting</p><iframe id=frame data-krr-local-frame onload=""></iframe><script>
                const frame = document.getElementById('frame');
                const status = document.getElementById('status');
                status.setAttribute('data-handler-type', typeof frame.onload);
                frame.onload(new Event('load'));
                status.textContent = 'Ready';
            </script>"#,
        ));

        assert!(
            snapshot.contains(r#"<p id="status" data-handler-type="function">Ready</p>"#),
            "{snapshot}"
        );
    }

    #[test]
    fn removing_absent_lifecycle_attribute_preserves_property_handler() {
        let snapshot = must_result(StaticHtmlRuntime.render(
            r#"<p id=status>Waiting</p><iframe id=frame data-krr-local-frame></iframe><script>
                const frame = document.getElementById('frame');
                frame.onload = () => document.getElementById('status').textContent = 'Ready';
                frame.removeAttribute('onload');
            </script>"#,
        ));

        assert!(
            snapshot.contains(r#"<p id="status">Ready</p>"#),
            "{snapshot}"
        );
    }

    #[test]
    fn removing_present_lifecycle_attribute_clears_property_handler() {
        let snapshot = must_result(StaticHtmlRuntime.render(
            r#"<p id=status>Waiting</p><iframe id=frame data-krr-local-frame onload="document.getElementById('status').textContent = 'Unexpected'"></iframe><script>
                const frame = document.getElementById('frame');
                frame.onload = () => document.getElementById('status').textContent = 'Ready';
                frame.removeAttribute('onload');
            </script>"#,
        ));

        assert!(
            snapshot.contains(r#"<p id="status">Waiting</p>"#),
            "{snapshot}"
        );
    }

    #[test]
    fn dynamically_added_body_onload_uses_the_window_lifecycle_dispatch() {
        let snapshot = must_result(StaticHtmlRuntime.render(
            r#"<body><p id=status>Waiting</p><script>
                document.body.setAttribute('onload', "document.getElementById('status').textContent = 'Ready'");
            </script></body>"#,
        ));

        assert!(
            snapshot.contains(r#"<p id="status">Ready</p>"#),
            "{snapshot}"
        );
    }

    #[test]
    fn replacing_body_onload_replaces_the_window_lifecycle_handler() {
        let snapshot = must_result(StaticHtmlRuntime.render(
            r#"<body onload="document.getElementById('status').textContent = 'Old'"><p id=status>Waiting</p><script>
                document.body.setAttribute('onload', "document.getElementById('status').textContent = 'New'");
            </script></body>"#,
        ));

        assert!(snapshot.contains(r#"<p id="status">New</p>"#), "{snapshot}");
    }

    #[test]
    fn removing_body_onload_clears_the_window_lifecycle_handler() {
        let snapshot = must_result(StaticHtmlRuntime.render(
            r#"<body onload="document.getElementById('status').textContent = 'Unexpected'"><p id=status>Waiting</p><script>
                document.body.removeAttribute('onload');
            </script></body>"#,
        ));

        assert!(
            snapshot.contains(r#"<p id="status">Waiting</p>"#),
            "{snapshot}"
        );
    }

    #[test]
    fn lifecycle_attribute_stringifies_stateful_values_once_for_storage_and_handler() {
        let snapshot = must_result(StaticHtmlRuntime.render(
            r#"<p id=status>Waiting</p><iframe id=frame data-krr-local-frame></iframe><script>
                const frame = document.getElementById('frame');
                const status = document.getElementById('status');
                const values = ["status.textContent = 'first'", "status.textContent = 'second'"];
                let reads = 0;
                const source = { toString() { return values[reads++]; } };
                frame.setAttribute('onload', source);
                status.setAttribute('data-reads', reads);
                status.setAttribute('data-source', frame.getAttribute('onload'));
            </script>"#,
        ));

        assert!(
            snapshot.contains(
                r#"<p id="status" data-reads="1" data-source="status.textContent = 'first'">first</p>"#
            ),
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
        let static_result = StaticHtmlRuntime::execute_inline_scripts(
            &mut static_isolate,
            &[],
            None,
            None,
            "http://[",
        );
        assert!(matches!(
            static_result,
            Err(HtmlRuntimeError::DomBridge(message))
                if message.starts_with("invalid document URL:")
        ));

        let mut interactive_isolate = v8::Isolate::new(Default::default());
        let interactive_result = StaticHtmlRuntime::execute_interactive_scripts(
            &mut interactive_isolate,
            &[],
            None,
            None,
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
    fn repeated_page_dom_callbacks_cannot_extend_the_execution_budget_indefinitely() {
        let result = StaticHtmlRuntime
            .start("<script>while (true) { __krr_dom('getElementById', 'missing'); }</script>");

        assert_eq!(result.err(), Some(HtmlRuntimeError::ExecutionTimeout));
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
    fn inline_scripts_forward_body_onload_install_errors() {
        let result = StaticHtmlRuntime::run_inline_interactive_scripts(
            "https://example.test/index.html",
            &[super::HtmlDocumentScript::Source(
                "document.body.dataset.ready = 'yes';".to_string(),
            )],
            Some(0),
            None,
            &mut |script, _source| Err(HtmlRuntimeError::JavaScriptCompile(script.to_string())),
        );

        assert!(matches!(
            result,
            Err(HtmlRuntimeError::JavaScriptCompile(message)) if message == "krr-html-body-onload"
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
