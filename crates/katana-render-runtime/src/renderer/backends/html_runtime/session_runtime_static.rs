use crate::renderer::backends::html_runtime::script::{
    DOM_CONTENT_LOADED_DISPATCH, HtmlTryCatchScope, WINDOW_LOAD_DISPATCH,
    body_onload_install_script, check_bridge_error, evaluate, evaluate_and_wait_for_promise,
    install_dom_bridge, perform_microtask_checkpoint,
};
use crate::renderer::backends::html_runtime::types::HtmlRuntimeError;

use super::StaticHtmlRuntime;

type ScriptEvaluator<'a> = dyn FnMut(&str, &str) -> Result<(), HtmlRuntimeError> + 'a;

impl StaticHtmlRuntime {
    pub(super) fn execute_inline_scripts(
        isolate: &mut v8::OwnedIsolate,
        scripts: &[String],
        body_onload_script_index: Option<usize>,
        body_onload_source: Option<&str>,
        document_url: &str,
    ) -> Result<v8::Global<v8::Context>, HtmlRuntimeError> {
        v8::scope!(let handle_scope, isolate);
        let context = v8::Context::new(handle_scope, Default::default());
        let context_scope = &mut v8::ContextScope::new(handle_scope, context);
        v8::tc_scope!(let scope, &mut **context_scope);
        install_dom_bridge(scope, document_url)?;
        let mut execute_script =
            |name: &str, script: &str| evaluate_static_script(scope, name, script);
        Self::run_static_scripts(
            scripts,
            body_onload_script_index,
            body_onload_source,
            ("krr-html-dom-content-loaded", DOM_CONTENT_LOADED_DISPATCH),
            ("krr-html-window-load", WINDOW_LOAD_DISPATCH),
            &mut execute_script,
        )?;
        Ok(v8::Global::new(scope, context))
    }

    fn run_static_scripts(
        scripts: &[String],
        body_onload_script_index: Option<usize>,
        body_onload_source: Option<&str>,
        content_loaded: (&str, &str),
        window_load: (&str, &str),
        execute_script: &mut ScriptEvaluator<'_>,
    ) -> Result<(), HtmlRuntimeError> {
        for (script_index, script) in scripts.iter().enumerate() {
            if body_onload_script_index == Some(script_index) {
                let install = body_onload_install_script(body_onload_source);
                execute_script("krr-html-body-onload", &install)?;
            }
            execute_script("inline-script", script)?;
        }
        if body_onload_script_index == Some(scripts.len()) {
            let install = body_onload_install_script(body_onload_source);
            execute_script("krr-html-body-onload", &install)?;
        }
        execute_script(content_loaded.0, content_loaded.1)?;
        execute_script(window_load.0, window_load.1)?;
        Ok(())
    }
}

fn evaluate_static_script(
    scope: &mut HtmlTryCatchScope<'_, '_, '_, '_>,
    name: &str,
    script: &str,
) -> Result<(), HtmlRuntimeError> {
    let result = if name == "krr-html-window-load" {
        evaluate_and_wait_for_promise(scope, name, script)
    } else {
        evaluate(scope, name, script)
    };
    result
        .and_then(|()| perform_microtask_checkpoint(scope))
        .and_then(|()| check_bridge_error(scope))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_static_scripts_forwards_window_load_error() {
        let mut calls = Vec::new();
        let result = StaticHtmlRuntime::run_static_scripts(
            &[],
            None,
            None,
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
}
