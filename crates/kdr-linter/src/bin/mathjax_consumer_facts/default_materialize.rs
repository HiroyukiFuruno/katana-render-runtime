use super::consumer::require_direct_call;
use super::source::{RUST_INPUTS, Source, call, ident, method, record_fact};
use super::syntax::{
    call_expr_path, is_some_binding, local_or_tail, ok_some_path, return_call, return_error,
    return_error_statement, return_expr_call,
};
use serde_json::Value;
use std::collections::BTreeMap;
use syn::spanned::Spanned;

const INSTALL_PATH_STATEMENTS: usize = 3;
const INSTALL_PATH_RESULT_INDEX: usize = 2;
const HELPER_SOURCE_INDEX: usize = 8;

pub(super) fn inspect_default_override(
    sources: &BTreeMap<&str, Source>,
    facts: &mut Vec<Value>,
) -> Result<(), String> {
    let path = RUST_INPUTS[0];
    verify_default_routes(
        &sources[path].ast,
        &sources[RUST_INPUTS[HELPER_SOURCE_INDEX]].ast,
        path,
        facts,
    )
}

fn verify_default_routes(
    file: &syn::File,
    helper: &syn::File,
    path: &str,
    facts: &mut Vec<Value>,
) -> Result<(), String> {
    if !super::materializer::canonical_materializer_impl(file, "MathJaxRendererOps") {
        return Err("default-materializer:renderer-owner-context".into());
    }
    let resolver = renderer_method(file, "resolve_mathjax_js")?;
    require_default_entry(&resolver.block)?;
    record_fact(facts, "explicit-override", path, resolver.span());
    let default = renderer_method(file, "resolve_mathjax_js_with_env")?;
    require_default_materialization(&default.block)?;
    super::materializer::require_generated_asset(file, helper, facts)?;
    let environment = renderer_method(file, "env_mathjax_js_from")?;
    require_environment_override(&environment.block)?;
    let install_path = renderer_method(file, "default_install_path")?;
    super::default_route::require_default_install_path(&install_path.block)?;
    record_fact(facts, "default-materialize", path, default.span());
    Ok(())
}

fn renderer_method<'a>(file: &'a syn::File, name: &str) -> Result<&'a syn::ImplItemFn, String> {
    let method = method(file, "MathJaxRendererOps", name)?;
    if !method.attrs.is_empty() {
        return Err("default-materializer:renderer-method-attributes".into());
    }
    Ok(method)
}

fn require_default_entry(block: &syn::Block) -> Result<(), String> {
    if block.stmts.len() != 1 {
        return Err("explicit-override:statement-count".into());
    }
    let expression = local_or_tail(block, 0)?;
    let call = call(expression).ok_or_else(|| "explicit-override:call".to_string())?;
    if super::source::path_name_from_expr(&call.func) != "Self::resolve_mathjax_js_with_env"
        || call.args.len() != 2
        || !env_var_call(&call.args[0])
        || !call_expr_path(&call.args[1], "Self::default_install_path", &[])
    {
        return Err("explicit-override:default-call".into());
    }
    Ok(())
}
fn env_var_call(expression: &syn::Expr) -> bool {
    let Some(actual) = call(expression) else {
        return false;
    };
    actual.args.len() == 1
        && super::source::path_name_from_expr(&actual.func) == "std::env::var_os"
        && matches!(&actual.args[0], syn::Expr::Lit(literal) if matches!(&literal.lit, syn::Lit::Str(value) if value.value() == "MATHJAX_JS"))
}
fn require_default_materialization(block: &syn::Block) -> Result<(), String> {
    if block.stmts.len() != INSTALL_PATH_STATEMENTS {
        return Err("default-materialize:statement-count".into());
    }
    require_env_materialization_branch(block)?;
    require_bundled_materialization(block)
}

fn require_env_materialization_branch(block: &syn::Block) -> Result<(), String> {
    let Some(syn::Stmt::Expr(syn::Expr::If(if_expr), _)) = block.stmts.first() else {
        return Err("default-materialize:env-branch".into());
    };
    let syn::Expr::Let(condition) = if_expr.cond.as_ref() else {
        return Err("default-materialize:env-branch".into());
    };
    let Some(syn::Expr::Try(try_expr)) = Some(condition.expr.as_ref()) else {
        return Err("default-materialize:env-branch".into());
    };
    if !is_some_binding(&condition.pat, "path")
        || !call_expr_path(&try_expr.expr, "Self::env_mathjax_js_from", &["env_value"])
        || if_expr.else_branch.is_some()
        || if_expr.then_branch.stmts.len() != 1
        || !return_call(if_expr.then_branch.stmts.first(), "Ok", &["path"])
    {
        return Err("default-materialize:env-branch".into());
    }
    Ok(())
}

fn require_bundled_materialization(block: &syn::Block) -> Result<(), String> {
    let Some(syn::Stmt::Local(local)) = block.stmts.get(1) else {
        return Err("default-materialize:path-binding".into());
    };
    let Some(init) = &local.init else {
        return Err("default-materialize:path-binding".into());
    };
    if !is_some_binding(&local.pat, "path")
        || !ident(&init.expr, "bundled_path")
        || !init.diverge.as_ref().is_some_and(|(_, diverge)| {
            return_error(diverge, "bundled MathJax path is unavailable")
        })
    {
        return Err("default-materialize:path-binding".into());
    }
    require_direct_call(
        local_or_tail(block, INSTALL_PATH_RESULT_INDEX)?,
        "MathJaxGeneratedRuntimeAsset::materialize_at",
        &["path"],
        "default-materialize",
    )
}
fn require_environment_override(block: &syn::Block) -> Result<(), String> {
    if block.stmts.len() != INSTALL_PATH_STATEMENTS {
        return Err("explicit-override:env-statement-count".into());
    }
    require_environment_binding(block)?;
    require_nonempty_environment_path(block)
}

fn require_environment_binding(block: &syn::Block) -> Result<(), String> {
    let Some(syn::Stmt::Local(local)) = block.stmts.first() else {
        return Err("explicit-override:env-bind".into());
    };
    if !is_some_binding(&local.pat, "path")
        || !ident(
            &local
                .init
                .as_ref()
                .ok_or("explicit-override:env-bind")?
                .expr,
            "value",
        )
        || !local.init.as_ref().is_some_and(|init| {
            init.diverge
                .as_ref()
                .is_some_and(|(_, diverge)| return_expr_call(diverge, "Ok", &["None"]))
        })
    {
        return Err("explicit-override:env-bind".into());
    }
    Ok(())
}

fn require_nonempty_environment_path(block: &syn::Block) -> Result<(), String> {
    let Some(syn::Stmt::Expr(syn::Expr::If(check), _)) = block.stmts.get(1) else {
        return Err("explicit-override:empty-check".into());
    };
    let syn::Expr::MethodCall(empty) = check.cond.as_ref() else {
        return Err("explicit-override:empty-check".into());
    };
    if empty.method != "is_empty"
        || !empty.args.is_empty()
        || !ident(&empty.receiver, "path")
        || check.else_branch.is_some()
    {
        return Err("explicit-override:empty-check".into());
    }
    if check.then_branch.stmts.len() != 1
        || !check
            .then_branch
            .stmts
            .first()
            .is_some_and(|statement| return_error_statement(statement, "MATHJAX_JS is empty"))
        || !ok_some_path(local_or_tail(block, INSTALL_PATH_RESULT_INDEX)?)
    {
        return Err("explicit-override:path-conversion".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "default_materialize_private_ast_tests.rs"]
mod private_ast_tests;
