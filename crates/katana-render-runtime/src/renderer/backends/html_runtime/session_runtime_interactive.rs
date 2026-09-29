use crate::renderer::backends::html_runtime::script::{
    BODY_ONLOAD_INSTALL, DOM_CONTENT_LOADED_DISPATCH, WINDOW_LOAD_DISPATCH, check_bridge_error,
    evaluate, install_dom_bridge, perform_microtask_checkpoint,
};
use crate::renderer::backends::html_runtime::types::HtmlRuntimeError;

use super::StaticHtmlRuntime;

type InteractiveScriptEvaluator<'a> = dyn FnMut(&str, &str) -> Result<(), HtmlRuntimeError> + 'a;

impl StaticHtmlRuntime {
    pub(super) fn execute_interactive_scripts(
        isolate: &mut v8::OwnedIsolate,
        scripts: &[String],
        body_onload_script_index: Option<usize>,
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
        Self::run_inline_interactive_scripts(
            document_url,
            scripts,
            body_onload_script_index,
            &mut evaluate,
        )?;
        Self::run_interactive_lifecycle_scripts(document_url, &mut evaluate)?;
        Ok(v8::Global::new(context_scope, context))
    }

    pub(super) fn run_inline_interactive_scripts(
        document_url: &str,
        scripts: &[String],
        body_onload_script_index: Option<usize>,
        evaluate: &mut InteractiveScriptEvaluator<'_>,
    ) -> Result<(), HtmlRuntimeError> {
        for (script_index, script) in scripts.iter().enumerate() {
            Self::install_interactive_body_onload(
                document_url,
                body_onload_script_index,
                script_index,
                evaluate,
            )?;
            let result = evaluate("inline-script", script);
            Self::accept_interactive_script_result(
                result,
                document_url,
                &format!("inline-script[{script_index}]"),
            )?;
        }
        Self::install_interactive_body_onload(
            document_url,
            body_onload_script_index,
            scripts.len(),
            evaluate,
        )
    }

    pub(super) fn install_interactive_body_onload(
        document_url: &str,
        body_onload_script_index: Option<usize>,
        script_index: usize,
        evaluate: &mut InteractiveScriptEvaluator<'_>,
    ) -> Result<(), HtmlRuntimeError> {
        (body_onload_script_index == Some(script_index))
            .then(|| {
                Self::run_interactive_script(
                    document_url,
                    "krr-html-body-onload",
                    BODY_ONLOAD_INSTALL,
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
