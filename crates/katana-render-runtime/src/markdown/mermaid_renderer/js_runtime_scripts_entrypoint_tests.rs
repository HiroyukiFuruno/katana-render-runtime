use super::MermaidRuntimeScripts;

#[test]
fn zenuml_runtime_loads_after_mermaid_zenuml_and_before_render_script() {
    let scripts = MermaidRuntimeScripts::build_with_zenuml(
        "mermaid bundle",
        "globalThis[\"mermaid-zenuml\"] = { id: \"registered\" };",
        r#"{"source":"zenuml\nA.method()","diagramType":"zenuml"}"#,
    );
    let names = script_names(&scripts);

    assert_order(&names, "mermaid-zenuml.min.js", "zenuml-runtime.min.js");
    assert_order(&names, "zenuml-runtime.min.js", "render-mermaid.js");
}

#[test]
fn non_zenuml_runtime_does_not_load_zenuml_runtime_bundle() {
    let scripts = MermaidRuntimeScripts::build_with_zenuml(
        "mermaid bundle",
        "globalThis[\"mermaid-zenuml\"] = { id: \"registered\" };",
        r#"{"source":"graph TD; A-->B","diagramType":""}"#,
    );

    assert!(!script_names(&scripts).contains(&"zenuml-runtime.min.js"));
}

#[test]
fn render_script_calls_only_mermaid_entrypoint() {
    let scripts = MermaidRuntimeScripts::build_with_zenuml(
        "mermaid bundle",
        "globalThis[\"mermaid-zenuml\"] = { id: \"registered\" };",
        r#"{"source":"graph TD; A-->B","diagramType":""}"#,
    );
    let render_script = scripts
        .iter()
        .find(|script| script.name == "render-mermaid.js")
        .map(|script| script.code.as_ref())
        .unwrap_or("");

    assert!(render_script.contains("katanaRunMermaidRuntime("));
    assert!(!render_script.contains("katanaInstallMermaidZenumlRuntimeAdapter"));
}

fn script_names(
    scripts: &[crate::markdown::diagram_js_runtime::DiagramRuntimeScript<'_>],
) -> Vec<&'static str> {
    scripts.iter().map(|script| script.name).collect()
}

fn assert_order(names: &[&str], before: &str, after: &str) {
    let before_index = names.iter().position(|name| *name == before).unwrap_or(0);
    let after_index = names.iter().position(|name| *name == after).unwrap_or(0);
    assert!(
        before_index < after_index,
        "{before} should be loaded before {after}: {names:?}"
    );
}
