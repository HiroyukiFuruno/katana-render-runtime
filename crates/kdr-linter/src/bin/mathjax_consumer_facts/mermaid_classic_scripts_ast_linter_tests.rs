use super::{Fixture, INPUTS, mermaid_classic_scripts, reject_mutations};

const SCRIPT_SOURCE_INDEX: usize = 0;
const REALM_SOURCE_INDEX: usize = 2;

const SCRIPT_STATEMENT_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "scripts.push(DiagramRuntimeScript::owned(",
        "let _ = scripts.push(DiagramRuntimeScript::owned(",
        "mermaid-scripts:push",
    ),
    (
        "scripts.push(DiagramRuntimeScript::owned(",
        "scripts.extend(DiagramRuntimeScript::owned(",
        "mermaid-scripts:push",
    ),
    (
        "let mut scripts = vec![",
        "let scripts = vec![",
        "mermaid-scripts:vector-binding",
    ),
    ("vec![", "format![", "mermaid-scripts:vector"),
    (
        "        if uses_zenuml_runtime {",
        "        while uses_zenuml_runtime {",
        "mermaid-scripts:zen-branch",
    ),
];

const SCRIPT_INCLUDE_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "const MERMAID_RUNTIME: &str = include_str!(\"../diagram_runtime/generated/mermaid-runtime.min.js\");",
        "const MERMAID_RUNTIME: &str = include_str!(\"../diagram_runtime/generated/mermaid-runtime.min.js\");\nconst MERMAID_RUNTIME: &str = include_str!(\"../diagram_runtime/generated/mermaid-runtime.min.js\");",
        "mermaid-scripts:include-count",
    ),
    (
        "include_str!(\"../diagram_runtime/generated/mermaid-runtime.min.js\")",
        "\"../diagram_runtime/generated/mermaid-runtime.min.js\"",
        "mermaid-scripts:include",
    ),
    (
        "concat!(env!(\"OUT_DIR\"), \"/mermaid-zenuml.min.js\")",
        "\"/mermaid-zenuml.min.js\"",
        "mermaid-scripts:plugin",
    ),
    (
        "\"/mermaid-zenuml.min.js\"",
        "\"/unexpected-mermaid-plugin.js\"",
        "mermaid-scripts:plugin",
    ),
    (
        "env!(\"OUT_DIR\")",
        "\"OUT_DIR\"",
        "mermaid-scripts:plugin-env",
    ),
];

const REALM_STATEMENT_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "    pub(crate) fn render(scripts: &[DiagramRuntimeScript<'_>]) -> Result<String, String> {\n        Self::ensure_initialized();\n\n        let mut isolate = v8::Isolate::new(Default::default());\n        v8::scope!(let handle_scope, &mut isolate);",
        "    pub(crate) fn render(scripts: &[DiagramRuntimeScript<'_>]) -> Result<String, String> {\n        Self::ensure_initialized();\n\n        let mut isolate = v8::Isolate::new(Default::default());\n        let scope = ();",
        "mermaid-classic-realm:scope-macro",
    ),
    (
        "        Ok(rendered)\n    }",
        "        let _ = rendered;\n    }",
        "mermaid-classic-route:tail",
    ),
    (
        "pub(crate) fn render(scripts: &[DiagramRuntimeScript<'_>]) -> Result<String, String> {\n        Self::ensure_initialized();\n\n        let mut isolate = v8::Isolate::new(Default::default());\n        v8::scope!(let handle_scope, &mut isolate);\n        let context = v8::Context::new(handle_scope, Default::default());\n        let context_scope = &mut v8::ContextScope::new(handle_scope, context);",
        "pub(crate) fn render(scripts: &[DiagramRuntimeScript<'_>]) -> Result<String, String> {\n        Self::ensure_initialized();\n\n        let mut isolate = v8::Isolate::new(Default::default());\n        v8::scope!(let handle_scope, &mut isolate);\n        let context = v8::Context::new(handle_scope, Default::default());\n        let context_scope = &v8::ContextScope::new(handle_scope, context);",
        "mermaid-classic-realm:context-scope",
    ),
    (
        "diagram_geometry_bridge::install(scope)?;",
        "diagram_geometry_bridge::initialize(scope)?;",
        "mermaid-classic-realm:geometry-install",
    ),
    (
        "let mut rendered = String::new();",
        "let mut rendered = String::with_capacity(0);",
        "mermaid-classic-realm",
    ),
    (
        "for script in scripts {",
        "'render_scripts: for script in scripts {",
        "mermaid-classic-realm:loop",
    ),
    (
        "code: Cow::Borrowed(code),",
        "bytes: Cow::Borrowed(code),",
        "mermaid-classic-realm:constructor",
    ),
];

#[test]
fn actual_source_statement_shapes_are_rejected_by_the_ast_route()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::copy_mermaid_classic_inputs(INPUTS)?;
    mermaid_classic_scripts::inspect_root(fixture.root())?;
    reject_mutations(
        &fixture,
        INPUTS[SCRIPT_SOURCE_INDEX],
        SCRIPT_STATEMENT_MUTATIONS,
    )?;
    reject_mutations(
        &fixture,
        INPUTS[SCRIPT_SOURCE_INDEX],
        SCRIPT_INCLUDE_MUTATIONS,
    )?;
    reject_mutations(
        &fixture,
        INPUTS[REALM_SOURCE_INDEX],
        REALM_STATEMENT_MUTATIONS,
    )?;
    Ok(())
}
