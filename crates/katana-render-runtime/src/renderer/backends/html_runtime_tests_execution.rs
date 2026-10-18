use super::{HTML_RUNTIME_HOST_BINDING_SOURCE, render};
use crate::renderer::backends::{HtmlRenderInput, HtmlRenderer, HtmlRuntimeError};

#[test]
fn rejects_execution_timeouts_without_a_partial_snapshot() {
    let timeout = HtmlRenderer.render(&HtmlRenderInput {
        source: "<p>visible</p><script>for (;;) {}</script>".to_string(),
    });

    assert!(matches!(timeout, Err(HtmlRuntimeError::ExecutionTimeout)));
}

#[test]
fn reports_javascript_compile_errors() {
    let result = render("<p>visible</p><script>const = ;</script>");
    for expected in ["JavaScript compile error", "inline-script:1:", "const = ;"] {
        assert!(
            matches!(&result, Err(error) if error.contains(expected)),
            "{result:?}"
        );
    }
}

#[test]
fn keeps_unrestricted_host_io_capabilities_unavailable_to_html_scripts() {
    let output = render(
        "<p>visible</p><script>for (const capability of ['fetch', 'WebSocket', 'require', 'process', 'Deno', 'Bun']) { if (globalThis[capability] !== undefined) throw new Error(capability); }</script>",
    );

    assert_eq!(output.as_deref(), Ok("<p>visible</p>"));
    for binding in ["fetch", "WebSocket", "require", "process", "Deno", "Bun"] {
        assert!(
            !contains_host_binding_identifier(HTML_RUNTIME_HOST_BINDING_SOURCE, binding),
            "HTML runtime source exposes host binding: {binding}"
        );
    }
}

fn contains_host_binding_identifier(source: &str, binding: &str) -> bool {
    /* WHY: DOMの要件を示すrequires等の文言を、requireのホスト公開と混同しない。 */
    source
        .split(|character: char| {
            !character.is_alphanumeric() && character != '_' && character != '$'
        })
        .any(|identifier| identifier == binding)
}

#[test]
fn host_binding_source_check_uses_complete_identifiers() {
    for source in ["requires", "required", "_require", "$require", "require_"] {
        assert!(!contains_host_binding_identifier(source, "require"));
    }
    for source in ["require", "globalThis.require", "'require'", "require()"] {
        assert!(contains_host_binding_identifier(source, "require"));
    }
}

#[test]
fn static_exporter_exposes_no_dynamic_request_transport() {
    let output = render(
        "<p id=status>pending</p><script>\
         const xhr=new XMLHttpRequest();\
         xhr.open('GET','state.txt');\
         xhr.send();\
         if(xhr.status===0 && xhr.statusText.includes('interactive session'))\
           document.getElementById('status').textContent='blocked';\
         </script>",
    );

    assert_eq!(output.as_deref(), Ok("<p id=\"status\">blocked</p>"));
}

#[test]
fn formats_each_html_runtime_error() {
    assert_eq!(
        HtmlRuntimeError::ExternalScript("app.js".to_string()).to_string(),
        "external script is not supported: app.js"
    );
    assert_eq!(
        HtmlRuntimeError::JavaScriptCompile("invalid syntax".to_string()).to_string(),
        "JavaScript compile error: invalid syntax"
    );
    assert_eq!(
        HtmlRuntimeError::JavaScriptException("boom".to_string()).to_string(),
        "JavaScript exception: boom"
    );
    assert_eq!(
        HtmlRuntimeError::DomBridge("invalid node".to_string()).to_string(),
        "HTML DOM bridge error: invalid node"
    );
    assert_eq!(
        HtmlRuntimeError::ExecutionTimeout.to_string(),
        "JavaScript execution timed out"
    );
}
