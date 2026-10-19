use super::*;

const BUNDLE_EXECUTION_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "            DiagramRuntimeScript::owned(\"mathjax-runtime.min.js\", runtime_source),\n            DiagramRuntimeScript::owned(\n                \"render-mathjax.js\",\n                format!(\"katanaRunMathJaxRuntime({request_json});\"),\n            ),",
        "            DiagramRuntimeScript::owned(\n                \"render-mathjax.js\",\n                format!(\"katanaRunMathJaxRuntime({request_json});\"),\n            ),\n            DiagramRuntimeScript::owned(\"mathjax-runtime.min.js\", runtime_source),",
        "script-order:name",
    ),
    (
        "katanaRunMathJaxRuntime({request_json});",
        "otherGlobal({request_json});",
        "global-invocation:template",
    ),
    (
        "katanaRunMathJaxRuntime({request_json});",
        "katanaRunMathJaxRuntime();",
        "global-invocation:template",
    ),
];

const BUNDLE_STRUCTURE_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "const MATHJAX_RUNTIME: &str = include_str!(\"../diagram_runtime/generated/mathjax-runtime.min.js\");",
        "const MATHJAX_RUNTIME: &str = \"not-generated\";",
        "include-str-generated:shape",
    ),
    (
        "include_str!(\"../diagram_runtime/generated/mathjax-runtime.min.js\")",
        "include_bytes!(\"../diagram_runtime/generated/mathjax-runtime.min.js\")",
        "include-str-generated:path",
    ),
    (
        "MATHJAX_RUNTIME\n    }",
        "format!(\"{MATHJAX_RUNTIME}\")\n    }",
        "include-str-generated:source-function",
    ),
    (
        "        vec![\n            DiagramRuntimeScript::owned(\"mathjax-runtime.min.js\", runtime_source),",
        "        vec![\n            DiagramRuntimeScript::owned(\"mathjax-runtime.min.js\", runtime_source),\n            DiagramRuntimeScript::owned(\"extra.js\", runtime_source),",
        "script-order:count",
    ),
];

#[test]
fn mathjax_consumer_facts_ast_linter_rejects_bundle_order_and_invocation_drift()
-> Result<(), Box<dyn std::error::Error>> {
    let repository = repository_root()?;
    let fixture = mathjax_fixture()?;
    assert_eq!(inspect_root(fixture.root())?["status"], "verified");
    reject_bundle_execution_contract_changes(&fixture, &repository)?;
    assert_eq!(inspect_root(fixture.root())?["status"], "verified");
    Ok(())
}

fn reject_bundle_execution_contract_changes(
    fixture: &Fixture,
    repository: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let scripts = RUST_INPUTS[MATHJAX_JS_SCRIPTS_INDEX];
    let original = repository.join(scripts);
    for &(before, after, expected) in BUNDLE_EXECUTION_MUTATIONS
        .iter()
        .chain(BUNDLE_STRUCTURE_MUTATIONS.iter())
    {
        fixture.replace(scripts, before, after)?;
        fixture.reject(expected)?;
        fixture.restore_from(scripts, &original)?;
    }
    Ok(())
}
