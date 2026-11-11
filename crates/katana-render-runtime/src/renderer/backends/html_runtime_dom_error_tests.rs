use super::{HtmlRenderInput, HtmlRenderer, HtmlRuntimeError};

const MISSING_INITIAL_ARGUMENT_OPERATIONS: &[&str] = &[
    "__krr_dom('getElementById')",
    "__krr_dom('querySelector')",
    "__krr_dom('createElement')",
    "__krr_dom('textContent')",
    "__krr_dom('textContent', 'not-a-node')",
    "__krr_dom('innerHTML')",
    "__krr_dom('getAttribute')",
    "__krr_dom('appendChild')",
    "__krr_dom('remove')",
    "__krr_dom('setTextContent')",
    "__krr_dom('setInnerHTML')",
    "__krr_dom('setAttribute')",
    "__krr_dom('removeAttribute')",
    "__krr_dom('styleGet')",
    "__krr_dom('styleSet')",
];
const MISSING_FOLLOW_UP_ARGUMENT_OPERATIONS: &[&str] = &[
    "const target = document.getElementById('target'); __krr_dom('getAttribute', target.__krrNodeId)",
    "const target = document.getElementById('target'); __krr_dom('appendChild', target.__krrNodeId)",
    "const target = document.getElementById('target'); __krr_dom('setTextContent', target.__krrNodeId)",
    "const target = document.getElementById('target'); __krr_dom('setInnerHTML', target.__krrNodeId)",
    "const target = document.getElementById('target'); __krr_dom('setAttribute', target.__krrNodeId)",
    "const target = document.getElementById('target'); __krr_dom('setAttribute', target.__krrNodeId, 'data-state')",
    "const target = document.getElementById('target'); __krr_dom('removeAttribute', target.__krrNodeId)",
    "const target = document.getElementById('target'); __krr_dom('styleGet', target.__krrNodeId)",
    "const target = document.getElementById('target'); __krr_dom('styleSet', target.__krrNodeId)",
    "const target = document.getElementById('target'); __krr_dom('styleSet', target.__krrNodeId, 'color')",
];

#[test]
fn v8_dom_bridge_rejects_missing_arguments_and_invalid_node_ids() {
    assert_dom_bridge_errors(MISSING_INITIAL_ARGUMENT_OPERATIONS);
    assert_dom_bridge_errors(MISSING_FOLLOW_UP_ARGUMENT_OPERATIONS);
}

fn assert_dom_bridge_errors(operations: &[&str]) {
    for operation in operations {
        let output = HtmlRenderer.render(&HtmlRenderInput {
            source: format!("<p id=target>Visible</p><script>{operation}</script>"),
        });

        assert!(
            matches!(output, Err(HtmlRuntimeError::DomBridge(_))),
            "operation must be rejected: {operation}"
        );
    }
}

const MUTATION_FAILURE_CASES: &[(&str, &str)] = &[
    (
        "document.createElement('bad name')",
        "unsupported element name: bad name",
    ),
    (
        "document.createElement('div').insertAdjacentText('beforebegin', 'x')",
        "insertAdjacentHTML beforebegin requires a parent",
    ),
    (
        "__krr_dom('setTextContent', '999999', 'x')",
        "HTML node 999999 does not exist",
    ),
    (
        "__krr_dom('appendChild', document.getElementById('target').__krrNodeId, '999999')",
        "HTML node 999999 does not exist",
    ),
];

#[test]
fn javascript_dom_mutations_propagate_native_failures() {
    for &(operation, expected) in MUTATION_FAILURE_CASES {
        let output = HtmlRenderer.render(&HtmlRenderInput {
            source: format!("<p id=target>Visible</p><script>{operation}</script>"),
        });
        assert!(
            matches!(output, Err(HtmlRuntimeError::DomBridge(ref message)) if message == expected),
            "unexpected result for {operation}: {output:?}"
        );
    }
}
