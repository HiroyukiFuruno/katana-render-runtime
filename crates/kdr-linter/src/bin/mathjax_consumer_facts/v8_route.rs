use super::consumer::{local, require_direct_call};
use super::source::{RUST_INPUTS, Source, call, ident, record_fact};
use serde_json::Value;
use std::collections::BTreeMap;
use syn::spanned::Spanned;

#[path = "mermaid_classic_capture.rs"]
pub(super) mod capture;

const V8_STATEMENTS: usize = 6;
const DIAGRAM_JS_RUNTIME_INPUT_INDEX: usize = 5;
const COMPILE_STATEMENT_INDEX: usize = 3;
const RUN_STATEMENT_INDEX: usize = 4;
const RESOLVE_STATEMENT_INDEX: usize = 5;
const COMPILE_ARGUMENT_COUNT: usize = 3;

pub(super) fn inspect_v8_compile(
    sources: &BTreeMap<&str, Source>,
    facts: &mut Vec<Value>,
) -> Result<(), String> {
    let path = RUST_INPUTS[DIAGRAM_JS_RUNTIME_INPUT_INDEX];
    let source = &sources[path];
    let evaluate = super::source::free_function(&source.ast, "evaluate")?;
    let projected = capture::evaluate(&evaluate.block)?;
    let block = &projected;
    if block.stmts.len() != V8_STATEMENTS {
        return Err("v8-compile-order:statement-count".into());
    }
    require_local_method(block, 0, "script_code", "as_ref", "script.code")?;
    require_required_allocation(block, 1, "source", "SOURCE_ALLOC_ERR", "script_code")?;
    require_required_allocation(block, 2, "origin_name", "FILENAME_ALLOC_ERR", "script.name")?;
    require_compile(block)?;
    require_run(block)?;
    require_resolve(block)?;
    super::v8_compile::require_error_constants(
        &source.ast,
        &[
            ("SOURCE_ALLOC_ERR", "source allocation failed"),
            ("FILENAME_ALLOC_ERR", "filename allocation failed"),
        ],
    )?;
    record_fact(facts, "v8-compile-order", path, evaluate.span());
    super::v8_compile::inspect_compiler_helper(sources, facts)?;
    Ok(())
}

fn require_local_method(
    block: &syn::Block,
    index: usize,
    name: &str,
    method_name: &str,
    receiver: &str,
) -> Result<(), String> {
    let expression = local(block, index)?;
    let syn::Expr::MethodCall(call) = expression else {
        return Err(format!("v8-compile-order:method:{method_name}"));
    };
    if super::source::path_name_from_expr(&call.receiver) != receiver
        || call.method != method_name
        || !call.args.is_empty()
    {
        return Err(format!("v8-compile-order:method:{method_name}"));
    }
    if !matches!(&block.stmts[index], syn::Stmt::Local(local) if matches!(&local.pat, syn::Pat::Ident(binding) if binding.ident == name))
    {
        return Err(format!("v8-compile-order:binding:{name}"));
    }
    Ok(())
}

fn require_required_allocation(
    block: &syn::Block,
    index: usize,
    binding: &str,
    error: &str,
    value: &str,
) -> Result<(), String> {
    let expression = local(block, index)?;
    let required_call = call(expression).ok_or_else(|| "v8-compile-order:required".to_string())?;
    if super::source::path_name_from_expr(&required_call.func) != "required"
        || required_call.args.len() != 2
        || !ident(&required_call.args[1], error)
    {
        return Err("v8-compile-order:required".into());
    }
    require_string_allocation(&required_call.args[0], value)?;
    if !matches!(&block.stmts[index], syn::Stmt::Local(local) if matches!(&local.pat, syn::Pat::Ident(pattern) if pattern.ident == binding))
    {
        return Err("v8-compile-order:allocation".into());
    }
    Ok(())
}

fn require_string_allocation(expression: &syn::Expr, value: &str) -> Result<(), String> {
    let Some(allocation) = call(expression) else {
        return Err("v8-compile-order:allocation".into());
    };
    if super::source::path_name_from_expr(&allocation.func) != "v8::String::new"
        || allocation.args.len() != 2
        || !ident(&allocation.args[0], "scope")
        || super::source::path_name_from_expr(&allocation.args[1]) != value
    {
        return Err("v8-compile-order:allocation".into());
    }
    Ok(())
}

fn require_compile(block: &syn::Block) -> Result<(), String> {
    let expression = local(block, COMPILE_STATEMENT_INDEX)?;
    let Some(ok_or_else) = method_call(expression) else {
        return Err("v8-compile-order:compile".into());
    };
    if ok_or_else.method != "ok_or_else" || !exception_closure(&ok_or_else.args) {
        return Err("v8-compile-order:compile".into());
    }
    let Some(compile) = call(&ok_or_else.receiver) else {
        return Err("v8-compile-order:compile".into());
    };
    if super::source::path_name_from_expr(&compile.func) != "ScriptCompiler::compile_with_origin"
        || compile.args.len() != COMPILE_ARGUMENT_COUNT
        || !ident(&compile.args[0], "scope")
        || !ident(&compile.args[1], "source")
        || !ident(&compile.args[2], "origin_name")
        || !matches!(&block.stmts[COMPILE_STATEMENT_INDEX], syn::Stmt::Local(local) if matches!(&local.pat, syn::Pat::Ident(pattern) if pattern.ident == "script"))
    {
        return Err("v8-compile-order:compile".into());
    }
    Ok(())
}

fn require_run(block: &syn::Block) -> Result<(), String> {
    let expression = local(block, RUN_STATEMENT_INDEX)?;
    let Some(ok_or_else) = method_call(expression) else {
        return Err("v8-compile-order:run".into());
    };
    if ok_or_else.method != "ok_or_else" || !exception_closure(&ok_or_else.args) {
        return Err("v8-compile-order:run".into());
    }
    let syn::Expr::MethodCall(run) = ok_or_else.receiver.as_ref() else {
        return Err("v8-compile-order:run".into());
    };
    if run.method != "run"
        || run.args.len() != 1
        || !ident(&run.args[0], "scope")
        || !ident(&run.receiver, "script")
        || !matches!(&block.stmts[RUN_STATEMENT_INDEX], syn::Stmt::Local(local) if matches!(&local.pat, syn::Pat::Ident(pattern) if pattern.ident == "value"))
    {
        return Err("v8-compile-order:run".into());
    }
    Ok(())
}

fn method_call(expression: &syn::Expr) -> Option<&syn::ExprMethodCall> {
    match expression {
        syn::Expr::MethodCall(call) => Some(call),
        syn::Expr::Try(value) => method_call(&value.expr),
        syn::Expr::Paren(value) => method_call(&value.expr),
        syn::Expr::Group(value) => method_call(&value.expr),
        _ => None,
    }
}

fn exception_closure(arguments: &syn::punctuated::Punctuated<syn::Expr, syn::Token![,]>) -> bool {
    let [syn::Expr::Closure(closure)] = arguments.iter().collect::<Vec<_>>().as_slice() else {
        return false;
    };
    let Some(actual) = call(&closure.body) else {
        return false;
    };
    super::source::path_name_from_expr(&actual.func) == "exception_message"
        && actual.args.len() == 1
        && ident(&actual.args[0], "scope")
}

fn require_resolve(block: &syn::Block) -> Result<(), String> {
    let syn::Stmt::Expr(expression, _) = &block.stmts[RESOLVE_STATEMENT_INDEX] else {
        return Err("v8-compile-order:resolve".into());
    };
    require_direct_call(
        expression,
        "resolve_value",
        &["scope", "value"],
        "v8-compile-order",
    )
}
