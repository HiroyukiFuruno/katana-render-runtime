use super::{Fixture, INPUTS, REALM_SOURCE_INDEX};

const REALM_BOUNDARY_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "Self::ensure_initialized();",
        "let _initialized = Self::ensure_initialized();",
        "mermaid-classic-realm:initialize",
    ),
    (
        "let context = v8::Context::new(handle_scope, Default::default());\n        let context_scope = &mut v8::ContextScope::new(handle_scope, context);\n        v8::tc_scope!(let scope, &mut **context_scope);\n        diagram_font_bridge::install(scope)?;",
        "let context = v8::Context::new(handle_scope, Default::default());\n        let context_scope = v8::ContextScope::new(handle_scope, context);\n        v8::tc_scope!(let scope, &mut **context_scope);\n        diagram_font_bridge::install(scope)?;",
        "mermaid-classic-realm:context-scope",
    ),
    (
        "diagram_font_bridge::install(scope)?;",
        "let _installed = diagram_font_bridge::install(scope)?;",
        "mermaid-classic-realm:install",
    ),
    (
        "for script in scripts {",
        "while true {",
        "mermaid-classic-realm:loop",
    ),
    (
        "rendered = evaluate(scope, script)?;",
        "other_rendered = evaluate(scope, script)?;",
        "mermaid-classic-realm:evaluate",
    ),
    (
        "        Self {\n            name,\n            code: Cow::Borrowed(code),\n        }",
        "        Self::new(name, code)",
        "mermaid-classic-realm:constructor",
    ),
    (
        "code: Cow::Borrowed(code),",
        "code: Cow::Borrowed(code),\n            ..Default::default()",
        "mermaid-classic-realm:constructor",
    ),
];

#[test]
fn actual_realm_source_rejects_uncovered_statement_shapes() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = Fixture::copy_mermaid_classic_inputs(INPUTS)?;
    fixture.reject_unique_mutations(INPUTS[REALM_SOURCE_INDEX], REALM_BOUNDARY_MUTATIONS)
}
