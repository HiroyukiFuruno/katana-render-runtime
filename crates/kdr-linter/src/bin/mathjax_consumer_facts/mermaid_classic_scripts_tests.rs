use super::mermaid_classic_scripts::{self, INPUTS};
use super::test_support::Fixture;
use serde_json::Value;
const SCRIPT_PRODUCER_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "diagram_type == MermaidDiagramType::Zenuml",
        "diagram_type != MermaidDiagramType::Zenuml",
        "mermaid-scripts:forward",
    ),
    (
        "            request_json,\n            diagram_type",
        "            bundle,\n            diagram_type",
        "mermaid-scripts:forward",
    ),
    (
        "if uses_zenuml_runtime {",
        "if false {",
        "mermaid-scripts:zen-branch",
    ),
    (
        "if uses_zenuml_runtime {",
        "if !uses_zenuml_runtime {",
        "mermaid-scripts:zen-branch",
    ),
    (
        "let mut scripts = vec![",
        "return Vec::new();\n        let mut scripts = vec![",
        "mermaid-scripts:body",
    ),
    (
        "\"mermaid-runtime.min.js\", MERMAID_RUNTIME",
        "\"mermaid-runtime.min.js\", ZENUML_RUNTIME",
        "mermaid-scripts:entry-code",
    ),
    (
        "\"mermaid.min.js\", bundle",
        "\"mermaid.min.js\", zenuml_bundle",
        "mermaid-scripts:entry-code",
    ),
    (
        "DiagramRuntimeScript::borrowed(\"mermaid.min.js\", bundle)",
        "DiagramRuntimeScript::owned(\"mermaid.min.js\", bundle)",
        "mermaid-scripts:entry",
    ),
    (
        "\"zenuml-runtime.min.js\",",
        "\"unknown-runtime.min.js\",",
        "mermaid-scripts:entry",
    ),
    (
        "render_script(request_json)",
        "render_script(bundle)",
        "mermaid-scripts:argument",
    ),
    (
        "katanaRunMermaidRuntime({request_json});",
        "katanaRunMermaidRuntime({});",
        "mermaid-scripts:format",
    ),
    (
        "katanaRunMermaidRuntime({request_json});",
        "fakeRuntime({request_json});",
        "mermaid-scripts:format",
    ),
    (
        "env!(\"OUT_DIR\")",
        "env!(\"HOME\")",
        "mermaid-scripts:plugin-env",
    ),
    (
        "../diagram_runtime/generated/mermaid-runtime.min.js",
        "../diagram_runtime/generated/zenuml-runtime.min.js",
        "mermaid-scripts:include-path",
    ),
    (
        "        scripts\n    }",
        "        Vec::new()\n    }",
        "mermaid-scripts:tail",
    ),
];
const LIVE_CALLER_BINDING_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "request.to_json_value().to_string()",
        "request.to_json_value()",
        "read-render-route:request-json",
    ),
    (
        "MermaidRuntimeScripts::build(&bundle, &request_json, request.diagram_type)",
        "MermaidRuntimeScripts::build(&bundle, &bundle, request.diagram_type)",
        "mermaid-request-route:argument",
    ),
    (
        "DiagramV8Runtime::render(&scripts)",
        "DiagramV8Runtime::render(&other_scripts)",
        "mermaid-request-route:argument",
    ),
    (
        "let scripts = MermaidRuntimeScripts::build",
        "return Err(String::new());\n        let scripts = MermaidRuntimeScripts::build",
        "mermaid-request-route:statement-count",
    ),
    (
        "read_mermaid_bundle(mermaid_js)",
        "read_mermaid_bundle(&other_path)",
        "mermaid-request-route:argument",
    ),
    (
        "MermaidRenderRequest::new(source, preset)",
        "MermaidRenderRequest::new({ let source = \"shadow\"; source }, preset)",
        "mermaid-request-route:argument",
    ),
    (
        "let request_json =",
        "let request_json @ shadow =",
        "mermaid-request-route:binding:request_json",
    ),
];

#[test]
fn mermaid_classic_ast_linter_derives_actual_source_manifests()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::copy_mermaid_classic_inputs(INPUTS)?;
    let report = mermaid_classic_scripts::inspect_root(fixture.root())?;
    verify_report_contract(&report)?;
    verify_manifest_order(&report)?;
    assert_eq!(
        report["manifest"]["formatProducer"]["semanticTemplate"],
        "katanaRunMermaidRuntime({request_json});"
    );
    verify_source_identity(&fixture, &report)?;
    assert!(report.get("requestJson").is_none());
    assert!(report.get("projectedJavaScript").is_none());
    Ok(())
}

fn verify_report_contract(report: &Value) -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        report["schema"],
        "kdr-linter/mermaid-classic-script-facts/v1"
    );
    assert_eq!(report["status"], "verified");
    assert_eq!(report["sourceProofVerified"], true);
    let facts = report["facts"].as_array().ok_or("facts array")?;
    assert_eq!(facts.len(), 9);
    let pins = report["inputSha256"].as_object().ok_or("pins")?;
    assert_eq!(pins.len(), INPUTS.len());
    for name in [
        "requestBytesVerified",
        "materializedScriptsVerified",
        "vendorBytesVerified",
        "pluginBytesVerified",
        "nativeExecutionVerified",
        "performanceVerified",
        "acceptedConsumerClosure",
    ] {
        assert_eq!(report[name], false, "unobserved boundary: {name}");
    }
    Ok(())
}

fn verify_manifest_order(report: &Value) -> Result<(), Box<dyn std::error::Error>> {
    let base = [
        "mermaid-runtime.min.js",
        "mermaid.min.js",
        "mermaid-zenuml.min.js",
    ];
    for (mode, extra) in [
        ("normal", &[][..]),
        ("zenuml", &["zenuml-runtime.min.js"][..]),
    ] {
        let names = base
            .iter()
            .copied()
            .chain(extra.iter().copied())
            .chain(["render-mermaid.js"])
            .collect::<Vec<_>>();
        let entries = report["manifest"][mode]
            .as_array()
            .ok_or("script entries")?;
        let actual = entries
            .iter()
            .map(|entry| entry["name"].as_str().ok_or("script name"))
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(actual, names);
    }
    Ok(())
}

fn verify_source_identity(
    fixture: &Fixture,
    report: &Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let facts = report["facts"].as_array().ok_or("facts array")?;
    let identity = facts
        .iter()
        .find(|fact| fact["id"] == "mermaid-classic-source-identities")
        .ok_or("identity fact")?;
    let [start, end] = ["lineStart", "lineEnd"].map(|key| {
        identity["range"][key]
            .as_u64()
            .and_then(|line| usize::try_from(line).ok())
            .ok_or("range line")
    });
    let source = std::fs::read_to_string(fixture.root().join(INPUTS[0]))?;
    let selected = source
        .lines()
        .skip(start? - 1)
        .take(end? - start? + 1)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        selected.contains("const MERMAID_RUNTIME:")
            && selected.contains("const ZENUML_RUNTIME:")
            && selected.contains("const MERMAID_ZENUML_RUNTIME:")
            && !selected.contains("#[cfg(test)]")
    );
    Ok(())
}
#[test]
fn mermaid_classic_ast_linter_rejects_actual_script_producer_mutations()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::copy_mermaid_classic_inputs(INPUTS)?;
    mermaid_classic_scripts::inspect_root(fixture.root())?;
    reject_mutations(&fixture, INPUTS[0], SCRIPT_PRODUCER_MUTATIONS)?;
    Ok(())
}
#[test]
fn mermaid_classic_ast_linter_rejects_live_caller_binding_mutations()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::copy_mermaid_classic_inputs(INPUTS)?;
    mermaid_classic_scripts::inspect_root(fixture.root())?;
    reject_mutations(&fixture, INPUTS[1], LIVE_CALLER_BINDING_MUTATIONS)?;
    Ok(())
}
#[path = "mermaid_classic_capture_tests.rs"]
mod capture;
fn reject_mutations(
    fixture: &Fixture,
    path: &str,
    cases: &[(&str, &str, &str)],
) -> Result<(), Box<dyn std::error::Error>> {
    for (original, replacement, rejection) in cases {
        fixture.replace(path, original, replacement)?;
        fixture.reject(rejection)?;
        fixture.restore(path)?;
        mermaid_classic_scripts::inspect_root(fixture.root())?;
    }
    Ok(())
}
