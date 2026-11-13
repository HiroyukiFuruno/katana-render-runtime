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

#[test]
fn timer_callbacks_observe_promise_microtasks_in_both_pumps() -> Result<(), String> {
    use crate::renderer::backends::html_browser::HtmlBrowserSource;

    let source = HtmlBrowserSource::new(
        r#"<p id="state"></p><script>
          const state = document.getElementById('state');
          let ready = false;
          setTimeout(() => { state.textContent += 'A'; Promise.resolve().then(() => { ready = true; state.textContent += 'B'; }); }, 0);
          setTimeout(() => { state.textContent += ready ? 'C' : 'X'; }, 0);
          setTimeout(() => {
            state.textContent += 'D'; ready = false;
            Promise.resolve().then(() => { ready = true; state.textContent += 'E'; });
            setTimeout(() => { state.textContent += ready ? 'G' : 'X'; }, 0);
          }, 1);
          setTimeout(() => { state.textContent += ready ? 'F' : 'X'; }, 1);
        </script>"#,
        "https://example.test/timers.html",
    ).map_err(|error| error.to_string())?;
    let mut session = StaticHtmlRuntime
        .start_interactive(&source)
        .map_err(test_error)?;
    let initial = session.snapshot().map_err(test_error)?;
    assert!(initial.contains(">ABC</p>"), "{initial}");
    assert!(session.advance_timers(1.0).map_err(test_error)?);
    let advanced = session.snapshot().map_err(test_error)?;
    assert!(advanced.contains(">ABCDEFG</p>"), "{advanced}");
    Ok(())
}

fn test_error(error: super::HtmlRuntimeError) -> String {
    error.to_string()
}

#[test]
fn timer_microtasks_share_the_execution_budget() -> Result<(), String> {
    let mut session = StaticHtmlRuntime
        .start(
            "<script>setTimeout(() => Promise.resolve().then(() => { for (;;) {} }), 1);</script>",
        )
        .map_err(test_error)?;
    assert!(matches!(
        session.advance_timers(1.0),
        Err(super::HtmlRuntimeError::ExecutionTimeout)
    ));
    Ok(())
}

#[test]
fn test_error_preserves_real_runtime_failure() -> Result<(), String> {
    let mut session = StaticHtmlRuntime
        .start("<script>setTimeout(() => { throw new Error('typed-runtime-error') }, 1)</script>")
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
