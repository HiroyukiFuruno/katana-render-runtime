use super::super::{MATHJAX_JS_RUNTIME_INDEX, RUST_INPUTS, mathjax_fixture};

const LOCAL_BINDING_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "let request = MathJaxRenderRequest::new(source, preset, display);",
        "let other_request = MathJaxRenderRequest::new(source, preset, display);",
        "route-binding:request",
    ),
    (
        "let scripts = MathJaxRuntimeScripts::build(mathjax_source, &request_json);",
        "let other_scripts = MathJaxRuntimeScripts::build(mathjax_source, &request_json);",
        "route-binding:scripts",
    ),
    (
        "let output = DiagramV8Runtime::render(&scripts)?;",
        "let other_output = DiagramV8Runtime::render(&scripts)?;",
        "route-binding:output",
    ),
];

#[test]
fn actual_mathjax_consumer_rejects_local_binding_name_drift()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = mathjax_fixture()?;
    fixture.reject_unique_mutations(
        RUST_INPUTS[MATHJAX_JS_RUNTIME_INDEX],
        LOCAL_BINDING_MUTATIONS,
    )
}
