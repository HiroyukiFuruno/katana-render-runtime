use super::{caller, evaluate, event, resolved_value};
use syn::{Block, Expr, Stmt};

const LEGACY_REQUEST_STATEMENTS: usize = 6;
const SHORT_REQUEST_STATEMENTS: usize = 5;
const LONG_REQUEST_STATEMENTS: usize = 8;
const LEGACY_EVALUATE_STATEMENTS: usize = 6;
const SHORT_EVALUATE_STATEMENTS: usize = 5;
const LONG_EVALUATE_STATEMENTS: usize = 10;
const STATEMENT_COUNT_ERROR: &str = "mermaid-request-route:statement-count";
const EVALUATE_COUNT_ERROR: &str = "v8-compile-order:statement-count";
const RESOLVE_BINDING_ERROR: &str = "mermaid-capture:resolve-binding";
const RESOLVE_SUCCESS_ERROR: &str = "mermaid-capture:resolve-success";
const RESULT_SHAPE_ERROR: &str = "mermaid-capture:result";
const REQUEST_EVENT: &str = "tracing::event!(target: \"katana_render_runtime::mermaid_classic_capture\", tracing::Level::TRACE, phase = \"request\", request_json = request_json.as_str(), script_count = scripts.len(),);";
const REQUEST_EVENT_REJECTION: &str = "mermaid-capture:request-event";
const SCRIPT_EVENT: &str = "tracing::event!(target: \"katana_render_runtime::mermaid_classic_capture\", tracing::Level::TRACE, phase = \"script\", name = script.name, code = script_code,);";
const COMPLETE_EVENT: &str = "tracing::event!(target: \"katana_render_runtime::mermaid_classic_capture\", tracing::Level::TRACE, phase = \"complete\",);";
const RESOLVED_CASES: &[(&str, &str, Option<&str>)] = &[
    (
        "valid resolver",
        "let resolved = resolve_value(scope, value)?;",
        None,
    ),
    (
        "wrong binding name",
        "let other = resolve_value(scope, value)?;",
        Some(RESOLVE_BINDING_ERROR),
    ),
    (
        "missing initializer",
        "let resolved;",
        Some(RESOLVE_BINDING_ERROR),
    ),
    (
        "diverging initializer",
        "let resolved = resolve_value(scope, value)? else { return; };",
        Some(RESOLVE_BINDING_ERROR),
    ),
    (
        "initializer without try",
        "let resolved = resolve_value(scope, value);",
        Some(RESOLVE_SUCCESS_ERROR),
    ),
];

#[test]
fn caller_accepts_legacy_statement_shape_and_rejects_adjacent_counts() -> Result<(), syn::Error> {
    for (case, count, expected) in [
        (
            "legacy request",
            LEGACY_REQUEST_STATEMENTS,
            Ok(LEGACY_REQUEST_STATEMENTS),
        ),
        (
            "request below legacy count",
            SHORT_REQUEST_STATEMENTS,
            Err(STATEMENT_COUNT_ERROR.to_string()),
        ),
        (
            "request above capture count",
            LONG_REQUEST_STATEMENTS,
            Err(STATEMENT_COUNT_ERROR.to_string()),
        ),
    ] {
        let block = statement_block(count)?;
        assert_eq!(
            caller(&block).map(|value| value.stmts.len()),
            expected,
            "{case}"
        );
    }
    Ok(())
}

#[test]
fn evaluate_accepts_legacy_statement_shape_and_rejects_adjacent_counts() -> Result<(), syn::Error> {
    for (case, count, expected) in [
        (
            "legacy evaluate",
            LEGACY_EVALUATE_STATEMENTS,
            Ok(LEGACY_EVALUATE_STATEMENTS),
        ),
        (
            "evaluate below legacy count",
            SHORT_EVALUATE_STATEMENTS,
            Err(EVALUATE_COUNT_ERROR.to_string()),
        ),
        (
            "evaluate above capture count",
            LONG_EVALUATE_STATEMENTS,
            Err(EVALUATE_COUNT_ERROR.to_string()),
        ),
    ] {
        let block = statement_block(count)?;
        assert_eq!(
            evaluate(&block).map(|value| value.stmts.len()),
            expected,
            "{case}"
        );
    }
    Ok(())
}

#[test]
fn evaluate_rejects_non_expression_result_after_captured_events() -> Result<(), syn::Error> {
    let source = format!(
        "{{ let first = true; {SCRIPT_EVENT} let second = true; let third = true; let fourth = true; let fifth = true; let resolved = resolve_value(scope, value)?; {COMPLETE_EVENT} let result = true; }}"
    );
    let block = syn::parse_str::<Block>(&source)?;
    assert_eq!(evaluate(&block).err().as_deref(), Some(RESULT_SHAPE_ERROR));
    Ok(())
}

#[test]
fn resolved_value_checks_local_binding_initializer_and_try_shape() -> Result<(), syn::Error> {
    assert_eq!(
        resolved_value(None).err().as_deref(),
        Some(RESOLVE_BINDING_ERROR)
    );
    for (case, statement, expected_error) in RESOLVED_CASES {
        assert_resolved_case(case, statement, *expected_error)?;
    }
    Ok(())
}

fn assert_resolved_case(
    case: &str,
    statement: &str,
    expected_error: Option<&str>,
) -> Result<(), syn::Error> {
    let block = syn::parse_str::<Block>(&format!("{{ {statement} }}"))?;
    let result = resolved_value(block.stmts.first());
    if let Some(expected) = expected_error {
        assert_eq!(result.err().as_deref(), Some(expected), "{case}");
    } else {
        assert!(
            matches!(&result, Ok(expression) if is_resolver_call(expression)),
            "{case}"
        );
    }
    Ok(())
}

fn is_resolver_call(expression: &Expr) -> bool {
    matches!(expression, Expr::Call(call) if matches!(&*call.func, Expr::Path(path) if path.path.is_ident("resolve_value")) && call.args.len() == 2)
}

#[test]
fn event_rejects_nonmacro_statements_and_parseable_nonmacro_templates() -> Result<(), syn::Error> {
    let ordinary = syn::parse_str::<Stmt>("let marker = true;")?;
    assert_eq!(
        event(Some(&ordinary), REQUEST_EVENT, REQUEST_EVENT_REJECTION),
        Err(REQUEST_EVENT_REJECTION.to_string())
    );

    let actual = syn::parse_str::<Stmt>(REQUEST_EVENT)?;
    assert_eq!(
        event(
            Some(&actual),
            "let expected = true;",
            REQUEST_EVENT_REJECTION
        ),
        Err("mermaid-capture:invalid-template".to_string())
    );
    Ok(())
}

fn statement_block(count: usize) -> Result<Block, syn::Error> {
    let statements = (0..count)
        .map(|index| format!("let value_{index} = {index};"))
        .collect::<Vec<_>>()
        .join(" ");
    syn::parse_str(&format!("{{ {statements} }}"))
}
