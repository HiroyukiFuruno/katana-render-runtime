use crate::renderer::backends::html_document::HtmlDocumentScript;
use crate::renderer::backends::html_runtime::script::{
    DOM_CONTENT_LOADED_DISPATCH, ParserBodyOnloadInstaller, WINDOW_LOAD_DISPATCH,
    body_onload_source_payload, check_bridge_error, evaluate, evaluate_and_wait_for_promise,
    evaluate_value, install_dom_bridge, perform_microtask_checkpoint,
};
use crate::renderer::backends::html_runtime::types::HtmlRuntimeError;

use super::StaticHtmlRuntime;

type InteractiveScriptEvaluator<'a> = dyn FnMut(&str, &str) -> Result<(), HtmlRuntimeError> + 'a;

impl StaticHtmlRuntime {
    pub(super) fn execute_interactive_scripts(
        isolate: &mut v8::OwnedIsolate,
        scripts: &[HtmlDocumentScript],
        body_onload_script_index: Option<usize>,
        body_onload_source: Option<&str>,
        document_url: &str,
    ) -> Result<v8::Global<v8::Context>, HtmlRuntimeError> {
        Self::with_new_context(isolate, |context_scope, context| {
            let installer = {
                v8::tc_scope!(let scope, &mut **context_scope);
                install_dom_bridge(scope, document_url)?
            };
            let mut evaluate = |name: &str, script: &str| -> Result<(), HtmlRuntimeError> {
                Self::evaluate_interactive_script(context_scope, name, script, &installer)
            };
            Self::run_inline_interactive_scripts(
                document_url,
                scripts,
                body_onload_script_index,
                body_onload_source,
                &mut evaluate,
            )?;
            Self::run_interactive_lifecycle_scripts(document_url, &mut evaluate)?;
            Ok(v8::Global::new(context_scope, context))
        })
    }

    fn evaluate_interactive_script(
        context_scope: &mut v8::ContextScope<'_, '_, v8::HandleScope<'_>>,
        name: &str,
        script: &str,
        installer: &ParserBodyOnloadInstaller,
    ) -> Result<(), HtmlRuntimeError> {
        v8::tc_scope!(let scope, &mut **context_scope);
        if name == "krr-html-body-onload" {
            installer.install_initial(scope, script)
        } else if name == "krr-html-later-body-onload" {
            installer.install(scope, Some(script), true)
        } else if name == "krr-html-window-load" {
            evaluate_and_wait_for_promise(scope, name, script)
        } else {
            evaluate(scope, name, script)
        }
        .and_then(|()| perform_microtask_checkpoint(scope))
        .and_then(|()| check_bridge_error(scope))
    }

    pub(super) fn run_inline_interactive_scripts(
        document_url: &str,
        scripts: &[HtmlDocumentScript],
        body_onload_script_index: Option<usize>,
        body_onload_source: Option<&str>,
        evaluate: &mut InteractiveScriptEvaluator<'_>,
    ) -> Result<(), HtmlRuntimeError> {
        for (script_index, script) in scripts.iter().enumerate() {
            Self::install_interactive_body_onload(
                document_url,
                body_onload_script_index,
                body_onload_source,
                script_index,
                evaluate,
            )?;
            let result = evaluate(script.name(), script.source());
            Self::accept_interactive_script_result(
                result,
                document_url,
                &format!("inline-script[{script_index}]"),
            )?;
        }
        Self::install_interactive_body_onload(
            document_url,
            body_onload_script_index,
            body_onload_source,
            scripts.len(),
            evaluate,
        )
    }

    pub(super) fn install_interactive_body_onload(
        document_url: &str,
        body_onload_script_index: Option<usize>,
        body_onload_source: Option<&str>,
        script_index: usize,
        evaluate: &mut InteractiveScriptEvaluator<'_>,
    ) -> Result<(), HtmlRuntimeError> {
        (body_onload_script_index == Some(script_index))
            .then(|| {
                let install = body_onload_source_payload(body_onload_source);
                Self::run_interactive_script(
                    document_url,
                    "krr-html-body-onload",
                    &install,
                    evaluate,
                )
            })
            .transpose()
            .map(|_| ())
    }

    pub(super) fn run_interactive_script(
        document_url: &str,
        name: &str,
        source: &str,
        evaluate: &mut InteractiveScriptEvaluator<'_>,
    ) -> Result<(), HtmlRuntimeError> {
        Self::accept_interactive_script_result(evaluate(name, source), document_url, name)
    }

    pub(super) fn run_interactive_lifecycle_scripts(
        document_url: &str,
        evaluate: &mut InteractiveScriptEvaluator<'_>,
    ) -> Result<(), HtmlRuntimeError> {
        let dom_content_loaded =
            format!("{DOM_CONTENT_LOADED_DISPATCH};globalThis.__krrRunTimers?.()");
        Self::accept_interactive_script_result(
            evaluate("krr-html-dom-content-loaded", &dom_content_loaded),
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

    pub(super) fn accept_interactive_script_result(
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

impl super::StaticHtmlRuntimeSession {
    pub(crate) fn advance_timers(&mut self, elapsed_ms: f64) -> Result<bool, HtmlRuntimeError> {
        let isolate = self
            .isolate
            .as_mut()
            .ok_or_else(super::session_interaction::discarded_runtime_error)?;
        let context = self
            .context
            .as_ref()
            .ok_or_else(super::session_interaction::discarded_runtime_error)?;
        v8::scope!(let handle_scope, isolate);
        let context = v8::Local::new(handle_scope, context);
        let context_scope = &mut v8::ContextScope::new(handle_scope, context);
        v8::tc_scope!(let scope, &mut **context_scope);
        let elapsed = if elapsed_ms.is_finite() {
            elapsed_ms.max(0.0)
        } else {
            0.0
        };
        let ran = evaluate_value(
            scope,
            "krr-html-timer-pump",
            &format!("globalThis.__krrRunTimers?.({elapsed});"),
        )?
        .is_true();
        perform_microtask_checkpoint(scope)?;
        check_bridge_error(scope)?;
        Ok(ran)
    }
}

#[cfg(test)]
mod tests {
    use super::StaticHtmlRuntime;
    use crate::renderer::backends::html_document::HtmlDocumentScript;

    #[test]
    fn inline_scripts_install_body_onload_at_source_order() -> Result<(), String> {
        let scripts = vec![
            HtmlDocumentScript::Source("first()".to_string()),
            HtmlDocumentScript::Source("second()".to_string()),
        ];
        let mut calls = Vec::new();
        StaticHtmlRuntime::run_inline_interactive_scripts(
            "https://example.test/index.html",
            &scripts,
            Some(1),
            Some("onload()"),
            &mut |name, source| {
                calls.push((name.to_string(), source.to_string()));
                Ok(())
            },
        )
        .map_err(test_error)?;
        assert_eq!(
            calls,
            vec![
                ("inline-script".to_string(), "first()".to_string()),
                (
                    "krr-html-body-onload".to_string(),
                    "\"onload()\"".to_string()
                ),
                ("inline-script".to_string(), "second()".to_string()),
            ]
        );
        Ok(())
    }

    #[test]
    fn lifecycle_scripts_are_dispatched_in_dom_load_order() -> Result<(), String> {
        let mut names = Vec::new();
        StaticHtmlRuntime::run_interactive_lifecycle_scripts(
            "https://example.test/index.html",
            &mut |name, _source| {
                names.push(name.to_string());
                Ok(())
            },
        )
        .map_err(test_error)?;
        assert_eq!(
            names,
            vec![
                "krr-html-dom-content-loaded".to_string(),
                "krr-html-window-load".to_string()
            ]
        );
        Ok(())
    }

    #[test]
    fn timer_advance_normalizes_non_finite_elapsed_input() -> Result<(), String> {
        let mut session = StaticHtmlRuntime
            .start("<main>timer boundary</main>")
            .map_err(test_error)?;
        assert!(!session.advance_timers(f64::NAN).map_err(test_error)?);
        Ok(())
    }

    #[test]
    fn timer_exception_propagates_as_typed_runtime_error() -> Result<(), String> {
        let mut session = StaticHtmlRuntime
            .start("<script>setTimeout(() => { throw new Error('timer failure') }, 1)</script>")
            .map_err(test_error)?;
        let error = session
            .advance_timers(1.0)
            .err()
            .ok_or("timer exception must propagate")?;
        assert!(matches!(
            error,
            super::HtmlRuntimeError::JavaScriptException(_)
        ));
        Ok(())
    }

    fn test_error(error: super::HtmlRuntimeError) -> String {
        error.to_string()
    }

    #[test]
    fn test_error_preserves_real_runtime_failure() -> Result<(), String> {
        let mut session = StaticHtmlRuntime
            .start(
                "<script>setTimeout(() => { throw new Error('typed-runtime-error') }, 1)</script>",
            )
            .map_err(test_error)?;
        let error = session
            .advance_timers(1.0)
            .err()
            .ok_or("script error missing")?;
        assert!(matches!(
            &error,
            super::HtmlRuntimeError::JavaScriptException(_)
        ));
        assert!(test_error(error).contains("typed-runtime-error"));
        Ok(())
    }
}
