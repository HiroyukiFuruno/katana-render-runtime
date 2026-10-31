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
        INPUTS[REALM_SOURCE_INDEX],
        REALM_STATEMENT_MUTATIONS,
    )?;
    Ok(())
}
