use super::super::consumer::require_direct_call;
use syn::{Block, Expr, Pat, Stmt};

#[cfg(test)]
#[path = "mermaid_classic_capture_unit_tests.rs"]
mod unit_tests;

const REQUEST_CAPTURE_STATEMENTS: usize = 6;
const REQUEST_CAPTURE_EVENT_INDEX: usize = 3;
const LEGACY_REQUEST_STATEMENT_COUNT: usize = 5;
const EVALUATE_CAPTURE_STATEMENTS: usize = 9;
const LEGACY_EVALUATE_STATEMENT_COUNT: usize = 6;
const SCRIPT_CAPTURE_EVENT_INDEX: usize = 1;
const COMPLETION_CAPTURE_EVENT_INDEX: usize = 7;
const RESOLVED_VALUE_INDEX: usize = 6;
const RESULT_STATEMENT_INDEX: usize = 8;
const CAPTURED_EVALUATE_PREFIX_STATEMENTS: usize = 5;

pub(in super::super) fn caller(block: &Block) -> Result<Block, String> {
    match block.stmts.len() {
        LEGACY_REQUEST_STATEMENT_COUNT => Ok(block.clone()),
        REQUEST_CAPTURE_STATEMENTS => {
            event(
                block.stmts.get(REQUEST_CAPTURE_EVENT_INDEX),
                "tracing::event!(target: \"katana_render_runtime::mermaid_classic_capture\", tracing::Level::TRACE, phase = \"request\", request_json = request_json.as_str(), script_count = scripts.len(),);",
                "mermaid-capture:request-event",
            )?;
            let mut projected = block.clone();
            projected.stmts.remove(REQUEST_CAPTURE_EVENT_INDEX);
            Ok(projected)
        }
        _ => Err("mermaid-request-route:statement-count".into()),
    }
}

pub(super) fn evaluate(block: &Block) -> Result<Block, String> {
    match block.stmts.len() {
        LEGACY_EVALUATE_STATEMENT_COUNT => Ok(block.clone()),
        EVALUATE_CAPTURE_STATEMENTS => captured_evaluate(block),
        _ => Err("v8-compile-order:statement-count".into()),
    }
}

fn captured_evaluate(block: &Block) -> Result<Block, String> {
    event(
        block.stmts.get(SCRIPT_CAPTURE_EVENT_INDEX),
        "tracing::event!(target: \"katana_render_runtime::mermaid_classic_capture\", tracing::Level::TRACE, phase = \"script\", name = script.name, code = script_code,);",
        "mermaid-capture:script-event",
    )?;
    event(
        block.stmts.get(COMPLETION_CAPTURE_EVENT_INDEX),
        "tracing::event!(target: \"katana_render_runtime::mermaid_classic_capture\", tracing::Level::TRACE, phase = \"complete\",);",
        "mermaid-capture:completion-event",
    )?;
    let resolved = resolved_value(block.stmts.get(RESOLVED_VALUE_INDEX))?;
    let Some(Stmt::Expr(result, None)) = block.stmts.get(RESULT_STATEMENT_INDEX) else {
        return Err("mermaid-capture:result".into());
    };
    require_direct_call(result, "Ok", &["resolved"], "mermaid-capture:result")?;
    let mut projected = block.clone();
    projected.stmts.remove(1);
    projected
        .stmts
        .truncate(CAPTURED_EVALUATE_PREFIX_STATEMENTS);
    projected.stmts.push(Stmt::Expr(resolved, None));
    Ok(projected)
}

fn resolved_value(statement: Option<&Stmt>) -> Result<Expr, String> {
    let Some(Stmt::Local(local)) = statement else {
        return Err("mermaid-capture:resolve-binding".into());
    };
    if !matches!(&local.pat, Pat::Ident(binding) if binding.ident == "resolved"
        && binding.subpat.is_none() && binding.by_ref.is_none() && binding.mutability.is_none())
    {
        return Err("mermaid-capture:resolve-binding".into());
    }
    let init = local
        .init
        .as_ref()
        .ok_or("mermaid-capture:resolve-binding")?;
    if init.diverge.is_some() {
        return Err("mermaid-capture:resolve-binding".into());
    }
    let Expr::Try(resolved) = init.expr.as_ref() else {
        return Err("mermaid-capture:resolve-success".into());
    };
    require_direct_call(
        &resolved.expr,
        "resolve_value",
        &["scope", "value"],
        "mermaid-capture:resolve",
    )?;
    Ok(*resolved.expr.clone())
}

fn event(statement: Option<&Stmt>, template: &str, rejection: &str) -> Result<(), String> {
    let Some(Stmt::Macro(actual)) = statement else {
        return Err(rejection.into());
    };
    let Stmt::Macro(expected) =
        syn::parse_str::<Stmt>(template).map_err(|_| "mermaid-capture:invalid-template")?
    else {
        return Err("mermaid-capture:invalid-template".into());
    };
    if super::super::source::path_name(&actual.mac.path)
        != super::super::source::path_name(&expected.mac.path)
        || actual.mac.tokens.to_string() != expected.mac.tokens.to_string()
        || actual.semi_token.is_none()
    {
        return Err(rejection.into());
    }
    Ok(())
}
