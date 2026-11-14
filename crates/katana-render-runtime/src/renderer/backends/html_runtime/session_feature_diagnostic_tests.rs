use super::{StaticHtmlRuntime, StaticHtmlRuntimeSession};
use crate::renderer::backends::html_browser::HtmlBrowserSource;
use crate::renderer::backends::html_runtime::evaluation::evaluate_value;
use crate::renderer::backends::html_runtime::types::HtmlRuntimeError;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

fn observe_state(
    session: &mut StaticHtmlRuntimeSession,
    expression: &str,
) -> Result<serde_json::Value, String> {
    session.observe_feature_state(expression)
}

impl StaticHtmlRuntimeSession {
    pub(crate) fn query_feature_node(
        &mut self,
        selector: &str,
    ) -> Option<crate::renderer::backends::html_runtime::types::HtmlNodeId> {
        self.isolate
            .as_mut()?
            .get_slot::<crate::renderer::backends::html_runtime::dom_state::HtmlDomBridgeState>()?
            .document
            .borrow_mut()
            .query_selector(selector)
            .map(crate::renderer::backends::html_runtime::types::HtmlNodeId)
    }

    pub(crate) fn observe_feature_state(
        &mut self,
        expression: &str,
    ) -> Result<serde_json::Value, String> {
        let isolate = self.isolate.as_mut().ok_or("missing runtime isolate")?;
        let context = self.context.as_ref().ok_or("missing runtime context")?;
        v8::scope!(let handle_scope, isolate);
        let context = v8::Local::new(handle_scope, context);
        let context_scope = &mut v8::ContextScope::new(handle_scope, context);
        v8::tc_scope!(let scope, &mut **context_scope);
        let code = format!("JSON.stringify(({expression}))");
        let result = evaluate_value(scope, "krr-feature-state-observation", &code)
            .map_err(|error: HtmlRuntimeError| error.to_string())?;
        let value = result.to_string(scope).ok_or("state is not serializable")?;
        serde_json::from_str(&value.to_rust_string_lossy(scope)).map_err(test_error)
    }
}

#[test]
fn writes_fixture_feature_state_when_requested() -> Result<(), String> {
    let Some(input) = std::env::var_os("KRR_HTML_FEATURE_INPUT") else {
        return Ok(());
    };
    let input = PathBuf::from(input).canonicalize().map_err(test_error)?;
    let html = fs::read_to_string(&input).map_err(test_error)?;
    let origin = url::Url::from_file_path(&input).map_err(|()| "invalid origin")?;
    let source = HtmlBrowserSource::new(html.clone(), origin.as_str()).map_err(test_error)?;
    let expression = std::env::var("KRR_HTML_FEATURE_EXPRESSION").map_err(test_error)?;
    let nonce = std::env::var("KRR_HTML_FEATURE_NONCE").map_err(test_error)?;
    let binary_sha = std::env::var("KRR_HTML_FEATURE_BINARY_SHA256").map_err(test_error)?;
    let output =
        PathBuf::from(std::env::var_os("KRR_HTML_FEATURE_OUTPUT").ok_or("missing output")?);
    let mut session = StaticHtmlRuntime
        .start_interactive(&source)
        .map_err(test_error)?;
    let state = observe_state(&mut session, &expression)?;
    let hash = html_hash(&html);
    let report = json!({"htmlSha256": hash, "executionNonce": nonce, "binarySha256": binary_sha, "state": state});
    fs::write(
        output,
        serde_json::to_vec_pretty(&report).map_err(test_error)?,
    )
    .map_err(test_error)
}

fn html_hash(html: &str) -> String {
    Sha256::digest(html.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
#[path = "session_feature_diagnostic_child_tests.rs"]
mod child_process_tests;

fn test_error<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}

#[test]
fn state_observation_rejects_json_undefined_values() -> Result<(), String> {
    let source = HtmlBrowserSource::new(
        "<p id=state>ready</p>",
        "https://example.test/undefined-feature.html",
    )
    .map_err(test_error)?;
    let mut session = StaticHtmlRuntime
        .start_interactive(&source)
        .map_err(test_error)?;
    for expression in ["undefined", "Symbol('not-json')"] {
        let error = observe_state(&mut session, expression)
            .err()
            .ok_or("undefined observation must return Err")?;
        assert!(!error.is_empty(), "observation error must be reported");
    }
    Ok(())
}

#[test]
fn state_observation_preserves_selector_miss_and_nonserializable_errors() -> Result<(), String> {
    let source = HtmlBrowserSource::new(
        "<p id=state>ready</p>",
        "https://example.test/selector-miss.html",
    )
    .map_err(test_error)?;
    let mut session = StaticHtmlRuntime
        .start_interactive(&source)
        .map_err(test_error)?;
    assert_eq!(
        observe_state(&mut session, "document.querySelector('#missing')")?,
        serde_json::Value::Null
    );
    let error = observe_state(
        &mut session,
        "document.querySelector('#missing').textContent",
    )
    .err()
    .ok_or("missing selector dereference must return Err")?;
    assert!(!error.is_empty());
    Ok(())
}

fn with_success<T, E>(result: Result<T, E>, context: &str, action: impl FnOnce(T)) {
    assert!(
        result.is_ok_and(|value| {
            action(value);
            true
        }),
        "{context} must succeed"
    );
}

fn with_fixture_session(
    html: &str,
    origin: &str,
    action: impl FnOnce(super::StaticHtmlRuntimeSession),
) {
    with_success(
        HtmlBrowserSource::new(html, origin),
        "fixture source",
        |source| {
            with_success(
                StaticHtmlRuntime.start_interactive(&source),
                "fixture session",
                action,
            );
        },
    );
}

#[test]
fn state_observation_reads_actual_dom_and_rejects_script_errors() {
    with_fixture_session(
        "<p id=state>before</p><script>document.getElementById('state').textContent='after'</script>",
        "https://example.test/fixture.html",
        |mut session| {
            with_success(
                observe_state(
                    &mut session,
                    "({value:document.getElementById('state').textContent})",
                ),
                "updated DOM observation",
                |state| assert_eq!(state, json!({"value":"after"})),
            );
            assert!(observe_state(&mut session, "missingVariable.value").is_err());
        },
    );
}

#[test]
fn data_script_blocks_are_preserved_without_execution_or_external_loading() {
    let html = "<script type='application/json' src='missing-private.json'>{\"value\":1}</script><script type='TEXT/JAVASCRIPT'>document.body.dataset.ready='yes'</script><script type='text/javascript;charset=utf-8'>invalid javascript:</script>";
    with_success(
        HtmlBrowserSource::new(html, "https://example.test/fixture.html"),
        "fixture source",
        |source| {
            for interactive in [false, true] {
                let result = if interactive {
                    StaticHtmlRuntime.start_interactive(&source)
                } else {
                    StaticHtmlRuntime.start(html)
                };
                with_success(result, "data script session", |mut session| {
                    assert_data_script_state(&mut session);
                });
            }
        },
    );
}

fn assert_data_script_state(session: &mut super::StaticHtmlRuntimeSession) {
    with_success(
        observe_state(session, "document.body.dataset.ready"),
        "executed script state",
        |state| {
            assert_eq!(state, json!("yes"));
        },
    );
    with_success(
        observe_state(
            session,
            "document.querySelector('script').getAttribute('src')",
        ),
        "unloaded data script source",
        |state| assert_eq!(state, json!("missing-private.json")),
    );
}

#[test]
fn append_preserves_node_identity_and_literal_text_in_argument_order() {
    with_fixture_session(
        "<div id=root></div><script>const root=document.getElementById('root');const child=document.createElement('b');child.textContent='node';root.append('<literal>',child,null,12);</script>",
        "https://example.test/append.html",
        |mut session| {
            with_success(
                observe_state(
                    &mut session,
                    "({text:root.textContent,identity:root.querySelector('b')===child,html:root.innerHTML})",
                ),
                "append DOM observation",
                |state| {
                    assert_eq!(
                        state,
                        json!({"text":"<literal>nodenull12","identity":true,"html":"&lt;literal&gt;<b>node</b>null12"})
                    )
                },
            );
        },
    );
}
