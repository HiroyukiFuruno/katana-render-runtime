use super::{Fixture, INPUTS, mermaid_classic_scripts, reject_mutations};

const REQUEST_CAPTURE_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "request_json = request_json.as_str()",
        "request_json = bundle.as_ref()",
        "mermaid-capture:request-event",
    ),
    (
        "script_count = scripts.len()",
        "script_count = 4",
        "mermaid-capture:request-event",
    ),
    (
        "phase = \"request\"",
        "phase = \"script\"",
        "mermaid-capture:request-event",
    ),
    (
        "tracing::Level::TRACE",
        "tracing::Level::INFO",
        "mermaid-capture:request-event",
    ),
    (
        "target: \"katana_render_runtime::mermaid_classic_capture\"",
        "target: \"unknown\"",
        "mermaid-capture:request-event",
    ),
    (
        "phase = \"request\",",
        "phase = \"request\", unknown = \"field\",",
        "mermaid-capture:request-event",
    ),
    (
        "tracing::event!(",
        "unknown::event!(",
        "mermaid-capture:request-event",
    ),
];
const EVALUATION_CAPTURE_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "code = script_code",
        "code = \"synthetic\"",
        "mermaid-capture:script-event",
    ),
    (
        "name = script.name",
        "name = \"other.js\"",
        "mermaid-capture:script-event",
    ),
    (
        "phase = \"complete\"",
        "phase = \"script\"",
        "mermaid-capture:completion-event",
    ),
    (
        "phase = \"complete\",",
        "phase = \"complete\", error = \"ignored\",",
        "mermaid-capture:completion-event",
    ),
    (
        "let resolved = resolve_value(scope, value)?;",
        "let resolved = resolve_value(scope, value);",
        "mermaid-capture:resolve-success",
    ),
    (
        "let resolved = resolve_value(scope, value)?;",
        "let resolved = resolve_value(other_scope, value)?;",
        "mermaid-capture:resolve",
    ),
    (
        "    Ok(resolved)",
        "    Ok(String::new())",
        "mermaid-capture:result",
    ),
    (
        "let resolved = resolve_value(scope, value)?;",
        "return Ok(String::new());\n    let resolved = resolve_value(scope, value)?;",
        "v8-compile-order:statement-count",
    ),
];
const SHARED_REALM_COMPILER_MUTATIONS: &[(&str, &str, &str)] = &[
    (
        "for script in scripts {",
        "for script in scripts.iter().rev() {",
        "mermaid-classic-realm:loop",
    ),
    (
        "rendered = evaluate(scope, script)?;",
        "if false { rendered = evaluate(scope, script)?; }",
        "mermaid-classic-realm:evaluate",
    ),
    (
        "rendered = evaluate(scope, script)?;",
        "rendered = evaluate(other_scope, script)?;",
        "mermaid-classic-realm:argument",
    ),
    (
        "let mut rendered = String::new();",
        "return Ok(String::new());\n        let mut rendered = String::new();",
        "mermaid-classic-realm:statement-count",
    ),
    (
        "v8::tc_scope!(let scope, &mut **context_scope);\n        diagram_font_bridge::install(scope)?;",
        "v8::tc_scope!(let scope, &mut **other_scope);\n        diagram_font_bridge::install(scope)?;",
        "mermaid-classic-realm:scope-macro",
    ),
    (
        "let context_scope = &mut v8::ContextScope::new(handle_scope, context);\n        v8::tc_scope!(let scope, &mut **context_scope);\n        diagram_font_bridge::install(scope)?;",
        "let context_scope = &mut v8::ContextScope::new(handle_scope, other_context);\n        v8::tc_scope!(let scope, &mut **context_scope);\n        diagram_font_bridge::install(scope)?;",
        "mermaid-classic-realm:argument",
    ),
    (
        "code: Cow::Borrowed(code)",
        "code: Cow::Borrowed(\"fake\")",
        "mermaid-classic-realm:argument",
    ),
    (
        "code: Cow::Owned(code)",
        "code: Cow::Borrowed(code)",
        "mermaid-classic-realm:callee",
    ),
    (
        "ScriptCompiler::compile_with_origin(scope, source, origin_name)",
        "ScriptCompiler::compile_with_origin(scope, other_source, origin_name)",
        "v8-compile-order",
    ),
];

#[test]
fn mermaid_classic_ast_linter_rejects_request_capture_mutations()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::copy_mermaid_classic_inputs(INPUTS)?;
    mermaid_classic_scripts::inspect_root(fixture.root())?;
    reject_mutations(&fixture, INPUTS[1], REQUEST_CAPTURE_MUTATIONS)?;
    Ok(())
}

#[test]
fn mermaid_classic_ast_linter_rejects_evaluation_capture_mutations()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::copy_mermaid_classic_inputs(INPUTS)?;
    mermaid_classic_scripts::inspect_root(fixture.root())?;
    reject_mutations(&fixture, INPUTS[2], EVALUATION_CAPTURE_MUTATIONS)?;
    Ok(())
}

#[test]
fn mermaid_classic_ast_linter_rejects_shared_realm_and_compiler_mutations()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::copy_mermaid_classic_inputs(INPUTS)?;
    mermaid_classic_scripts::inspect_root(fixture.root())?;
    reject_mutations(&fixture, INPUTS[2], SHARED_REALM_COMPILER_MUTATIONS)?;
    reject_mutations(
        &fixture,
        INPUTS[4],
        &[(
            "mod v8_script;",
            "pub mod v8_script;",
            "v8-script-origin-helper:module-registration",
        )],
    )?;
    Ok(())
}
