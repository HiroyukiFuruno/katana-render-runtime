use super::*;

const SCRIPT_CODE_STATEMENT_INDEX: usize = 0;
const SOURCE_ALLOCATION_STATEMENT_INDEX: usize = 1;

type Rejection = super::test_support::SourceRejection;

const ALLOCATION_CASES: &[Rejection] = &[
    (
        "allocation binding changed",
        SOURCE_ALLOCATION_STATEMENT_INDEX,
        "let other = required(v8::String::new(scope, script_code), SOURCE_ALLOC_ERR)?;",
        check_source_allocation,
        "v8-compile-order:allocation",
    ),
    (
        "method is not a call",
        SCRIPT_CODE_STATEMENT_INDEX,
        "let script_code = script.code;",
        check_local_method,
        "v8-compile-order:method:as_ref",
    ),
    (
        "method receiver drift",
        SCRIPT_CODE_STATEMENT_INDEX,
        "let script_code = script.name.as_ref();",
        check_local_method,
        "v8-compile-order:method:as_ref",
    ),
    (
        "method binding drift",
        SCRIPT_CODE_STATEMENT_INDEX,
        "let other = script.code.as_ref();",
        check_local_method,
        "v8-compile-order:binding:script_code",
    ),
    (
        "non-local statement",
        SOURCE_ALLOCATION_STATEMENT_INDEX,
        "script_code;",
        check_source_allocation,
        "route-statement",
    ),
    (
        "required helper missing",
        SOURCE_ALLOCATION_STATEMENT_INDEX,
        "let source = script_code;",
        check_source_allocation,
        "v8-compile-order:required",
    ),
    (
        "required helper changed",
        SOURCE_ALLOCATION_STATEMENT_INDEX,
        "let source = maybe_required(v8::String::new(scope, script_code), SOURCE_ALLOC_ERR)?;",
        check_source_allocation,
        "v8-compile-order:required",
    ),
    (
        "allocation argument not a call",
        SOURCE_ALLOCATION_STATEMENT_INDEX,
        "let source = required(script_code, SOURCE_ALLOC_ERR)?;",
        check_source_allocation,
        "v8-compile-order:allocation",
    ),
];

const EXECUTION_CASES: &[Rejection] = &[
    (
        "compile statement not local",
        COMPILE_STATEMENT_INDEX,
        "compiled;",
        require_compile,
        "route-statement",
    ),
    (
        "compile expression not a method",
        COMPILE_STATEMENT_INDEX,
        "let script = compiled;",
        require_compile,
        "v8-compile-order:compile",
    ),
    (
        "compile callback not closure",
        COMPILE_STATEMENT_INDEX,
        concat!(
            "let script = ScriptCompiler::compile_with_origin(",
            "scope, source, origin_name).ok_or_else(error_value)?;"
        ),
        require_compile,
        "v8-compile-order:compile",
    ),
    (
        "compile callback body not call",
        COMPILE_STATEMENT_INDEX,
        concat!(
            "let script = ScriptCompiler::compile_with_origin(",
            "scope, source, origin_name).ok_or_else(|| || scope)?;"
        ),
        require_compile,
        "v8-compile-order:compile",
    ),
    (
        "compile receiver not a call",
        COMPILE_STATEMENT_INDEX,
        "let script = opaque.ok_or_else(|| exception_message(scope))?;",
        require_compile,
        "v8-compile-order:compile",
    ),
    (
        "compile argument drift",
        COMPILE_STATEMENT_INDEX,
        concat!(
            "let script = ScriptCompiler::compile_with_origin(",
            "scope, source, other_name).ok_or_else(|| exception_message(scope))?;"
        ),
        require_compile,
        "v8-compile-order:compile",
    ),
    (
        "run expression not a method",
        RUN_STATEMENT_INDEX,
        "let value = computed;",
        require_run,
        "v8-compile-order:run",
    ),
    (
        "run receiver not a method",
        RUN_STATEMENT_INDEX,
        "let value = opaque.ok_or_else(|| exception_message(scope))?;",
        require_run,
        "v8-compile-order:run",
    ),
    (
        "run argument drift",
        RUN_STATEMENT_INDEX,
        "let value = script.run(other_scope).ok_or_else(|| exception_message(scope))?;",
        require_run,
        "v8-compile-order:run",
    ),
    (
        "resolve is not tail expression",
        RESOLVE_STATEMENT_INDEX,
        "let resolved = resolve_value(scope, value)?;",
        require_resolve,
        "v8-compile-order:resolve",
    ),
];

const INVALID_CASE: &[Rejection] = &[(
    "invalid statement source",
    0,
    "let invalid = ;",
    require_compile,
    "v8-compile-order:compile",
)];

fn check_local_method(block: &syn::Block) -> Result<(), String> {
    require_local_method(block, 0, "script_code", "as_ref", "script.code")
}

fn check_source_allocation(block: &syn::Block) -> Result<(), String> {
    require_required_allocation(block, 1, "source", "SOURCE_ALLOC_ERR", "script_code")
}

fn assert_rejections(block: &syn::Block, cases: &[Rejection]) -> Result<(), String> {
    let cases = super::test_support::parse_rejections(cases)?;
    for (name, index, statement, validate, expected) in cases {
        let mut candidate = block.clone();
        candidate.stmts[index] = statement;
        assert_eq!(
            validate(&candidate).err().as_deref(),
            Some(expected),
            "{name}"
        );
    }
    Ok(())
}

#[test]
fn v8_route_ast_linter_rejects_malformed_case_source() {
    assert!(super::test_support::parse_rejections(INVALID_CASE).is_err());
}

#[test]
fn v8_route_ast_linter_rejects_allocation_shape_drift() -> Result<(), String> {
    assert_rejections(
        &super::test_support::actual_evaluate_block()?,
        ALLOCATION_CASES,
    )
}

#[test]
fn v8_route_ast_linter_rejects_execution_shape_drift() -> Result<(), String> {
    assert_rejections(
        &super::test_support::actual_evaluate_block()?,
        EXECUTION_CASES,
    )
}

#[test]
fn v8_route_ast_linter_accepts_parenthesized_adapter() -> Result<(), String> {
    let mut block = super::test_support::actual_evaluate_block()?;
    block.stmts[COMPILE_STATEMENT_INDEX] = syn::parse_quote!(
        let script = (ScriptCompiler::compile_with_origin(scope, source, origin_name)
            .ok_or_else(|| exception_message(scope)))?;
    );
    assert_eq!(require_compile(&block), Ok(()));
    Ok(())
}

#[test]
fn v8_route_ast_linter_reports_missing_source_function() -> Result<(), String> {
    let mut sources = super::test_support::actual_sources()?;
    let runtime = sources
        .get_mut(RUST_INPUTS[DIAGRAM_JS_RUNTIME_INPUT_INDEX])
        .ok_or("missing real V8 source")?;
    runtime.ast.items.retain(
        |item| !matches!(item, syn::Item::Fn(function) if function.sig.ident == "evaluate"),
    );
    assert_eq!(
        super::inspect_v8_compile(&sources, &mut Vec::new())
            .err()
            .as_deref(),
        Some("missing-function:evaluate")
    );
    Ok(())
}

#[test]
fn v8_route_ast_linter_reports_source_constant_drift() -> Result<(), String> {
    let mut sources = super::test_support::actual_sources()?;
    let runtime = sources
        .get_mut(RUST_INPUTS[DIAGRAM_JS_RUNTIME_INPUT_INDEX])
        .ok_or("missing real V8 source")?;
    let constant = runtime
        .ast
        .items
        .iter_mut()
        .find_map(|item| match item {
            syn::Item::Const(item) if item.ident == "SOURCE_ALLOC_ERR" => Some(item),
            _ => None,
        })
        .ok_or("missing source allocation error constant")?;
    constant.expr = syn::parse_quote!("wrong allocation error");
    assert_eq!(
        super::inspect_v8_compile(&sources, &mut Vec::new())
            .err()
            .as_deref(),
        Some("v8-compile-order:allocation-error")
    );
    Ok(())
}

#[cfg(test)]
#[path = "v8_route_origin_tests.rs"]
mod origin_helper_tests;

#[test]
fn grouped_method_call_is_unwrapped() -> Result<(), syn::Error> {
    let call = syn::parse_str::<syn::Expr>("script.run(scope)")?;
    let group = syn::Expr::Group(syn::ExprGroup {
        attrs: Vec::new(),
        group_token: Default::default(),
        expr: Box::new(call),
    });
    assert_eq!(
        method_call(&group)
            .map(|value| value.method.to_string())
            .as_deref(),
        Some("run")
    );
    Ok(())
}
